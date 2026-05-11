import { MapContainer, TileLayer, Marker, Popup, Polyline, useMap } from "react-leaflet";
import L from "leaflet";
import { useFleetStore } from "../store/fleetStore";
import { useState, useEffect } from "react";

// Fix Leaflet default icon paths broken by Vite bundling.
delete (L.Icon.Default.prototype as unknown as Record<string, unknown>)._getIconUrl;
L.Icon.Default.mergeOptions({
  iconRetinaUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png",
  iconUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png",
  shadowUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png",
});

/** Build a coloured div-icon for a truck marker. */
function truckIcon(color: string, heading: number, zoom: number): L.DivIcon {
  const baseSize = 36;
  const scale = Math.pow(1.3, zoom - 12);
  const size = Math.min(Math.max(20, baseSize * scale), 120);

  const truckSvg = `
    <svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" 
      style="transform: rotate(${heading}deg); transition: transform 0.5s ease, width 0.2s, height 0.2s;">
      <rect x="6" y="4" width="12" height="16" rx="2" fill="${color}" stroke="white" stroke-width="1"/>
      <rect x="7" y="5" width="10" height="4" rx="1" fill="white" fill-opacity="0.3"/>
      <rect x="7" y="10" width="10" height="9" rx="1" fill="black" fill-opacity="0.2"/>
      <rect x="4" y="6" width="2" height="4" fill="#333"/>
      <rect x="18" y="6" width="2" height="4" fill="#333"/>
      <rect x="4" y="14" width="2" height="4" fill="#333"/>
      <rect x="18" y="14" width="2" height="4" fill="#333"/>
    </svg>
  `;

  return L.divIcon({
    className: "responsive-truck-marker",
    html: `
      <div style="
          display: flex; 
          align-items: center; 
          justify-content: center;
          pointer-events:none
          ">
          ${truckSvg}
      </div>`,
    iconSize: [size, size],
    iconAnchor: [size / 2, size / 2],
  });
}

function ZoomListener({ setZoom }: { setZoom: (z: number) => void }) {
  const map = useMap();

  useEffect(() => {
    const onZoom = () => setZoom(map.getZoom());
    map.on("zoomend", onZoom);
    return () => {
      map.off("zoomend", onZoom);
    };
  }, [map, setZoom]);

  return null;
}

/** Re-centers the map on history route when entering history mode. */
function HistoryFit({ positions }: { positions: [number, number][] }) {
  const map = useMap();
  useEffect(() => {
    if (positions.length > 1) {
      map.fitBounds(positions);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [positions.length]);
  return null;
}

// Distinct trail colours per truck so overlapping paths are easy to read.
const TRAIL_COLORS = [
  "#58a6ff", // blue   — HT-001
  "#3fb950", // green  — HT-002
  "#d29922", // yellow — HT-003
  "#f78166", // orange — HT-004
  "#bc8cff", // purple — HT-005
];

function trailColor(fleetId: string, index: number): string {
  // Try to derive colour from truck index (HT-001 → 0, HT-002 → 1, …)
  const match = fleetId.match(/(\d+)$/);
  if (match) {
    const n = parseInt(match[1], 10) - 1;
    return TRAIL_COLORS[n % TRAIL_COLORS.length];
  }
  return TRAIL_COLORS[index % TRAIL_COLORS.length];
}

export default function FleetMap() {
  const [currentZoom, setCurrentZoom] = useState(12);
  const trucks = useFleetStore((s) => s.trucks);
  const selectTruck = useFleetStore((s) => s.selectTruck);
  const isLiveMode = useFleetStore((s) => s.isLiveMode);
  const historyPoints = useFleetStore((s) => s.historyPoints);
  const historyScrubIndex = useFleetStore((s) => s.historyScrubIndex);

  const historyPositions: [number, number][] = historyPoints.map((p) => [p.latitude, p.longitude]);
  const scrubPoint = historyPoints[historyScrubIndex];

  // Positions up to the current scrub index (so polyline grows as user scrubs).
  const scrubPositions: [number, number][] = historyPositions.slice(0, historyScrubIndex + 1);

  return (
    <MapContainer
      center={[-0.495, 116.865]}
      zoom={11}
      style={{ height: "100%", width: "100%", background: "#0d1117" }}
      zoomControl={false}
    >
      <ZoomListener setZoom={setCurrentZoom} />
      <TileLayer
        url="https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png"
        attribution='&copy; <a href="https://carto.com/">CARTO</a>'
      />

      {/* ── Live mode: per-truck trail polylines + current position markers ── */}
      {isLiveMode &&
        Object.values(trucks).map(({ telemetry, isStale, trail }, idx) => {
          const isCritical = telemetry.speed_kmh > 60 || telemetry.engine_rpm > 2500;
          const color = isStale ? "#8b949e" : isCritical ? "#f85149" : "#3fb950";
          const trailCol = trailColor(telemetry.fleet_id, idx);

          const trailPositions: [number, number][] = trail.map((p) => [p.lat, p.lon]);

          return (
            <div key={telemetry.fleet_id}>
              {/* Live trail polyline — faded colour of the truck */}
              {trailPositions.length > 1 && (
                <Polyline
                  positions={trailPositions}
                  color={trailCol}
                  weight={2}
                  opacity={0.55}
                  dashArray="4 3"
                />
              )}

              {/* Current position marker */}
              <Marker
                position={[telemetry.latitude, telemetry.longitude]}
                icon={truckIcon(color, telemetry.heading_degrees, currentZoom)}
                eventHandlers={{ click: () => selectTruck(telemetry.fleet_id) }}
              >
                <Popup>
                  <strong>{telemetry.fleet_id}</strong>
                  <br />
                  {telemetry.operational_state} | {telemetry.speed_kmh.toFixed(1)} km/h
                  <br />
                  RPM: {telemetry.engine_rpm} | Elev: {telemetry.elevation_meters.toFixed(0)} m
                </Popup>
              </Marker>
            </div>
          );
        })}

      {/* ── History mode: full route polyline + scrub marker ── */}
      {!isLiveMode && historyPositions.length > 0 && (
        <>
          <HistoryFit positions={historyPositions} />
          {/* Full ghost route */}
          <Polyline positions={historyPositions} color="#30363d" weight={2} opacity={0.5} />
          {/* Elapsed path up to scrub position */}
          <Polyline positions={scrubPositions} color="#d29922" weight={3} />
          {scrubPoint && (
            <Marker
              position={[scrubPoint.latitude, scrubPoint.longitude]}
              icon={truckIcon("#d29922", 0, currentZoom)}
            >
              <Popup>
                {new Date(scrubPoint.timestamp).toLocaleTimeString()}
                <br />
                {scrubPoint.speed_kmh?.toFixed(1)} km/h | {scrubPoint.operational_state}
                {scrubPoint.elevation_meters != null && (
                  <><br />Elev: {scrubPoint.elevation_meters.toFixed(0)} m</>
                )}
              </Popup>
            </Marker>
          )}
        </>
      )}
    </MapContainer>
  );
}
