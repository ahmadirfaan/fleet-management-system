import { useFleetStore } from "../store/fleetStore";
import { X } from "lucide-react";

export default function HistoryScrubber() {
  const isLiveMode = useFleetStore((s) => s.isLiveMode);
  const historyTruckId = useFleetStore((s) => s.historyTruckId);
  const historyPoints = useFleetStore((s) => s.historyPoints);
  const historyScrubIndex = useFleetStore((s) => s.historyScrubIndex);
  const setScrubIndex = useFleetStore((s) => s.setScrubIndex);
  const exitHistoryMode = useFleetStore((s) => s.exitHistoryMode);

  if (isLiveMode || historyPoints.length === 0) return null;

  const current = historyPoints[historyScrubIndex];

  return (
    <div className="absolute bottom-0 left-0 right-0 z-[1000] bg-fleet-panel/95 border-t border-fleet-border px-6 py-3 backdrop-blur">
      <div className="flex items-center gap-4">
        <div className="flex-1">
          <div className="flex justify-between text-[10px] text-fleet-muted mb-1">
            <span>HISTORY MODE — {historyTruckId}</span>
            <span>{current ? new Date(current.timestamp).toLocaleString() : ""}</span>
          </div>
          <input
            type="range"
            min={0}
            max={historyPoints.length - 1}
            value={historyScrubIndex}
            onChange={(e) => setScrubIndex(Number(e.target.value))}
            className="w-full accent-fleet-yellow"
          />
        </div>
        {current && (
          <div className="text-xs text-fleet-muted font-mono min-w-[140px]">
            <p>{current.speed_kmh?.toFixed(1)} km/h</p>
            <p>{current.operational_state}</p>
          </div>
        )}
        <button
          onClick={exitHistoryMode}
          className="flex items-center gap-1 text-fleet-muted hover:text-fleet-text text-xs"
        >
          <X size={14} /> Live
        </button>
      </div>
    </div>
  );
}
