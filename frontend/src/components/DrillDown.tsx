import { X, History } from "lucide-react";
import { useFleetStore } from "../store/fleetStore";

export default function DrillDown() {
  const trucks = useFleetStore((s) => s.trucks);
  const selectedTruckId = useFleetStore((s) => s.selectedTruckId);
  const selectTruck = useFleetStore((s) => s.selectTruck);
  const enterHistoryMode = useFleetStore((s) => s.enterHistoryMode);

  if (!selectedTruckId) return null;
  const state = trucks[selectedTruckId];
  if (!state) return null;

  const { telemetry } = state;

  const stats = [
    { label: "Speed", value: `${telemetry.speed_kmh.toFixed(1)} km/h` },
    { label: "Engine RPM", value: telemetry.engine_rpm.toLocaleString() },
    { label: "Load", value: `${telemetry.payload_weight_tons.toFixed(0)} t` },
    { label: "Fuel", value: `${telemetry.fuel_level_percent.toFixed(1)} %` },
    { label: "Elevation", value: `${telemetry.elevation_meters.toFixed(0)} m` },
    { label: "Heading", value: `${telemetry.heading_degrees}°` },
  ];

  return (
    <div className="absolute right-0 top-0 bottom-0 w-72 bg-fleet-panel border-l border-fleet-border z-[1000] flex flex-col shadow-2xl">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-fleet-border">
        <div>
          <h2 className="text-sm font-bold">{telemetry.fleet_id}</h2>
          <p className="text-xs text-fleet-muted">{telemetry.operational_state}</p>
        </div>
        <button onClick={() => selectTruck(null)} className="text-fleet-muted hover:text-fleet-text">
          <X size={16} />
        </button>
      </div>

      {/* Telemetry grid */}
      <div className="grid grid-cols-2 gap-2 px-4 py-3 border-b border-fleet-border">
        {stats.map((s) => (
          <div key={s.label} className="bg-fleet-bg rounded p-2">
            <p className="text-[10px] text-fleet-muted uppercase">{s.label}</p>
            <p className="text-sm font-mono font-bold mt-0.5">{s.value}</p>
          </div>
        ))}
      </div>

      {/* Anomaly badge */}
      {telemetry.is_anomaly && (
        <div className="mx-4 mt-3 px-3 py-2 rounded bg-fleet-red/20 border border-fleet-red/50 text-fleet-red text-xs font-semibold">
          ⚠ {telemetry.anomaly_type ?? "ANOMALY"}
        </div>
      )}

      {/* Actions */}
      <div className="px-4 py-3 mt-auto border-t border-fleet-border">
        <button
          onClick={() => { enterHistoryMode(telemetry.fleet_id); selectTruck(null); }}
          className="w-full flex items-center justify-center gap-2 bg-fleet-border hover:bg-fleet-border/70 text-fleet-text text-xs rounded px-3 py-2"
        >
          <History size={14} /> View History
        </button>
      </div>
    </div>
  );
}
