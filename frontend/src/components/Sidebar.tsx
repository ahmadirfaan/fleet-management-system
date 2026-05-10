import { useFleetStore } from "../store/fleetStore";
import { AlertTriangle, CheckCircle2, Truck } from "lucide-react";

export default function Sidebar() {
  const trucks = useFleetStore((s) => s.trucks);
  const alerts = useFleetStore((s) => s.alerts);
  const acknowledgeAlert = useFleetStore((s) => s.acknowledgeAlert);
  const selectTruck = useFleetStore((s) => s.selectTruck);
  const enterHistoryMode = useFleetStore((s) => s.enterHistoryMode);

  const unacked = alerts.filter((a) => !a.is_acknowledged);
  const acked = alerts.filter((a) => a.is_acknowledged).slice(0, 10);

  return (
    <aside className="w-72 flex flex-col bg-fleet-panel border-r border-fleet-border overflow-y-auto">
      {/* Header */}
      <div className="px-4 py-3 border-b border-fleet-border">
        <h1 className="text-sm font-bold tracking-widest uppercase text-fleet-muted">
          SYNPS MINI FLEET
        </h1>
        <p className="text-xs text-fleet-muted mt-0.5">Command Center</p>
      </div>

      {/* Fleet summary */}
      <section className="px-4 py-3 border-b border-fleet-border">
        <h2 className="text-xs font-semibold uppercase text-fleet-muted mb-2">Fleet Status</h2>
        <div className="space-y-1">
          {Object.values(trucks).map(({ telemetry, isStale }) => {
            const isCritical = telemetry.speed_kmh > 60 || telemetry.engine_rpm > 2500;
            const color = isStale
              ? "text-fleet-muted"
              : isCritical
              ? "text-fleet-red"
              : "text-fleet-green";
            return (
              <button
                key={telemetry.fleet_id}
                className="w-full flex items-center gap-2 text-left px-2 py-1.5 rounded hover:bg-fleet-border transition-colors"
                onClick={() => selectTruck(telemetry.fleet_id)}
              >
                <Truck size={14} className={color} />
                <span className="text-xs font-mono flex-1">{telemetry.fleet_id}</span>
                <span className="text-xs text-fleet-muted">{telemetry.operational_state}</span>
              </button>
            );
          })}
          {Object.keys(trucks).length === 0 && (
            <p className="text-xs text-fleet-muted">Awaiting telemetry...</p>
          )}
        </div>
      </section>

      {/* Unacknowledged alerts */}
      <section className="px-4 py-3 border-b border-fleet-border flex-1">
        <h2 className="text-xs font-semibold uppercase text-fleet-muted mb-2 flex items-center gap-1">
          <AlertTriangle size={12} className="text-fleet-red" />
          Active Alerts ({unacked.length})
        </h2>
        <div className="space-y-2">
          {unacked.map((a) => (
            <div
              key={a.id}
              className={`rounded p-2 text-xs border ${
                a.severity === "CRITICAL"
                  ? "border-fleet-red/50 bg-fleet-red/10"
                  : "border-fleet-yellow/50 bg-fleet-yellow/10"
              }`}
            >
              <div className="flex justify-between items-start">
                <div>
                  <span className="font-bold">{a.call_sign}</span>
                  <span className="ml-1 text-fleet-muted">{a.alert_type}</span>
                </div>
                <span
                  className={`text-[10px] font-bold ${
                    a.severity === "CRITICAL" ? "text-fleet-red" : "text-fleet-yellow"
                  }`}
                >
                  {a.severity}
                </span>
              </div>
              <p className="text-fleet-muted mt-0.5">
                {new Date(a.start_timestamp).toLocaleTimeString()}
              </p>
              <div className="flex gap-1 mt-1.5">
                <button
                  onClick={() => acknowledgeAlert(a.id)}
                  className="flex items-center gap-1 bg-fleet-green/20 text-fleet-green border border-fleet-green/40 rounded px-2 py-0.5 text-[10px] hover:bg-fleet-green/30"
                >
                  <CheckCircle2 size={10} /> Acknowledge
                </button>
                <button
                  onClick={() => enterHistoryMode(a.call_sign)}
                  className="flex items-center gap-1 bg-fleet-border text-fleet-muted border border-fleet-border rounded px-2 py-0.5 text-[10px] hover:bg-fleet-border/70"
                >
                  History
                </button>
              </div>
            </div>
          ))}
          {unacked.length === 0 && (
            <p className="text-xs text-fleet-muted">No active alerts</p>
          )}
        </div>
      </section>

      {/* Alert history */}
      {acked.length > 0 && (
        <section className="px-4 py-3">
          <h2 className="text-xs font-semibold uppercase text-fleet-muted mb-2">
            Alert History
          </h2>
          <div className="space-y-1">
            {acked.map((a) => (
              <div key={a.id} className="text-xs text-fleet-muted flex justify-between">
                <span>{a.call_sign} – {a.alert_type}</span>
                <CheckCircle2 size={12} className="text-fleet-green" />
              </div>
            ))}
          </div>
        </section>
      )}
    </aside>
  );
}
