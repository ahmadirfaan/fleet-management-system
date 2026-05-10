export interface TelemetryEvent {
  fleet_id: string;
  timestamp: string;
  latitude: number;
  longitude: number;
  elevation_meters: number;
  speed_kmh: number;
  engine_rpm: number;
  fuel_level_percent: number;
  payload_weight_tons: number;
  heading_degrees: number;
  operational_state: string;
  is_anomaly: boolean;
  anomaly_type: string | null;
}

export interface Fleet {
  id: string;
  call_sign: string;
  fleet_type: string;
  make_model: string;
  status: string;
}

export interface Alert {
  id: string;
  call_sign: string;
  alert_type: string;
  severity: "WARNING" | "CRITICAL";
  start_timestamp: string;
  is_acknowledged: boolean;
  telemetry_snapshot: Record<string, number> | null;
}

export interface HistoryPoint {
  timestamp: string;
  latitude: number;
  longitude: number;
  speed_kmh: number | null;
  engine_rpm: number | null;
  fuel_level_percent: number | null;
  operational_state: string | null;
}
