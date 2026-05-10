import { create } from "zustand";
import type { TelemetryEvent, Alert, HistoryPoint } from "../types";

const API = import.meta.env.VITE_API_BASE_URL ?? "";

// ── Truck live state ──────────────────────────────────────────────────────────

export interface TruckLiveState {
  telemetry: TelemetryEvent;
  lastSeen: number; // Date.now()
  isStale: boolean; // no data >30s
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
    // GPS glitch → do NOT update position (is_anomaly + anomaly_type == GPS_GLITCH)
    const isGpsGlitch = event.is_anomaly && event.anomaly_type === "GPS_GLITCH";

    set((state) => {
      const existing = state.trucks[event.fleet_id];
      const updated: TruckLiveState = {
        telemetry: isGpsGlitch && existing
          ? { ...existing.telemetry, is_anomaly: true, anomaly_type: "GPS_GLITCH" }
          : event,
        lastSeen: Date.now(),
        isStale: false,
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
    const res = await fetch(`${API}/api/history?truck_id=${truckId}&limit=500`);
    const points: HistoryPoint[] = await res.json();
    set({
      isLiveMode: false,
      historyTruckId: truckId,
      historyPoints: points.reverse(), // chronological
      historyScrubIndex: points.length - 1,
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
