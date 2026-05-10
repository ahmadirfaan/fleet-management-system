import { MapContainer, TileLayer, Marker, Popup, Polyline, useMap } from "react-leaflet";
import L from "leaflet";
import { useFleetStore } from "../store/fleetStore";

// Fix Leaflet default icon paths broken by Vite bundling.
delete (L.Icon.Default.prototype as unknown as Record<string, unknown>)._getIconUrl;
L.Icon.Default.mergeOptions({
  iconRetinaUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png",
  iconUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png",
  shadowUrl: "https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png",
});

/** Build a coloured div-icon for a truck marker. */
function truckIcon(color: string, heading: number): L.DivIcon {
  return L.divIcon({
    className: "",
    html: `
      <div style="
        width:32px;height:32px;border-radius:50% 50% 50% 0;
        background:${color};border:2px solid #fff;
        transform:rotate(${heading - 45}deg);
        box-shadow:0 0 6px ${color};
      "></div>`,
    iconSize: [32, 32],
    iconAnchor: [16, 16],
  });
}

/** Re-centers the map on history route when entering history mode. */
function HistoryFit({ positions }: { positions: [number, number][] }) {
  const map = useMap();
  if (positions.length > 1) {
    map.fitBounds(positions);
  }
  return null;
}

export default function FleetMap() {
  const trucks = useFleetStore((s) => s.trucks);
  const selectTruck = useFleetStore((s) => s.selectTruck);
  const isLiveMode = useFleetStore((s) => s.isLiveMode);
  const historyPoints = useFleetStore((s) => s.historyPoints);
  const historyScrubIndex = useFleetStore((s) => s.historyScrubIndex);

  const historyPositions: [number, number][] = historyPoints.map((p) => [p.latitude, p.longitude]);
  const scrubPoint = historyPoints[historyScrubIndex];

  return (
    <MapContainer
      center={[-0.51, 116.83]}
      zoom={12}
      style={{ height: "100%", width: "100%", background: "#0d1117" }}
      zoomControl={false}
    >
      <TileLayer
        url="https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png"
        attribution='&copy; <a href="https://carto.com/">CARTO</a>'
      />

      {/* Live mode markers */}
      {isLiveMode &&
        Object.values(trucks).map(({ telemetry, isStale }) => {
          const isCritical =
            telemetry.speed_kmh > 60 || telemetry.engine_rpm > 2500;
          const color = isStale ? "#8b949e" : isCritical ? "#f85149" : "#3fb950";
          return (
            <Marker
              key={telemetry.fleet_id}
              position={[telemetry.latitude, telemetry.longitude]}
              icon={truckIcon(color, telemetry.heading_degrees)}
              eventHandlers={{ click: () => selectTruck(telemetry.fleet_id) }}
            >
              <Popup>
                <strong>{telemetry.fleet_id}</strong>
                <br />
                {telemetry.operational_state} | {telemetry.speed_kmh.toFixed(1)} km/h
              </Popup>
            </Marker>
          );
        })}

      {/* History mode polyline + scrub marker */}
      {!isLiveMode && historyPositions.length > 0 && (
        <>
          <HistoryFit positions={historyPositions} />
          <Polyline positions={historyPositions} color="#d29922" weight={2} />
          {scrubPoint && (
            <Marker
              position={[scrubPoint.latitude, scrubPoint.longitude]}
              icon={truckIcon("#d29922", 0)}
            >
              <Popup>
                {new Date(scrubPoint.timestamp).toLocaleTimeString()}
                <br />
                {scrubPoint.speed_kmh?.toFixed(1)} km/h | {scrubPoint.operational_state}
              </Popup>
            </Marker>
          )}
        </>
      )}
    </MapContainer>
  );
}
