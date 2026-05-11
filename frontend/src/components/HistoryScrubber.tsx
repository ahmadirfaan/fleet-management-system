import { useMemo } from "react";
import { useFleetStore } from "../store/fleetStore";
import { X } from "lucide-react";
import type { HistoryPoint } from "../types";

// ── Elevation chart (pure SVG, no library needed) ────────────────────────────

interface ElevationChartProps {
  points: HistoryPoint[];
  scrubIndex: number;
  onScrub: (i: number) => void;
}

function ElevationChart({ points, scrubIndex, onScrub }: ElevationChartProps) {
  const W = 600;
  const H = 60;
  const PAD = 4;

  const elevations = useMemo(
    () => points.map((p) => p.elevation_meters ?? 0),
    [points]
  );

  const min = useMemo(() => Math.min(...elevations), [elevations]);
  const max = useMemo(() => Math.max(...elevations), [elevations]);
  const range = max - min || 1;

  // Build SVG polyline points string
  const polylinePoints = useMemo(() => {
    return elevations
      .map((e, i) => {
        const x = PAD + (i / (elevations.length - 1)) * (W - 2 * PAD);
        const y = H - PAD - ((e - min) / range) * (H - 2 * PAD);
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  }, [elevations, min, range]);

  // Area fill path
  const areaPath = useMemo(() => {
    if (elevations.length === 0) return "";
    const pts = elevations.map((e, i) => {
      const x = PAD + (i / (elevations.length - 1)) * (W - 2 * PAD);
      const y = H - PAD - ((e - min) / range) * (H - 2 * PAD);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    return `M ${pts[0]} L ${pts.join(" L ")} L ${(PAD + (W - 2 * PAD)).toFixed(1)},${(H - PAD).toFixed(1)} L ${PAD},${(H - PAD).toFixed(1)} Z`;
  }, [elevations, min, range]);

  // Scrub marker X position
  const scrubX =
    elevations.length > 1
      ? PAD + (scrubIndex / (elevations.length - 1)) * (W - 2 * PAD)
      : PAD;
  const scrubE = elevations[scrubIndex] ?? min;
  const scrubY = H - PAD - ((scrubE - min) / range) * (H - 2 * PAD);

  // Handle click on SVG to jump scrub position
  const handleClick = (e: React.MouseEvent<SVGSVGElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const relX = e.clientX - rect.left;
    const fraction = (relX - PAD) / (W - 2 * PAD);
    const idx = Math.round(fraction * (points.length - 1));
    onScrub(Math.max(0, Math.min(points.length - 1, idx)));
  };

  if (elevations.length < 2) return null;

  return (
    <div className="w-full">
      <div className="flex justify-between text-[9px] text-fleet-muted mb-0.5 px-1">
        <span>ELEVATION (m)</span>
        <span>{min.toFixed(0)} – {max.toFixed(0)} m</span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="w-full cursor-crosshair"
        style={{ height: 60 }}
        onClick={handleClick}
      >
        {/* Area fill */}
        <path d={areaPath} fill="#d29922" fillOpacity={0.15} />
        {/* Line */}
        <polyline
          points={polylinePoints}
          fill="none"
          stroke="#d29922"
          strokeWidth="1.5"
          strokeLinejoin="round"
        />
        {/* Scrub vertical line */}
        <line
          x1={scrubX}
          y1={PAD}
          x2={scrubX}
          y2={H - PAD}
          stroke="#f0f6fc"
          strokeWidth="1"
          strokeDasharray="3 2"
        />
        {/* Scrub dot */}
        <circle cx={scrubX} cy={scrubY} r="3" fill="#d29922" />
      </svg>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

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
    <div className="absolute bottom-0 left-0 right-0 z-[1000] bg-fleet-panel/95 border-t border-fleet-border px-4 py-3 backdrop-blur">
      {/* Header row */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] font-semibold text-fleet-muted tracking-widest uppercase">
          History — {historyTruckId}
        </span>
        <button
          onClick={exitHistoryMode}
          className="flex items-center gap-1 text-fleet-muted hover:text-fleet-text text-xs"
        >
          <X size={14} /> Live
        </button>
      </div>

      {/* Elevation chart */}
      <ElevationChart
        points={historyPoints}
        scrubIndex={historyScrubIndex}
        onScrub={setScrubIndex}
      />

      {/* Scrubber + stats row */}
      <div className="flex items-center gap-4 mt-1">
        <div className="flex-1">
          <div className="flex justify-between text-[10px] text-fleet-muted mb-1">
            <span>{historyPoints.length > 0 ? new Date(historyPoints[0].timestamp).toLocaleString() : ""}</span>
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
          <div className="text-xs text-fleet-muted font-mono min-w-[160px] space-y-0.5">
            <p>{current.speed_kmh?.toFixed(1)} km/h | {current.operational_state}</p>
            <p>RPM: {current.engine_rpm ?? "—"}</p>
            {current.elevation_meters != null && (
              <p>Elev: {current.elevation_meters.toFixed(0)} m</p>
            )}
            <p>Fuel: {current.fuel_level_percent?.toFixed(1) ?? "—"}%</p>
          </div>
        )}
      </div>
    </div>
  );
}
