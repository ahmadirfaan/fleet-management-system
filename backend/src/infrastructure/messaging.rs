//! Messaging adapter — MQTT listener (Mosquitto → mpsc channel).
//!
//! Decodes Protobuf payloads into domain `TelemetryReading` and pushes them
//! into the processor channel. Only this file knows about `rumqttc` and `prost`.

use anyhow::Result;
use prost::Message as ProstMessage;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tracing::{error, info, warn};

use crate::domain::models::{ExcavatorReading, TelemetryReading};

// Import the compiled protobuf types.
use crate::proto::Telemetry as ProtoTelemetry;

/// Entry-point: starts the MQTT listener with automatic reconnect.
pub async fn run_mqtt_listener(host: String, port: u16, tx: Sender<TelemetryReading>) {
    loop {
        if let Err(e) = connect_and_subscribe(host.clone(), port, tx.clone()).await {
            error!("MQTT connection error: {e}. Retrying in 3s...");
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
}

async fn connect_and_subscribe(
    host: String,
    port: u16,
    tx: Sender<TelemetryReading>,
) -> Result<()> {
    let mut opts = MqttOptions::new("fleet-backend", &host, port);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_session(true);

    let (client, mut eventloop) = AsyncClient::new(opts, 256);
    client.subscribe("fleet/telemetry/#", QoS::AtMostOnce).await?;
    info!("MQTT subscribed to fleet/telemetry/#");

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(msg))) => {
                match ProtoTelemetry::decode(msg.payload.as_ref()) {
                    Ok(proto) => {
                        let reading = proto_to_domain(proto);
                        if tx.send(reading).await.is_err() {
                            // Channel closed — processor has shut down.
                            return Ok(());
                        }
                    }
                    Err(e) => warn!("Protobuf decode error on {}: {e}", msg.topic),
                }
            }
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
    }
}

/// Convert the protobuf-generated struct into the domain model.
fn proto_to_domain(p: ProtoTelemetry) -> TelemetryReading {
    TelemetryReading {
        fleet_id: p.fleet_id,
        timestamp_ms: p.timestamp_ms,
        latitude: p.latitude,
        longitude: p.longitude,
        elevation_meters: p.elevation_meters,
        speed_kmh: p.speed_kmh,
        engine_rpm: p.engine_rpm,
        fuel_level_percent: p.fuel_level_percent,
        payload_weight_tons: p.payload_weight_tons,
        heading_degrees: p.heading_degrees,
        operational_state: p.operational_state,
        excavator_data: p.excavator_data.map(|ex| ExcavatorReading {
            boom_angle_degrees: ex.boom_angle_degrees,
            arm_angle_degrees: ex.arm_angle_degrees,
            bucket_angle_degrees: ex.bucket_angle_degrees,
            swing_speed_rpm: ex.swing_speed_rpm,
        }),
        is_anomaly: false, // ingest always starts as non-anomalous; use case may override
    }
}
