import { create } from "zustand";
import type { TelemetryEvent, Alert, HistoryPoint, LiveTrailPoint } from "../types";

const API = import.meta.env.VITE_API_BASE_URL ?? "";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Great-circle distance in km between two lat/lon points. */
function haversineKm(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const R = 6371;
  const dLat = ((lat2 - lat1) * Math.PI) / 180;
  const dLon = ((lon2 - lon1) * Math.PI) / 180;
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((lat1 * Math.PI) / 180) *
      Math.cos((lat2 * Math.PI) / 180) *
      Math.sin(dLon / 2) ** 2;
  return R * 2 * Math.asin(Math.sqrt(a));
}

/**
 * Remove points that imply an impossible speed jump from the previous point
 * (> 200 km/h implied by distance/time).  This cleans up GPS glitch rows that
 * pre-date the is_anomaly DB column, as well as any that slip through.
 */
function filterGlitchPoints(points: HistoryPoint[]): HistoryPoint[] {
  const out: HistoryPoint[] = [];
  for (const pt of points) {
    if (out.length === 0) {
      out.push(pt);
      continue;
    }
    const prev = out[out.length - 1];
    const distKm = haversineKm(prev.latitude, prev.longitude, pt.latitude, pt.longitude);
    const dtH = (new Date(pt.timestamp).getTime() - new Date(prev.timestamp).getTime()) / 3_600_000;
    const impliedSpeed = dtH > 0 ? distKm / dtH : Infinity;
    if (impliedSpeed <= 200) {
      out.push(pt);
    }
    // else: silently drop the glitch point — it won't appear in the polyline
  }
  return out;
}

// ── Constants ─────────────────────────────────────────────────────────────────

/** Maximum number of live trail points kept per truck (ring buffer). */
const LIVE_TRAIL_MAX = 200;

// ── Truck live state ──────────────────────────────────────────────────────────

export interface TruckLiveState {
  telemetry: TelemetryEvent;
  lastSeen: number; // Date.now()
  isStale: boolean; // no data >30s
  /** Chronological ring-buffer of recent positions for the live polyline. */
  trail: LiveTrailPoint[];
}

interface FleetStore {
  // Live tracking
  trucks: Record<string, TruckLiveState>;
  alerts: Alert[];
  isLiveMode: boolean;

  // History mode
  historyTruckId: string | null;
  historyPoints: HistoryPoint[];
  historyScrubIndex: number;

  // Selected truck (drill-down)
  selectedTruckId: string | null;

  // Actions
  applyTelemetry: (event: TelemetryEvent) => void;
  setAlerts: (alerts: Alert[]) => void;
  acknowledgeAlert: (id: string) => Promise<void>;
  selectTruck: (id: string | null) => void;
  enterHistoryMode: (truckId: string) => Promise<void>;
  exitHistoryMode: () => void;
  setScrubIndex: (i: number) => void;
  markStale: () => void;
}

export const useFleetStore = create<FleetStore>((set, get) => ({
  trucks: {},
  alerts: [],
  isLiveMode: true,
  historyTruckId: null,
  historyPoints: [],
  historyScrubIndex: 0,
  selectedTruckId: null,

  applyTelemetry: (event) => {
    const isGpsGlitch = event.is_anomaly && event.anomaly_type === "GPS_GLITCH";

    set((state) => {
      const existing = state.trucks[event.fleet_id];

      // Build updated trail: keep prior trail, append new point (unless GPS glitch —
      // we don't want the 150 km jump in the visual trail).
      const prevTrail: LiveTrailPoint[] = existing?.trail ?? [];
      const newTrail: LiveTrailPoint[] = isGpsGlitch
        ? prevTrail
        : [
            ...prevTrail.slice(-(LIVE_TRAIL_MAX - 1)),
            { lat: event.latitude, lon: event.longitude, timestamp: Date.now() },
          ];

      const updated: TruckLiveState = {
        telemetry: isGpsGlitch && existing
          ? { ...existing.telemetry, is_anomaly: true, anomaly_type: "GPS_GLITCH" }
          : event,
        lastSeen: Date.now(),
        isStale: false,
        trail: newTrail,
      };
      return { trucks: { ...state.trucks, [event.fleet_id]: updated } };
    });
  },

  setAlerts: (alerts) => set({ alerts }),

  acknowledgeAlert: async (id) => {
    await fetch(`${API}/api/alerts/${id}/acknowledge`, { method: "PUT" });
    set((state) => ({
      alerts: state.alerts.map((a) =>
        a.id === id ? { ...a, is_acknowledged: true } : a
      ),
    }));
  },

  selectTruck: (id) => set({ selectedTruckId: id }),

  enterHistoryMode: async (truckId) => {
    // Fetch enough points to cover multiple full circuit laps.
    // One full circuit at ~10 m/s average takes ~300–400 seconds.
    // 2000 points = ~33 minutes of data = at least 5 full laps visible.
    const res = await fetch(`${API}/api/history?truck_id=${truckId}&limit=2000`);
    const points: HistoryPoint[] = await res.json();
    // API returns latest-first; reverse to chronological so the polyline
    // draws oldest→newest (left→right on the scrubber).
    // Two-pass clean:
    //   1. Drop rows explicitly flagged is_anomaly=true (new data, post-migration).
    //   2. Drop any remaining points that imply >200 km/h speed jump (old data,
    //      pre-migration, or any that slipped through the DB flag).
    const chronological = filterGlitchPoints(
      [...points].reverse().filter((p) => !p.is_anomaly)
    );
    set({
      isLiveMode: false,
      historyTruckId: truckId,
      historyPoints: chronological,
      historyScrubIndex: chronological.length - 1,
    });
  },

  exitHistoryMode: () =>
    set({ isLiveMode: true, historyTruckId: null, historyPoints: [], historyScrubIndex: 0 }),

  setScrubIndex: (i) => set({ historyScrubIndex: i }),

  markStale: () => {
    const now = Date.now();
    set((state) => {
      const updated = { ...state.trucks };
      for (const id in updated) {
        if (now - updated[id].lastSeen > 30_000) {
          updated[id] = { ...updated[id], isStale: true };
        }
      }
      return { trucks: updated };
    });
  },
}));
