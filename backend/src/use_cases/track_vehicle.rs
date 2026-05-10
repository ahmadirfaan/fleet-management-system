//! Use case: TrackVehicle
//!
//! Consumes a raw `TelemetryReading`, applies business rules (GPS glitch,
//! out-of-order, over-rev, overspeed), persists to the repository, and
//! publishes an `SseEvent` to the broadcast channel.
//!
//! This layer has NO dependency on Axum, SQLx, or MQTT — only on the
//! repository trait defined below and the domain models.

use std::collections::HashMap;

use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::domain::{
    errors::{AppError, DomainError},
    models::{AlertSeverity, AlertType, SseEvent, TelemetryReading},
};
use crate::infrastructure::persistence::FleetRepository;

// ── Business rule constants ───────────────────────────────────────────────────

/// Implied speed (km/h) above which a position jump is a GPS glitch.
const GPS_GLITCH_SPEED_KMH: f64 = 200.0;
/// Raw distance (km) above which a jump is always a glitch regardless of speed.
const GPS_GLITCH_DIST_KM: f64 = 50.0;
/// Engine RPM threshold for sustained over-rev alert.
const OVER_REV_RPM: i32 = 2500;
/// Speed threshold for overspeed alert (km/h).
const OVERSPEED_KMH: f32 = 60.0;

// ── In-memory state per truck ─────────────────────────────────────────────────

struct TruckState {
    last_timestamp_ms: i64,
    last_lat: f64,
    last_lon: f64,
}

// ── Use case struct ───────────────────────────────────────────────────────────

pub struct TrackVehicleUseCase {
    repo: FleetRepository,
    sse_tx: broadcast::Sender<String>,
    /// Per-truck in-memory state for anomaly detection (owned by this use case).
    truck_states: HashMap<String, TruckState>,
}

impl TrackVehicleUseCase {
    pub fn new(repo: FleetRepository, sse_tx: broadcast::Sender<String>) -> Self {
        Self {
            repo,
            sse_tx,
            truck_states: HashMap::new(),
        }
    }

    /// Process one telemetry reading end-to-end.
    pub async fn execute(&mut self, tel: TelemetryReading) -> Result<(), AppError> {
        let fleet_id = &tel.fleet_id;

        // ── Rule 1: Out-of-order detection ───────────────────────────────────
        if let Some(prev) = self.truck_states.get(fleet_id) {
            if tel.timestamp_ms <= prev.last_timestamp_ms {
                warn!(
                    "[{fleet_id}] Out-of-order (ts={}) — saving to DB, skipping SSE",
                    tel.timestamp_ms
                );
                self.repo.insert_telemetry(&tel).await?;
                self.repo
                    .insert_alert(fleet_id, AlertType::OutOfOrder, AlertSeverity::Warning, &tel)
                    .await?;
                return Ok(());
            }
        }

        // ── Rule 2: GPS Glitch detection ──────────────────────────────────────
        if let Some(prev) = self.truck_states.get(fleet_id) {
            let dist = haversine_km(prev.last_lat, prev.last_lon, tel.latitude, tel.longitude);
            let dt_h = (tel.timestamp_ms - prev.last_timestamp_ms) as f64 / 3_600_000.0;
            let implied_speed = if dt_h > 0.0 { dist / dt_h } else { f64::MAX };

            if implied_speed > GPS_GLITCH_SPEED_KMH || dist > GPS_GLITCH_DIST_KM {
                warn!(
                    "[{fleet_id}] GPS glitch (dist={dist:.1}km, implied={implied_speed:.0}km/h)"
                );
                self.repo.insert_telemetry(&tel).await?;
                self.repo
                    .insert_alert(fleet_id, AlertType::GpsGlitch, AlertSeverity::Critical, &tel)
                    .await?;
                self.broadcast(&tel, true, Some(AlertType::GpsGlitch));
                // Position NOT updated in memory.
                return Ok(());
            }
        }

        // ── Rule 3: Health classification ─────────────────────────────────────
        let anomaly: Option<AlertType> = if tel.engine_rpm > OVER_REV_RPM {
            info!("[{fleet_id}] Over-rev: {} RPM", tel.engine_rpm);
            self.repo
                .insert_alert(
                    fleet_id,
                    AlertType::SustainedOverRev,
                    AlertSeverity::Critical,
                    &tel,
                )
                .await?;
            Some(AlertType::SustainedOverRev)
        } else if tel.speed_kmh > OVERSPEED_KMH {
            info!("[{fleet_id}] Overspeed: {} km/h", tel.speed_kmh);
            self.repo
                .insert_alert(fleet_id, AlertType::Overspeed, AlertSeverity::Warning, &tel)
                .await?;
            Some(AlertType::Overspeed)
        } else {
            None
        };

        // ── Rule 4: Persist valid telemetry ───────────────────────────────────
        self.repo.insert_telemetry(&tel).await?;

        // ── Rule 5: Update in-memory position ─────────────────────────────────
        self.truck_states.insert(
            fleet_id.clone(),
            TruckState {
                last_timestamp_ms: tel.timestamp_ms,
                last_lat: tel.latitude,
                last_lon: tel.longitude,
            },
        );

        // ── Rule 6: Broadcast via SSE ──────────────────────────────────────────
        let is_anomaly = anomaly.is_some();
        self.broadcast(&tel, is_anomaly, anomaly);

        Ok(())
    }

    fn broadcast(&self, tel: &TelemetryReading, is_anomaly: bool, anomaly: Option<AlertType>) {
        let ts = chrono::DateTime::from_timestamp_millis(tel.timestamp_ms)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        let event = SseEvent {
            fleet_id: tel.fleet_id.clone(),
            timestamp: ts,
            latitude: tel.latitude,
            longitude: tel.longitude,
            elevation_meters: tel.elevation_meters,
            speed_kmh: tel.speed_kmh,
            engine_rpm: tel.engine_rpm,
            fuel_level_percent: tel.fuel_level_percent,
            payload_weight_tons: tel.payload_weight_tons,
            heading_degrees: tel.heading_degrees,
            operational_state: tel.operational_state.clone(),
            is_anomaly,
            anomaly_type: anomaly.map(|a| a.as_str().to_string()),
        };

        if let Ok(json) = serde_json::to_string(&event) {
            let _ = self.sse_tx.send(format!("data: {json}\n\n"));
        }
    }
}

// ── Pure domain function: Haversine distance ──────────────────────────────────

/// Returns the great-circle distance in kilometres between two (lat, lon) pairs.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_jakarta_bandung() {
        // Jakarta → Bandung ≈ 119 km
        let d = haversine_km(-6.2, 106.8, -6.9, 107.6);
        assert!((d - 119.0).abs() < 5.0, "got {d}");
    }

    #[test]
    fn haversine_same_point_is_zero() {
        let d = haversine_km(-6.2, 106.8, -6.2, 106.8);
        assert!(d < 0.001, "expected ~0, got {d}");
    }

    #[test]
    fn haversine_antipodal_is_half_circumference() {
        // Roughly half Earth circumference ≈ 20_015 km
        let d = haversine_km(0.0, 0.0, 0.0, 180.0);
        assert!((d - 20_015.0).abs() < 10.0, "got {d}");
    }

    #[test]
    fn gps_glitch_speed_threshold_constant() {
        // Implied speed for a 150km jump in 1 second should exceed the threshold.
        let dist = 150.0_f64;
        let dt_h = 1.0 / 3600.0;
        let implied = dist / dt_h;
        assert!(implied > GPS_GLITCH_SPEED_KMH);
    }
}
