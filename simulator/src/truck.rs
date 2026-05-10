use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use prost::Message;
use rand::Rng;
use rumqttc::{AsyncClient, QoS};
use tokio::sync::Mutex;
use tracing::info;

use crate::Telemetry;

// ── Mine area centred roughly in Kalimantan ───────────────────────────────────
const BASE_LAT: f64 = -0.51;
const BASE_LON: f64 = 116.83;

/// Truck operational state machine.
#[derive(Clone, Debug, PartialEq)]
pub enum TruckOp {
    Idle,
    Loading,
    Hauling,
    Dumping,
    Returning,
}

#[derive(Clone)]
pub struct Truck {
    pub call_sign: String,
    pub lat: f64,
    pub lon: f64,
    pub heading: i32,
    pub speed: f32,
    pub fuel: f32,
    pub payload: f32,
    pub rpm: i32,
    pub op_state: TruckOp,
    pub op_ticks: u32, // ticks spent in current state
    /// Chaos mode: glitch fires every GLITCH_INTERVAL ticks.
    pub tick: u64,
}

const GLITCH_INTERVAL: u64 = 60; // every ~60 seconds

pub fn init_trucks() -> Vec<Truck> {
    (1..=5)
        .map(|i| Truck {
            call_sign: format!("HT-{:03}", i),
            lat: BASE_LAT + (i as f64) * 0.01,
            lon: BASE_LON + (i as f64) * 0.005,
            heading: (i * 45) % 360,
            speed: 0.0,
            fuel: 80.0 + (i as f32) * 2.0,
            payload: 0.0,
            rpm: 800,
            op_state: TruckOp::Idle,
            op_ticks: 0,
            tick: 0,
        })
        .collect()
}

/// Main loop for a single truck – publishes every 1 second.
pub async fn run_truck(idx: usize, client: AsyncClient, trucks: Arc<Mutex<Vec<Truck>>>) {
    loop {
        let tel = {
            let mut locked = trucks.lock().await;
            let truck = &mut locked[idx];
            step_truck(truck)
        };

        publish(&client, &tel).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Burst mode: publish `count` messages without sleeping.
pub async fn burst(idx: usize, count: u32, client: AsyncClient, trucks: Arc<Mutex<Vec<Truck>>>) {
    info!("Burst: {} messages for truck idx={idx}", count);
    for _ in 0..count {
        let tel = {
            let mut locked = trucks.lock().await;
            let truck = &mut locked[idx];
            step_truck(truck)
        };
        publish(&client, &tel).await;
        // Minimal yield so Tokio can process acks.
        tokio::task::yield_now().await;
    }
}

/// Advance the truck state machine and return the telemetry payload.
fn step_truck(t: &mut Truck) -> Telemetry {
    let mut rng = rand::thread_rng();

    t.tick += 1;
    t.op_ticks += 1;

    // ── State transitions ─────────────────────────────────────────────────────
    let transition = match t.op_state {
        TruckOp::Idle if t.op_ticks > 5 => Some(TruckOp::Loading),
        TruckOp::Loading if t.op_ticks > 15 => Some(TruckOp::Hauling),
        TruckOp::Hauling if t.op_ticks > 30 => Some(TruckOp::Dumping),
        TruckOp::Dumping if t.op_ticks > 8 => Some(TruckOp::Returning),
        TruckOp::Returning if t.op_ticks > 20 => Some(TruckOp::Idle),
        _ => None,
    };
    if let Some(new_state) = transition {
        t.op_state = new_state;
        t.op_ticks = 0;
    }

    // ── Physical values ───────────────────────────────────────────────────────
    match t.op_state {
        TruckOp::Idle => {
            t.speed = 0.0;
            t.rpm = 750 + rng.gen_range(0..100);
            t.payload = 0.0;
        }
        TruckOp::Loading => {
            t.speed = 5.0 + rng.gen_range(0.0..3.0);
            t.rpm = 1200 + rng.gen_range(0..200);
            t.payload = (t.payload + 15.0).min(250.0);
        }
        TruckOp::Hauling => {
            t.speed = 30.0 + rng.gen_range(0.0..15.0);
            t.rpm = 1800 + rng.gen_range(0..400);
            t.payload = 250.0;
            t.heading = (t.heading + rng.gen_range(-5..5)).rem_euclid(360);
        }
        TruckOp::Dumping => {
            t.speed = 3.0;
            t.rpm = 900 + rng.gen_range(0..150);
            t.payload = (t.payload - 30.0).max(0.0);
        }
        TruckOp::Returning => {
            t.speed = 40.0 + rng.gen_range(0.0..10.0);
            t.rpm = 1600 + rng.gen_range(0..300);
            t.payload = 0.0;
            t.heading = (t.heading + 180).rem_euclid(360);
        }
    }

    // Consume fuel
    t.fuel = (t.fuel - 0.01).max(0.0);

    // Move position based on heading + speed (tiny delta for sim).
    let speed_deg_per_s = (t.speed as f64) / 111_000.0; // rough conversion
    t.lat += speed_deg_per_s * (t.heading as f64).to_radians().cos() * 0.01;
    t.lon += speed_deg_per_s * (t.heading as f64).to_radians().sin() * 0.01;

    let elevation = 150.0 + rng.gen_range(0.0f32..50.0);
    let ts_ms = Utc::now().timestamp_millis();

    // ── Chaos Mode 1: GPS Glitch (every GLITCH_INTERVAL ticks) ───────────────
    let (lat, lon) = if t.tick % GLITCH_INTERVAL == 0 {
        info!("[{}] CHAOS: GPS glitch injected", t.call_sign);
        // Jump 150 km away.
        (t.lat + 1.35, t.lon + 0.90)
    } else {
        (t.lat, t.lon)
    };

    // ── Chaos Mode 2: Out-of-order timestamp (every 90 ticks) ─────────────────
    let timestamp_ms = if t.tick % 90 == 0 {
        info!("[{}] CHAOS: Out-of-order timestamp injected", t.call_sign);
        ts_ms - 300_000 // 5 minutes in the past
    } else {
        ts_ms
    };

    let op_state_str = match t.op_state {
        TruckOp::Idle => "IDLE",
        TruckOp::Loading => "LOADING",
        TruckOp::Hauling => "HAULING",
        TruckOp::Dumping => "DUMPING",
        TruckOp::Returning => "RETURNING",
    };

    Telemetry {
        fleet_id: t.call_sign.clone(),
        timestamp_ms,
        latitude: lat,
        longitude: lon,
        elevation_meters: elevation,
        speed_kmh: t.speed,
        engine_rpm: t.rpm,
        fuel_level_percent: t.fuel,
        payload_weight_tons: t.payload,
        heading_degrees: t.heading,
        operational_state: op_state_str.to_string(),
        excavator_data: None,
    }
}

async fn publish(client: &AsyncClient, tel: &Telemetry) {
    let topic = format!("fleet/telemetry/{}", tel.fleet_id);
    let mut buf = Vec::new();
    tel.encode(&mut buf).expect("protobuf encode");

    if let Err(e) = client.publish(topic, QoS::AtMostOnce, false, buf).await {
        tracing::warn!("MQTT publish error: {e}");
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_truck(idx: usize) -> Truck {
        init_trucks().remove(idx)
    }

    #[test]
    fn test_idle_to_loading_transition() {
        let mut t = make_truck(0);
        assert_eq!(t.op_state, TruckOp::Idle);
        // Advance past idle threshold (5 ticks)
        for _ in 0..6 {
            step_truck(&mut t);
        }
        assert_eq!(t.op_state, TruckOp::Loading);
    }

    #[test]
    fn test_full_cycle() {
        let mut t = make_truck(1);
        // Run enough ticks to complete a full cycle.
        for _ in 0..100 {
            step_truck(&mut t);
        }
        // After ~80 ticks the truck should have gone through at least one returning phase.
        assert!(matches!(
            t.op_state,
            TruckOp::Idle | TruckOp::Loading | TruckOp::Hauling | TruckOp::Returning
        ));
    }

    #[test]
    fn test_gps_glitch_injected_at_interval() {
        let mut t = make_truck(0);
        // Advance to just before glitch tick.
        t.tick = GLITCH_INTERVAL - 1;
        let normal = step_truck(&mut t); // tick == GLITCH_INTERVAL now
        // The glitch fires on tick == GLITCH_INTERVAL (divisible by GLITCH_INTERVAL).
        let dist_lat = (normal.latitude - t.lat).abs();
        assert!(dist_lat > 1.0, "Expected glitch lat offset > 1 degree, got {dist_lat}");
    }

    #[test]
    fn test_out_of_order_timestamp_injected() {
        let mut t = make_truck(0);
        t.tick = 89; // next step will be tick==90
        let tel = step_truck(&mut t);
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Timestamp should be 5 minutes in the past.
        assert!(now_ms - tel.timestamp_ms > 290_000);
    }

    #[test]
    fn test_fuel_decreases() {
        let mut t = make_truck(0);
        let initial = t.fuel;
        for _ in 0..10 {
            step_truck(&mut t);
        }
        assert!(t.fuel < initial, "Fuel should decrease over time");
    }

    #[test]
    fn test_payload_loads_during_loading() {
        let mut t = make_truck(0);
        t.op_state = TruckOp::Loading;
        t.op_ticks = 0;
        t.payload = 0.0;
        for _ in 0..5 {
            step_truck(&mut t);
        }
        assert!(t.payload > 0.0);
    }
}
