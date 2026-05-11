//! HTTP interface — Axum route handlers.
//!
//! This layer only knows about HTTP (Axum) and the use-case structs.
//! It delegates all logic to `ManageFleetUseCase` and the SSE broadcast channel.
//! No SQL or MQTT code lives here.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, put},
    Router,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::domain::errors::AppError;
use crate::use_cases::manage_fleet::ManageFleetUseCase;

/// Shared state injected into all Axum handlers.
#[derive(Clone)]
pub struct HttpState {
    pub manage_fleet: Arc<ManageFleetUseCase>,
    pub sse_tx: broadcast::Sender<String>,
}

/// Build the Axum router with all routes wired.
pub fn build_router(state: HttpState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/stream", get(sse_stream))
        .route("/api/fleets", get(list_fleets))
        .route("/api/alerts", get(list_alerts))
        .route("/api/alerts/:id/acknowledge", put(acknowledge_alert))
        .route("/api/history", get(history))
        .route("/api/history/page", get(history_page))
        .with_state(Arc::new(state))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/health
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

/// GET /api/stream — SSE live telemetry feed.
async fn sse_stream(
    State(state): State<Arc<HttpState>>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(msg) => {
            // msg is "data: {...}\n\n" — strip framing; Axum adds it back.
            let data = msg
                .trim_start_matches("data: ")
                .trim_end_matches("\n\n")
                .to_string();
            Some(Ok(Event::default().data(data)))
        }
        Err(_) => None, // lagged — skip
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// GET /api/fleets
async fn list_fleets(State(state): State<Arc<HttpState>>) -> Result<impl IntoResponse, AppError> {
    let fleets = state.manage_fleet.list_fleets().await?;
    Ok(Json(fleets))
}

/// GET /api/alerts
async fn list_alerts(State(state): State<Arc<HttpState>>) -> Result<impl IntoResponse, AppError> {
    let alerts = state.manage_fleet.list_alerts().await?;
    Ok(Json(alerts))
}

/// PUT /api/alerts/:id/acknowledge
async fn acknowledge_alert(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    state.manage_fleet.acknowledge_alert(id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /api/history?truck_id=HT-001&limit=500
#[derive(Deserialize)]
struct HistoryParams {
    truck_id: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    500
}

async fn history(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<HistoryParams>,
) -> Result<impl IntoResponse, AppError> {
    let points = state
        .manage_fleet
        .get_history(&params.truck_id, params.limit)
        .await?;
    Ok(Json(points))
}

/// GET /api/history/page?truck_id=HT-001&page=0&page_size=100
#[derive(Deserialize)]
struct HistoryPageParams {
    truck_id: String,
    #[serde(default)]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
}

fn default_page_size() -> i64 {
    100
}

async fn history_page(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<HistoryPageParams>,
) -> Result<impl IntoResponse, AppError> {
    let page = state
        .manage_fleet
        .get_history_page(&params.truck_id, params.page, params.page_size)
        .await?;
    Ok(Json(page))
}
