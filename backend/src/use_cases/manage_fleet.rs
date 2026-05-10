//! Use case: ManageFleet
//!
//! Encapsulates read-only fleet queries and the alert acknowledgement workflow.
//! No mutation of business rules happens here — this layer orchestrates the
//! repository calls and maps errors to domain errors.

use uuid::Uuid;

use crate::domain::{
    errors::{AppError, DomainError},
    models::{Fleet, HealthAlert, HistoryPoint},
};
use crate::infrastructure::persistence::FleetRepository;

pub struct ManageFleetUseCase {
    repo: FleetRepository,
}

impl ManageFleetUseCase {
    pub fn new(repo: FleetRepository) -> Self {
        Self { repo }
    }

    /// List all registered fleets.
    pub async fn list_fleets(&self) -> Result<Vec<Fleet>, AppError> {
        self.repo.get_fleets().await.map_err(AppError::Persistence)
    }

    /// List health alerts (most recent 100, all statuses).
    pub async fn list_alerts(&self) -> Result<Vec<HealthAlert>, AppError> {
        self.repo.get_alerts().await.map_err(AppError::Persistence)
    }

    /// Acknowledge a specific alert. Returns `DomainError::AlertNotFound` if
    /// the alert does not exist or was already acknowledged.
    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<(), AppError> {
        let updated = self
            .repo
            .acknowledge_alert(alert_id)
            .await
            .map_err(AppError::Persistence)?;

        if updated {
            Ok(())
        } else {
            Err(AppError::Domain(DomainError::AlertNotFound(
                alert_id.to_string(),
            )))
        }
    }

    /// Fetch historical telemetry for a single truck (latest-first, capped at `limit`).
    pub async fn get_history(
        &self,
        truck_id: &str,
        limit: i64,
    ) -> Result<Vec<HistoryPoint>, AppError> {
        self.repo
            .get_history(truck_id, limit)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => AppError::Domain(DomainError::UnknownFleet(
                    truck_id.to_string(),
                )),
                other => AppError::Persistence(other),
            })
    }
}
