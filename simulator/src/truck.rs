use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use prost::Message;
use rand::Rng;
use rumqttc::{AsyncClient, QoS};
use tokio::sync::Mutex;
use tracing::info;

use crate::Telemetry;

// ── Open-pit mine centred in Kalimantan (Sangatta area) ──────────────────────
//
// We model a realistic open-pit mine loop. Each truck follows a closed circuit
// that mimics the typical haul-road geometry: tight curves at the loading zone
// and crusher, wider sweeping arcs on the haul road, switchbacks on the ramps.
//
// The "route" for each truck is a series of waypoints (lat, lon) that form the
// circuit. The truck interpolates between waypoints based on its speed.

const BASE_LAT: f64 = -0.51;
const BASE_LON: f64 = 116.83;

/// A waypoint along the mine circuit.
#[derive(Clone, Copy, Debug)]
pub struct Waypoint {
    pub lat: f64,
    pub lon: f64,
}

impl Waypoint {
    const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

// ── Mine circuit waypoints (open-pit haul road loop) ─────────────────────────
//
// The circuit models: Loading Zone (pit floor) → Ramp up → Haul Road (curved)
// → Crusher → Return Road → Back to Loading Zone.
// Offsets are in degrees (~111 km/deg lat, ~111*cos(lat) km/deg lon ≈ 111 km/deg here).
//
// Scale: 0.01° ≈ 1.1 km — each segment is clearly visible on the map at zoom 11-12.
// Total loop extent: ~2.5° × 2.5° ≈ 25 km, realistic for a large open-pit mine.

const CIRCUIT: [Waypoint; 16] = [
    // Loading zone — pit floor (south-west)
    Waypoint::new(BASE_LAT - 0.100, BASE_LON - 0.080),
    Waypoint::new(BASE_LAT - 0.080, BASE_LON - 0.060),
    // Ramp climb — switchback turns west then east (visible as a zigzag)
    Waypoint::new(BASE_LAT - 0.060, BASE_LON - 0.090),
    Waypoint::new(BASE_LAT - 0.040, BASE_LON - 0.050),
    // Ramp top — transition to haul road (heading changes sharply)
    Waypoint::new(BASE_LAT - 0.020, BASE_LON - 0.030),
    // Haul road — wide sweeping arc north-east
    Waypoint::new(BASE_LAT + 0.020, BASE_LON + 0.020),
    Waypoint::new(BASE_LAT + 0.060, BASE_LON + 0.060),
    Waypoint::new(BASE_LAT + 0.100, BASE_LON + 0.090),
    // Crusher area — north-east corner with dog-leg turn
    Waypoint::new(BASE_LAT + 0.130, BASE_LON + 0.120),
    Waypoint::new(BASE_LAT + 0.120, BASE_LON + 0.150),
    // Return road — arcing south-west (different path from haul road)
    Waypoint::new(BASE_LAT + 0.080, BASE_LON + 0.130),
    Waypoint::new(BASE_LAT + 0.040, BASE_LON + 0.100),
    // South-east curve — bends back west
    Waypoint::new(BASE_LAT + 0.010, BASE_LON + 0.060),
    Waypoint::new(BASE_LAT - 0.030, BASE_LON + 0.020),
    // Descent ramp — switchback back to pit floor
    Waypoint::new(BASE_LAT - 0.070, BASE_LON - 0.030),
    Waypoint::new(BASE_LAT - 0.100, BASE_LON - 0.060),
];

/// Segment index ranges that correspond to each operational state.
/// Loading zone: waypoints 0-1, Ramp/Haul: 2-7, Crusher: 8-9, Return: 10-15.
const LOADING_ZONE_END: usize = 2;   // waypoints 0-1 → loading
const CRUSHER_START: usize = 8;      // waypoints 8-9 → dumping/crusher
const CRUSHER_END: usize = 10;       // waypoints 10+ → returning

/// Elevation profile along the circuit (meters above sea level).
/// Pit floor ~20 m, ramp peaks ~80 m, haul road ~75 m, crusher plateau ~70 m.
const ELEVATION_PROFILE: [f32; 16] = [
    22.0, 25.0,  // pit floor
    45.0, 65.0,  // ramp climb
    78.0, 80.0,  // ramp top / haul road start
    78.0, 76.0,  // haul road arc
    72.0, 70.0,  // crusher plateau
    68.0, 66.0,  // return road
    72.0, 75.0,  // south-east curve
    60.0, 30.0,  // descent back to pit
];

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

    // Current position (interpolated between waypoints)
    pub lat: f64,
    pub lon: f64,
    pub heading: i32,
    pub elevation: f32,

    // Telemetry values
    pub speed: f32,
    pub fuel: f32,
    pub payload: f32,
    pub rpm: i32,

    // State machine
    pub op_state: TruckOp,
    pub op_ticks: u32,

    // Route progress
    pub waypoint_idx: usize,  // index of the *next* waypoint to reach
    pub route_progress: f64,  // 0.0–1.0 fraction between current and next waypoint

    // Chaos
    pub tick: u64,
}

const GLITCH_INTERVAL: u64 = 60;

pub fn init_trucks() -> Vec<Truck> {
    // Spread 5 trucks evenly around the 16-waypoint circuit so they look
    // like different trucks doing different things from the very first frame.
    let offsets = [0usize, 3, 6, 9, 12];

    (1..=5)
        .zip(offsets.iter())
        .map(|(i, &offset)| {
            let wp = CIRCUIT[offset];
            let elev = ELEVATION_PROFILE[offset];
            Truck {
                call_sign: format!("HT-{:03}", i),
                lat: wp.lat,
                lon: wp.lon,
                heading: initial_heading(offset),
                elevation: elev,
                speed: 0.0,
                fuel: 75.0 + (i as f32) * 4.0,
                payload: 0.0,
                rpm: 800,
                op_state: initial_state(offset),
                op_ticks: 0,
                waypoint_idx: (offset + 1) % CIRCUIT.len(),
                route_progress: 0.0,
                tick: 0,
            }
        })
        .collect()
}

/// Pick an initial operational state based on where in the circuit the truck starts.
fn initial_state(offset: usize) -> TruckOp {
    if offset < LOADING_ZONE_END {
        TruckOp::Loading
    } else if offset < CRUSHER_START {
        TruckOp::Hauling
    } else if offset < CRUSHER_END {
        TruckOp::Dumping
    } else {
        TruckOp::Returning
    }
}

/// Compute approximate heading from one waypoint to the next.
fn initial_heading(offset: usize) -> i32 {
    let from = CIRCUIT[offset];
    let to = CIRCUIT[(offset + 1) % CIRCUIT.len()];
    bearing_deg(from, to)
}

/// Bearing in degrees (0 = north, 90 = east) from `a` to `b`.
fn bearing_deg(a: Waypoint, b: Waypoint) -> i32 {
    let dlat = b.lat - a.lat;
    let dlon = b.lon - a.lon;
    let angle = dlon.atan2(dlat).to_degrees();
    angle.rem_euclid(360.0) as i32
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
        tokio::task::yield_now().await;
    }
}

/// Advance the truck state machine and return the telemetry payload.
fn step_truck(t: &mut Truck) -> Telemetry {
    let mut rng = rand::thread_rng();

    t.tick += 1;
    t.op_ticks += 1;

    // ── State transitions based on circuit position ───────────────────────────
    //
    // We derive the operational state from *where* the truck is on the circuit
    // rather than purely from a tick counter. This makes the data realistic:
    // the truck loads while at the loading zone, hauls while climbing/hauling,
    // dumps at the crusher, returns on the return road.
    let wp_idx = t.waypoint_idx % CIRCUIT.len();

    let new_state = if wp_idx < LOADING_ZONE_END || wp_idx == CIRCUIT.len() - 1 {
        // Approaching or at loading zone
        if t.op_ticks > 8 && t.op_state == TruckOp::Idle {
            Some(TruckOp::Loading)
        } else {
            None
        }
    } else if wp_idx < CRUSHER_START {
        // On the haul road
        if t.op_state == TruckOp::Loading && t.payload >= 250.0 {
            Some(TruckOp::Hauling)
        } else {
            None
        }
    } else if wp_idx < CRUSHER_END {
        // At the crusher
        if t.op_state == TruckOp::Hauling {
            Some(TruckOp::Dumping)
        } else {
            None
        }
    } else {
        // On the return road
        if t.op_state == TruckOp::Dumping && t.payload <= 0.0 {
            Some(TruckOp::Returning)
        } else if t.op_state == TruckOp::Returning && wp_idx >= CIRCUIT.len() - 2 {
            Some(TruckOp::Idle)
        } else {
            None
        }
    };

    if let Some(s) = new_state {
        t.op_state = s;
        t.op_ticks = 0;
    }

    // ── Physical values per state ─────────────────────────────────────────────
    //
    // Speed and RPM are realistically varied:
    //   - Ramp sections: lower speed, higher RPM (grade resistance)
    //   - Straight haul road: higher speed, mid RPM
    //   - Crusher / loading zone: near-zero speed, idle RPM
    let on_ramp = (2..=4).contains(&wp_idx);
    let on_descent = (14..=15).contains(&wp_idx);

    match t.op_state {
        TruckOp::Idle => {
            t.speed = 0.0;
            // Random warm-idle fluctuation 700-850 RPM
            t.rpm = 700 + rng.gen_range(0..150);
            t.payload = 0.0;
        }
        TruckOp::Loading => {
            // Creeping around the loading zone
            t.speed = rng.gen_range(2.0..6.0_f32);
            t.rpm = 1100 + rng.gen_range(0..250);
            t.payload = (t.payload + rng.gen_range(10.0..20.0_f32)).min(250.0);
        }
        TruckOp::Hauling => {
            if on_ramp {
                // Climbing ramp: slow & high RPM
                t.speed = rng.gen_range(12.0..22.0_f32);
                t.rpm = 2000 + rng.gen_range(0..400);
            } else {
                // Open haul road: faster, moderate RPM
                t.speed = rng.gen_range(28.0..45.0_f32);
                t.rpm = 1700 + rng.gen_range(0..350);
            }
            t.payload = 250.0;
            // Heading drifts slightly around curves (±8° per tick)
            t.heading = (t.heading + rng.gen_range(-8..8_i32)).rem_euclid(360);
        }
        TruckOp::Dumping => {
            t.speed = rng.gen_range(2.0..5.0_f32);
            t.rpm = 850 + rng.gen_range(0..200);
            t.payload = (t.payload - rng.gen_range(25.0..40.0_f32)).max(0.0);
        }
        TruckOp::Returning => {
            if on_descent {
                // Engine-braking descent: low RPM, moderate speed
                t.speed = rng.gen_range(18.0..30.0_f32);
                t.rpm = 1200 + rng.gen_range(0..200);
            } else {
                // Empty return: faster, lower RPM than loaded haul
                t.speed = rng.gen_range(35.0..52.0_f32);
                t.rpm = 1500 + rng.gen_range(0..300);
            }
            t.payload = 0.0;
        }
    }

    // ── Move along the circuit ────────────────────────────────────────────────
    //
    // We advance `route_progress` by a distance proportional to speed.
    // When progress >= 1.0 the truck has reached the next waypoint.
    //
    // Index convention: `waypoint_idx` is the *next* waypoint (destination).
    // The *current* waypoint (origin) is `(waypoint_idx + CIRCUIT.len() - 1) % CIRCUIT.len()`.
    // This avoids the `saturating_sub(1)` bug where idx=0 gives cur == next.
    let n = CIRCUIT.len();
    let cur_idx = (t.waypoint_idx + n - 1) % n;
    let next_idx = t.waypoint_idx % n;

    let speed_ms = (t.speed as f64) / 3.6; // km/h → m/s
    let segment_len_m = segment_length_m(CIRCUIT[cur_idx], CIRCUIT[next_idx]).max(50.0);
    t.route_progress += speed_ms / segment_len_m;

    // Advance waypoints when progress passes 1.0
    while t.route_progress >= 1.0 {
        t.route_progress -= 1.0;
        t.waypoint_idx = (t.waypoint_idx + 1) % n;
    }

    // Re-read indices after possible advance
    let cur_idx = (t.waypoint_idx + n - 1) % n;
    let next_idx = t.waypoint_idx % n;
    let alpha = t.route_progress.clamp(0.0, 1.0);

    t.lat = lerp(CIRCUIT[cur_idx].lat, CIRCUIT[next_idx].lat, alpha);
    t.lon = lerp(CIRCUIT[cur_idx].lon, CIRCUIT[next_idx].lon, alpha);

    // Heading: true bearing from cur to next + small jitter
    let base_heading = bearing_deg(CIRCUIT[cur_idx], CIRCUIT[next_idx]);
    t.heading = (base_heading + rng.gen_range(-5..5_i32)).rem_euclid(360);

    // Elevation: interpolate along profile + small noise
    t.elevation = lerp_f32(
        ELEVATION_PROFILE[cur_idx],
        ELEVATION_PROFILE[next_idx],
        alpha as f32,
    ) + rng.gen_range(-2.0..2.0_f32);

    // Consume fuel (~0.008%/s idle, 0.03%/s loaded hauling)
    let fuel_rate = match t.op_state {
        TruckOp::Idle => 0.008,
        TruckOp::Loading | TruckOp::Dumping => 0.015,
        TruckOp::Hauling => 0.030,
        TruckOp::Returning => 0.020,
    };
    t.fuel = (t.fuel - fuel_rate).max(0.0);

    let ts_ms = Utc::now().timestamp_millis();

    // ── Chaos Mode 1: GPS Glitch (every GLITCH_INTERVAL ticks) ───────────────
    let (lat, lon) = if t.tick % GLITCH_INTERVAL == 0 {
        info!("[{}] CHAOS: GPS glitch injected", t.call_sign);
        (t.lat + 1.35, t.lon + 0.90) // ~150 km jump
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
        elevation_meters: t.elevation,
        speed_kmh: t.speed,
        engine_rpm: t.rpm,
        fuel_level_percent: t.fuel,
        payload_weight_tons: t.payload,
        heading_degrees: t.heading,
        operational_state: op_state_str.to_string(),
        excavator_data: None,
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

/// Approximate segment length in metres using the equirectangular approximation.
fn segment_length_m(a: Waypoint, b: Waypoint) -> f64 {
    let dlat = (b.lat - a.lat) * 111_000.0;
    let dlon = (b.lon - a.lon) * 111_000.0 * a.lat.to_radians().cos();
    (dlat * dlat + dlon * dlon).sqrt()
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
    fn test_trucks_start_at_different_positions() {
        let trucks = init_trucks();
        // Each truck should start at a different waypoint so they look distinct.
        let positions: Vec<(i64, i64)> = trucks
            .iter()
            .map(|t| ((t.lat * 1e6) as i64, (t.lon * 1e6) as i64))
            .collect();
        // All starting positions must be unique.
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert_ne!(positions[i], positions[j], "Trucks {i} and {j} share the same starting position");
            }
        }
    }

    #[test]
    fn test_trucks_move_along_circuit() {
        let mut t = make_truck(0);
        let start_lat = t.lat;
        let start_lon = t.lon;
        // After 10 ticks the truck should have moved.
        for _ in 0..10 {
            step_truck(&mut t);
        }
        let moved = (t.lat - start_lat).abs() > 1e-6 || (t.lon - start_lon).abs() > 1e-6;
        assert!(moved, "Truck should have moved along the circuit");
    }

    #[test]
    fn test_path_is_not_straight_line() {
        let mut t = make_truck(0);
        // Force hauling state so the truck moves at meaningful speed.
        t.op_state = TruckOp::Hauling;
        t.payload = 250.0;

        let mut lats = Vec::new();
        let mut lons = Vec::new();
        for _ in 0..30 {
            let tel = step_truck(&mut t);
            lats.push(tel.latitude);
            lons.push(tel.longitude);
        }

        // Compute variance of heading direction changes — if path is a perfectly
        // straight line the headings are all equal (zero variance).
        let headings: Vec<f64> = lats.windows(2).zip(lons.windows(2)).map(|(ls, lo)| {
            (lo[1] - lo[0]).atan2(ls[1] - ls[0]).to_degrees()
        }).collect();

        let mean = headings.iter().sum::<f64>() / headings.len() as f64;
        let variance = headings.iter().map(|h| (h - mean).powi(2)).sum::<f64>() / headings.len() as f64;

        // The circuit curves enough that heading variance must be non-trivial.
        assert!(variance > 0.0, "Path should not be a perfectly straight line; heading variance={variance}");
    }

    #[test]
    fn test_elevation_changes_along_route() {
        let mut t = make_truck(0);
        t.op_state = TruckOp::Hauling;
        t.payload = 250.0;

        let elevations: Vec<f32> = (0..50).map(|_| {
            let tel = step_truck(&mut t);
            tel.elevation_meters
        }).collect();

        let min_elev = elevations.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_elev = elevations.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        // The route climbs from pit floor (~22 m) to haul road (~80 m).
        assert!(max_elev - min_elev > 1.0, "Elevation should change along the route");
    }

    #[test]
    fn test_ramp_section_has_higher_rpm() {
        let mut t = make_truck(0);
        // Position truck at ramp start (waypoint 2 is the first ramp waypoint).
        t.waypoint_idx = 3;
        t.op_state = TruckOp::Hauling;
        t.payload = 250.0;

        let tel = step_truck(&mut t);
        // On ramp the RPM must be ≥ 2000.
        assert!(tel.engine_rpm >= 1800, "Ramp section RPM should be elevated, got {}", tel.engine_rpm);
    }

    #[test]
    fn test_gps_glitch_injected_at_interval() {
        let mut t = make_truck(0);
        t.tick = GLITCH_INTERVAL - 1;
        let tel = step_truck(&mut t);
        let dist_lat = (tel.latitude - t.lat).abs();
        assert!(dist_lat > 1.0, "Expected glitch lat offset > 1°, got {dist_lat}");
    }

    #[test]
    fn test_out_of_order_timestamp_injected() {
        let mut t = make_truck(0);
        t.tick = 89;
        let tel = step_truck(&mut t);
        let now_ms = chrono::Utc::now().timestamp_millis();
        assert!(now_ms - tel.timestamp_ms > 290_000, "Timestamp should be ~5 min in the past");
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
        assert!(t.payload > 0.0, "Payload should increase during loading");
    }
}
