use anyhow::Result;
use axum::{routing::post, Router};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod truck;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/minifleet.rs"));
}

pub use proto::Telemetry;

/// Shared MQTT client passed to trucks and the control HTTP server.
#[derive(Clone)]
struct SimState {
    client: AsyncClient,
    /// Trucks indexed by call_sign for burst control.
    trucks: Arc<Mutex<Vec<truck::Truck>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".into())
        .parse()?;

    // Retry loop – simulator starts before broker is ready.
    let (client, eventloop) = loop {
        let mut opts = MqttOptions::new("fleet-simulator", &mqtt_host, mqtt_port);
        opts.set_keep_alive(Duration::from_secs(30));
        opts.set_clean_session(true);
        opts.set_inflight(100);

        let (client, eventloop) = rumqttc::AsyncClient::new(opts, 512);
        // Probe the broker.
        if client
            .publish("fleet/ping", QoS::AtMostOnce, false, b"ping".to_vec())
            .await
            .is_ok()
        {
            break (client, eventloop);
        }
        tracing::warn!("MQTT not ready, retrying in 2s...");
        tokio::time::sleep(Duration::from_secs(2)).await;
    };

    info!("Connected to MQTT broker at {mqtt_host}:{mqtt_port}");

    // Drain the MQTT eventloop in a background task.
    tokio::spawn(async move {
        let mut el = eventloop;
        loop {
            let _ = el.poll().await;
        }
    });

    // Initialise 5 haul trucks.
    let trucks = truck::init_trucks();
    let truck_count = trucks.len();
    let shared_trucks = Arc::new(Mutex::new(trucks));

    // Spawn a task per truck that publishes telemetry every second.
    for i in 0..truck_count {
        let client_clone = client.clone();
        let trucks_clone = Arc::clone(&shared_trucks);
        tokio::spawn(async move {
            truck::run_truck(i, client_clone, trucks_clone).await;
        });
    }

    // ── Control server for burst trigger ─────────────────────────────────────
    let state = SimState {
        client: client.clone(),
        trucks: Arc::clone(&shared_trucks),
    };

    let app = Router::new()
        .route("/sim/burst/:truck_id", post(burst_handler))
        .with_state(state);

    info!("Simulator control server on :9090");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// POST /sim/burst/:truck_id  – triggers 500 messages immediately for named truck
async fn burst_handler(
    axum::extract::Path(truck_id): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SimState>,
) -> impl axum::response::IntoResponse {
    let trucks = state.trucks.lock().await;
    let idx = trucks.iter().position(|t| t.call_sign == truck_id);
    drop(trucks);

    if let Some(i) = idx {
        let client = state.client.clone();
        let trucks_clone = Arc::clone(&state.trucks);
        tokio::spawn(async move {
            truck::burst(i, 500, client, trucks_clone).await;
        });
        (axum::http::StatusCode::OK, format!("Burst triggered for {truck_id}"))
    } else {
        (axum::http::StatusCode::NOT_FOUND, format!("Truck {truck_id} not found"))
    }
}
