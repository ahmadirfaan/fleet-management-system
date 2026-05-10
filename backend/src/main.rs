//! Composition root — wires all layers together and starts the runtime.
//!
//! Nothing in main.rs contains business logic. It reads configuration,
//! constructs concrete implementations of each layer, and starts the
//! async tasks that keep the system running.

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod domain;
mod infrastructure;
mod interface;
mod use_cases;

// Compiled protobuf types — only infrastructure/messaging uses these.
mod proto {
    include!(concat!(env!("OUT_DIR"), "/minifleet.rs"));
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // ── Configuration from environment ────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://fleet:fleet_secret@localhost:5432/fleetdb".into());
    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".into())
        .parse()?;
    let server_port: u16 = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;

    // ── Layer 3: Infrastructure — database pool ───────────────────────────────
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    info!("Connected to PostgreSQL");

    let repo = infrastructure::persistence::FleetRepository::new(pool);

    // ── SSE broadcast channel (capacity 512 — lagged slow clients drop events) ─
    let (sse_tx, _) = broadcast::channel::<String>(512);

    // ── Layer 2: Use cases ────────────────────────────────────────────────────
    let track_use_case =
        use_cases::track_vehicle::TrackVehicleUseCase::new(repo.clone(), sse_tx.clone());
    let manage_use_case = Arc::new(use_cases::manage_fleet::ManageFleetUseCase::new(repo));

    // ── Processor worker: mpsc consumer → TrackVehicleUseCase ────────────────
    // Channel capacity 4096: absorbs burst traffic without dropping messages.
    let (proc_tx, mut proc_rx) =
        tokio::sync::mpsc::channel::<domain::models::TelemetryReading>(4096);

    tokio::spawn(async move {
        let mut uc = track_use_case;
        while let Some(reading) = proc_rx.recv().await {
            if let Err(e) = uc.execute(reading).await {
                tracing::error!("TrackVehicle error: {e}");
            }
        }
    });

    // ── Layer 3: Infrastructure — MQTT listener ───────────────────────────────
    tokio::spawn(infrastructure::messaging::run_mqtt_listener(
        mqtt_host,
        mqtt_port,
        proc_tx,
    ));

    // ── Layer 4: Interface — HTTP server ──────────────────────────────────────
    let http_state = interface::http::HttpState {
        manage_fleet: manage_use_case,
        sse_tx,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = interface::http::build_router(http_state).layer(cors);

    let addr = format!("0.0.0.0:{server_port}");
    info!("Listening on {addr}");
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
