# DATA CONTRACTS & PAYLOAD SCHEMAS

To handle high-frequency telemetry from millions of trucks efficiently, we use a hybrid serialization approach. 
1. **IoT to Backend (MQTT):** Protocol Buffers (Protobuf).
2. **Backend to Frontend (SSE/REST):** JSON.

## Trade-off & Defense (To document in README)
- **Why Protobuf for IoT?** Mining environments have constrained bandwidth. JSON is human-readable but bloated with repeated keys. Protobuf compiles to a compact binary format, saving significant network bandwidth and CPU parsing cycles on the edge.
- **Why JSON for Frontend?** Browsers natively parse JSON easily. The Rust backend will deserialize the Protobuf from MQTT, process it, and serialize it back to JSON for the SSE stream to the React app.
- **Schema Evolution Story (6-Month Plan):** Protobuf handles schema evolution gracefully. If we add a new sensor in 6 months (e.g., `tire_pressure`), we simply add `float tire_pressure = 13;` to the proto file. Older trucks still sending the old schema won't crash the backend (the new field will default to 0/empty). Older backends won't crash either; they simply ignore tag `13`. We strictly forbid reusing old tag numbers.

## 1. Protobuf Schema (`telemetry.proto`)
The Rust Simulator and Backend must compile this schema using `prost` or `tonic`.

```protobuf
syntax = "proto3";

package minifleet;

message Telemetry {
  // Required fields in practice, though proto3 makes everything optional
  string fleet_id = 1; 
  int64 timestamp_ms = 2; // Unix epoch milliseconds to avoid string parsing
  double latitude = 3;
  double longitude = 4;
  float elevation_meters = 5;
  float speed_kmh = 6;
  int32 engine_rpm = 7;
  float fuel_level_percent = 8;
  float payload_weight_tons = 9;
  int32 heading_degrees = 10;
  string operational_state = 11; // 'LOADING', 'HAULING', 'IDLE'

  // Optional extension (used only if fleet_type is EXCAVATOR)
  ExcavatorData excavator_data = 12;
}

message ExcavatorData {
  float boom_angle_degrees = 1;
  float arm_angle_degrees = 2;
  float bucket_angle_degrees = 3;
  float swing_speed_rpm = 4;
}

```

## SSE PAyload (Backend - Frontend)
```json
{
  "fleet_id": "HT-001",
  "timestamp": "2023-10-01T12:00:00Z", // Converted back to ISO 8601 for JS Date parsing
  "latitude": -6.200000,
  "longitude": 106.816666,
  "elevation_meters": 120.5,
  "speed_kmh": 45.5,
  "engine_rpm": 1800,
  "fuel_level_percent": 80.0,
  "payload_weight_tons": 250.0,
  "heading_degrees": 90,
  "operational_state": "HAULING",
  "is_anomaly": false,
  "anomaly_type": null
}
```

### In-Memory State struct (Backend Processor)
The backend must maintain a HashMap<String, TruckState> containing the last_timestamp_ms, last_lat, and last_lon to calculate jumps (GPS Glitch) and drop out-of-order messages.

