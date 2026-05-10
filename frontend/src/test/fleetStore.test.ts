import { describe, it, expect, beforeEach } from "vitest";
import { useFleetStore } from "../store/fleetStore";
import type { TelemetryEvent } from "../types";

// Helper to build a minimal telemetry event.
function makeTel(overrides: Partial<TelemetryEvent> = {}): TelemetryEvent {
  return {
    fleet_id: "HT-001",
    timestamp: new Date().toISOString(),
    latitude: -0.51,
    longitude: 116.83,
    elevation_meters: 150,
    speed_kmh: 30,
    engine_rpm: 1500,
    fuel_level_percent: 80,
    payload_weight_tons: 100,
    heading_degrees: 90,
    operational_state: "HAULING",
    is_anomaly: false,
    anomaly_type: null,
    ...overrides,
  };
}

describe("Zustand fleetStore", () => {
  beforeEach(() => {
    useFleetStore.setState({
      trucks: {},
      alerts: [],
      isLiveMode: true,
      historyTruckId: null,
      historyPoints: [],
      historyScrubIndex: 0,
      selectedTruckId: null,
    });
  });

  // ── Positive tests ──────────────────────────────────────────────────────────

  it("[POS-01] applies a normal telemetry event and updates store", () => {
    const tel = makeTel();
    useFleetStore.getState().applyTelemetry(tel);
    const state = useFleetStore.getState().trucks["HT-001"];
    expect(state).toBeDefined();
    expect(state.telemetry.speed_kmh).toBe(30);
    expect(state.isStale).toBe(false);
  });

  it("[POS-02] updates existing truck position on new telemetry", () => {
    useFleetStore.getState().applyTelemetry(makeTel({ latitude: -0.51, longitude: 116.83 }));
    useFleetStore.getState().applyTelemetry(makeTel({ latitude: -0.52, longitude: 116.84 }));
    const { telemetry } = useFleetStore.getState().trucks["HT-001"];
    expect(telemetry.latitude).toBeCloseTo(-0.52);
  });

  it("[POS-03] selectTruck sets selectedTruckId", () => {
    useFleetStore.getState().selectTruck("HT-002");
    expect(useFleetStore.getState().selectedTruckId).toBe("HT-002");
  });

  it("[POS-04] selectTruck(null) clears drill-down", () => {
    useFleetStore.getState().selectTruck("HT-001");
    useFleetStore.getState().selectTruck(null);
    expect(useFleetStore.getState().selectedTruckId).toBeNull();
  });

  it("[POS-05] setAlerts stores alert list", () => {
    const mockAlert = {
      id: "alert-1",
      call_sign: "HT-001",
      alert_type: "OVERSPEED",
      severity: "WARNING" as const,
      start_timestamp: new Date().toISOString(),
      is_acknowledged: false,
      telemetry_snapshot: null,
    };
    useFleetStore.getState().setAlerts([mockAlert]);
    expect(useFleetStore.getState().alerts).toHaveLength(1);
  });

  it("[POS-06] exitHistoryMode restores live mode", () => {
    useFleetStore.setState({ isLiveMode: false, historyTruckId: "HT-001" });
    useFleetStore.getState().exitHistoryMode();
    expect(useFleetStore.getState().isLiveMode).toBe(true);
    expect(useFleetStore.getState().historyTruckId).toBeNull();
  });

  it("[POS-07] setScrubIndex updates index", () => {
    useFleetStore.setState({ historyPoints: [{} as never, {} as never, {} as never], historyScrubIndex: 0 });
    useFleetStore.getState().setScrubIndex(2);
    expect(useFleetStore.getState().historyScrubIndex).toBe(2);
  });

  it("[POS-08] markStale does not stale a truck seen recently", () => {
    const tel = makeTel();
    useFleetStore.getState().applyTelemetry(tel);
    useFleetStore.getState().markStale();
    expect(useFleetStore.getState().trucks["HT-001"].isStale).toBe(false);
  });

  // ── Negative tests ──────────────────────────────────────────────────────────

  it("[NEG-01] GPS_GLITCH does NOT update truck position", () => {
    // Establish baseline position.
    useFleetStore.getState().applyTelemetry(makeTel({ latitude: -0.51, longitude: 116.83 }));
    // Send a glitch event with a far-away position.
    useFleetStore.getState().applyTelemetry(
      makeTel({ latitude: -2.51, longitude: 118.83, is_anomaly: true, anomaly_type: "GPS_GLITCH" })
    );
    const { telemetry } = useFleetStore.getState().trucks["HT-001"];
    // Position must remain at baseline, not jump.
    expect(telemetry.latitude).toBeCloseTo(-0.51);
    expect(telemetry.longitude).toBeCloseTo(116.83);
  });

  it("[NEG-02] GPS_GLITCH sets anomaly flag on cached state", () => {
    useFleetStore.getState().applyTelemetry(makeTel({ latitude: -0.51, longitude: 116.83 }));
    useFleetStore.getState().applyTelemetry(
      makeTel({ latitude: -2.51, longitude: 118.83, is_anomaly: true, anomaly_type: "GPS_GLITCH" })
    );
    expect(useFleetStore.getState().trucks["HT-001"].telemetry.is_anomaly).toBe(true);
  });

  it("[NEG-03] markStale marks truck with lastSeen >30s ago", () => {
    const tel = makeTel();
    useFleetStore.getState().applyTelemetry(tel);
    // Manually backdate lastSeen.
    useFleetStore.setState((s) => ({
      trucks: {
        ...s.trucks,
        "HT-001": { ...s.trucks["HT-001"], lastSeen: Date.now() - 35_000 },
      },
    }));
    useFleetStore.getState().markStale();
    expect(useFleetStore.getState().trucks["HT-001"].isStale).toBe(true);
  });

  it("[NEG-04] unknown truck id is not affected by GPS_GLITCH from another truck", () => {
    useFleetStore.getState().applyTelemetry(makeTel({ fleet_id: "HT-002", latitude: -0.60 }));
    // Send glitch for HT-001 only.
    useFleetStore.getState().applyTelemetry(
      makeTel({ fleet_id: "HT-001", latitude: -2.0, is_anomaly: true, anomaly_type: "GPS_GLITCH" })
    );
    // HT-002 should be unaffected and still undefined (not in store for HT-001 glitch).
    const ht002 = useFleetStore.getState().trucks["HT-002"];
    expect(ht002.telemetry.latitude).toBeCloseTo(-0.60);
  });

  it("[NEG-05] setAlerts with empty array clears alert list", () => {
    useFleetStore.setState({ alerts: [{ id: "x" } as never] });
    useFleetStore.getState().setAlerts([]);
    expect(useFleetStore.getState().alerts).toHaveLength(0);
  });
});
