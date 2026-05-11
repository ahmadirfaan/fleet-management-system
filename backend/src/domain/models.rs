use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Fleet master data ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub id: Uuid,
    pub call_sign: String,
    pub fleet_type: FleetType,
    pub make_model: String,
    pub status: FleetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FleetType {
    HaulTruck,
    Excavator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FleetStatus {
    Active,
    Maintenance,
    Breakdown,
}

// ── Telemetry reading ─────────────────────────────────────────────────────────

/// Canonical domain model for a single telemetry reading.
/// All infrastructure adapters (MQTT protobuf, DB rows) convert into this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryReading {
    pub fleet_id: String, // call_sign, resolved to UUID by persistence layer
    pub timestamp_ms: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_meters: f32,
    pub speed_kmh: f32,
    pub engine_rpm: i32,
    pub fuel_level_percent: f32,
    pub payload_weight_tons: f32,
    pub heading_degrees: i32,
    pub operational_state: String,
    pub excavator_data: Option<ExcavatorReading>,
    /// True when this reading was flagged as a GPS glitch or other anomaly at ingest time.
    /// The row is kept for audit purposes; consumers should exclude it from polyline rendering.
    #[serde(default)]
    pub is_anomaly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcavatorReading {
    pub boom_angle_degrees: f32,
    pub arm_angle_degrees: f32,
    pub bucket_angle_degrees: f32,
    pub swing_speed_rpm: f32,
}

// ── Alert ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub id: Uuid,
    pub call_sign: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub start_timestamp: DateTime<Utc>,
    pub is_acknowledged: bool,
    pub telemetry_snapshot: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertType {
    GpsGlitch,
    SustainedOverRev,
    Overspeed,
    OutOfOrder,
    FuelAnomaly,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::GpsGlitch => "GPS_GLITCH",
            AlertType::SustainedOverRev => "SUSTAINED_OVER_REV",
            AlertType::Overspeed => "OVERSPEED",
            AlertType::OutOfOrder => "OUT_OF_ORDER",
            AlertType::FuelAnomaly => "FUEL_ANOMALY",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertSeverity::Warning => "WARNING",
            AlertSeverity::Critical => "CRITICAL",
        }
    }
}

// ── History point ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub timestamp: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_meters: Option<f64>,
    pub speed_kmh: Option<f64>,
    pub engine_rpm: Option<i32>,
    pub fuel_level_percent: Option<f64>,
    pub payload_weight_tons: Option<f64>,
    pub operational_state: Option<String>,
    /// Mirrors the DB `is_anomaly` flag — true for GPS glitch rows.
    pub is_anomaly: bool,
}

// ── Paginated history response ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPage {
    pub truck_id: String,
    pub page: i64,
    pub page_size: i64,
    pub total_count: i64,
    pub points: Vec<HistoryPoint>,
}

// ── SSE event (broadcast to frontend) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SseEvent {
    pub fleet_id: String,
    pub timestamp: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_meters: f32,
    pub speed_kmh: f32,
    pub engine_rpm: i32,
    pub fuel_level_percent: f32,
    pub payload_weight_tons: f32,
    pub heading_degrees: i32,
    pub operational_state: String,
    pub is_anomaly: bool,
    pub anomaly_type: Option<String>,
}
