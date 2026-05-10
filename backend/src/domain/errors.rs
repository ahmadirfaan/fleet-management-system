use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Unknown fleet: {0}")]
    UnknownFleet(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(i64),

    #[error("Alert not found or already acknowledged: {0}")]
    AlertNotFound(String),
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Persistence error: {0}")]
    Persistence(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

// Allow axum handlers to return AppError as HTTP responses.
impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        let (status, msg) = match &self {
            AppError::Domain(DomainError::UnknownFleet(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            AppError::Domain(DomainError::AlertNotFound(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            AppError::Domain(DomainError::InvalidTimestamp(_)) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, msg).into_response()
    }
}
