//! Persistence adapter — wraps all SQLx queries behind a typed repository.

use chrono::DateTime;
use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    models::{AlertSeverity, AlertType, Fleet, FleetStatus, FleetType, HealthAlert, HistoryPoint, TelemetryReading},
};

/// Repository: all DB operations for the fleet management domain.
#[derive(Clone)]
pub struct FleetRepository {
    pool: PgPool,
}

impl FleetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Write operations ──────────────────────────────────────────────────────

    pub async fn insert_telemetry(&self, tel: &TelemetryReading) -> Result<(), sqlx::Error> {
        let fleet_uuid = self.resolve_fleet_uuid(&tel.fleet_id).await?;
        let ts = DateTime::from_timestamp_millis(tel.timestamp_ms).ok_or_else(|| {
            sqlx::Error::Decode(
                anyhow::anyhow!(DomainError::InvalidTimestamp(tel.timestamp_ms)).into(),
            )
        })?;

        sqlx::query(
            r#"
            INSERT INTO telemetry_logs
                (fleet_id, timestamp, geom, elevation_meters, speed_kmh, engine_rpm,
                 fuel_level_percent, payload_weight_tons, heading_degrees, operational_state)
            VALUES
                ($1, $2, ST_SetSRID(ST_MakePoint($3, $4), 4326),
                 $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(fleet_uuid)
        .bind(ts)
        .bind(tel.longitude)
        .bind(tel.latitude)
        .bind(tel.elevation_meters as f64)
        .bind(tel.speed_kmh as f64)
        .bind(tel.engine_rpm)
        .bind(tel.fuel_level_percent as f64)
        .bind(tel.payload_weight_tons as f64)
        .bind(tel.heading_degrees)
        .bind(&tel.operational_state)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_alert(
        &self,
        fleet_id_str: &str,
        alert_type: AlertType,
        severity: AlertSeverity,
        tel: &TelemetryReading,
    ) -> Result<(), sqlx::Error> {
        let fleet_uuid = self.resolve_fleet_uuid(fleet_id_str).await?;
        let ts = DateTime::from_timestamp_millis(tel.timestamp_ms).ok_or_else(|| {
            sqlx::Error::Decode(
                anyhow::anyhow!(DomainError::InvalidTimestamp(tel.timestamp_ms)).into(),
            )
        })?;

        let snapshot = json!({
            "speed_kmh": tel.speed_kmh,
            "engine_rpm": tel.engine_rpm,
            "latitude": tel.latitude,
            "longitude": tel.longitude,
            "fuel_level_percent": tel.fuel_level_percent,
        });

        sqlx::query(
            r#"
            INSERT INTO health_alerts
                (fleet_id, alert_type, severity, start_timestamp, telemetry_snapshot)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(fleet_uuid)
        .bind(alert_type.as_str())
        .bind(severity.as_str())
        .bind(ts)
        .bind(snapshot)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE health_alerts SET is_acknowledged = TRUE \
             WHERE id = $1 AND is_acknowledged = FALSE",
        )
        .bind(alert_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── Read operations ───────────────────────────────────────────────────────

    pub async fn get_fleets(&self) -> Result<Vec<Fleet>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, call_sign, fleet_type, make_model, status FROM fleets",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Fleet {
                id: r.get("id"),
                call_sign: r.get("call_sign"),
                fleet_type: parse_fleet_type(r.get("fleet_type")),
                make_model: r.get("make_model"),
                status: parse_fleet_status(r.get("status")),
            })
            .collect())
    }

    pub async fn get_alerts(&self) -> Result<Vec<HealthAlert>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT ha.id, f.call_sign, ha.alert_type, ha.severity,
                   ha.start_timestamp, ha.is_acknowledged, ha.telemetry_snapshot
            FROM health_alerts ha
            JOIN fleets f ON f.id = ha.fleet_id
            ORDER BY ha.start_timestamp DESC
            LIMIT 100
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| HealthAlert {
                id: r.get("id"),
                call_sign: r.get("call_sign"),
                alert_type: parse_alert_type(r.get("alert_type")),
                severity: parse_alert_severity(r.get("severity")),
                start_timestamp: r.get("start_timestamp"),
                is_acknowledged: r.get("is_acknowledged"),
                telemetry_snapshot: r.get("telemetry_snapshot"),
            })
            .collect())
    }

    pub async fn get_history(
        &self,
        fleet_id_str: &str,
        limit: i64,
    ) -> Result<Vec<HistoryPoint>, sqlx::Error> {
        let fleet_uuid = self.resolve_fleet_uuid(fleet_id_str).await?;

        let rows = sqlx::query(
            r#"
            SELECT
                timestamp,
                ST_Y(geom) AS latitude,
                ST_X(geom) AS longitude,
                speed_kmh,
                engine_rpm,
                fuel_level_percent,
                operational_state
            FROM telemetry_logs
            WHERE fleet_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(fleet_uuid)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| HistoryPoint {
                timestamp: r.get("timestamp"),
                latitude: r.get::<Option<f64>, _>("latitude").unwrap_or(0.0),
                longitude: r.get::<Option<f64>, _>("longitude").unwrap_or(0.0),
                speed_kmh: r.get("speed_kmh"),
                engine_rpm: r.get("engine_rpm"),
                fuel_level_percent: r.get("fuel_level_percent"),
                operational_state: r.get("operational_state"),
            })
            .collect())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    async fn resolve_fleet_uuid(&self, call_sign: &str) -> Result<Uuid, sqlx::Error> {
        let row = sqlx::query("SELECT id FROM fleets WHERE call_sign = $1")
            .bind(call_sign)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        Ok(row.get("id"))
    }
}

// ── String → enum parsers ─────────────────────────────────────────────────────

fn parse_fleet_type(s: &str) -> FleetType {
    match s {
        "EXCAVATOR" => FleetType::Excavator,
        _ => FleetType::HaulTruck,
    }
}

fn parse_fleet_status(s: &str) -> FleetStatus {
    match s {
        "MAINTENANCE" => FleetStatus::Maintenance,
        "BREAKDOWN" => FleetStatus::Breakdown,
        _ => FleetStatus::Active,
    }
}

fn parse_alert_type(s: &str) -> AlertType {
    match s {
        "GPS_GLITCH" => AlertType::GpsGlitch,
        "SUSTAINED_OVER_REV" => AlertType::SustainedOverRev,
        "OVERSPEED" => AlertType::Overspeed,
        "OUT_OF_ORDER" => AlertType::OutOfOrder,
        _ => AlertType::FuelAnomaly,
    }
}

fn parse_alert_severity(s: &str) -> AlertSeverity {
    match s {
        "CRITICAL" => AlertSeverity::Critical,
        _ => AlertSeverity::Warning,
    }
}
