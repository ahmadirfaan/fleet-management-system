import { create } from "zustand";
import type { TelemetryEvent, Alert, HistoryPoint, LiveTrailPoint } from "../types";

const API = import.meta.env.VITE_API_BASE_URL ?? "";

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
    const chronological = [...points].reverse();
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
