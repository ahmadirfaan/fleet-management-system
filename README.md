# FLEET MANAGEMENT SYSTEM — Real-Time Mine Fleet Management System

Single-command bring-up. Production-grade Rust backend. Live 2D map dashboard.

```bash
docker compose up --build
```

Then open **http://localhost:3000**.

---

## Services

| Service       | URL                        | Description                         |
|--------------|---------------------------|-------------------------------------|
| Frontend      | http://localhost:3000      | React command-center dashboard       |
| Backend API   | http://localhost:8080      | Axum REST + SSE server               |
| Sim Control   | http://localhost:9090      | Simulator burst endpoint             |
| MQTT Broker   | localhost:1883             | Eclipse Mosquitto                    |
| PostgreSQL    | localhost:5432             | PostGIS enabled                      |

---

## Architecture & Trade-offs

### Why `tokio::sync::mpsc` instead of Kafka/RabbitMQ?
Running JVM/Erlang-based brokers locally requires heavy RAM. For this MVP an in-memory channel perfectly mocks a message broker. At 10,000-truck scale, Component A (MQTT listener) and Component B (Processor worker) would be split into separate microservices communicating via Apache Kafka.

### Why SSE instead of WebSockets?
Telemetry tracking is strictly unidirectional (Server → Client). SSE is lighter, natively reconnects, and is natively supported by browsers. WebSockets would be chosen if the frontend needed to send commands back to the trucks.

### Why PostGIS instead of plain float columns?
PostGIS enables `ST_Contains` / `ST_Within` spatial queries directly in the database. This is critical for geofence detection (Loading Zone, Crusher) without complex math in Rust. Trade-off: slightly larger Docker image (`postgis/postgis`).

### Why Protobuf for IoT and JSON for the Frontend?
Mining environments have constrained bandwidth. Protobuf binary format saves significant bandwidth and CPU parsing cycles on the edge device. Browsers natively parse JSON, so the backend deserializes Protobuf from MQTT and re-serialises to JSON for the SSE stream.

### Schema Evolution (6-Month Plan)
Adding a new sensor (e.g. `tire_pressure`) only requires `float tire_pressure = 13;` in the `.proto` file. Old trucks continue working (new field defaults to 0). Old backends silently ignore unknown tag `13`. Tag numbers are never reused.

---

## Data Flow

```
Simulator (Rust) ──Protobuf──► Mosquitto MQTT
                                      │
                          Component A: MQTT Listener (Tokio task)
                                      │ mpsc channel
                          Component B: Processor Worker
                                ├── Validates GPS glitch, OOO timestamps
                                ├── Writes to PostgreSQL (PostGIS)
                                └── Broadcasts JSON to SSE hub
                                              │
                                      React Frontend
                                ├── Leaflet 2D map (marker interpolation)
                                ├── Left sidebar: fleet summary + alert queue
                                ├── Right drawer: vehicle drill-down
                                └── Bottom overlay: history time scrubber
```

---

## Simulator Chaos Modes

| Mode | Trigger | Backend Response |
|------|---------|-----------------|
| GPS Glitch | Every 60 ticks (1/min per truck) | Flags `GPS_GLITCH`, saves to DB, frontend marker does NOT move |
| Out-of-Order | Every 90 ticks | Saved to DB, dropped from SSE broadcast |
| Network Burst | `POST /sim/burst/:truck_id` | 500 instant messages; mpsc backpressure absorbs without crash |

---

## Running Tests

### Unit tests (Rust — no infra needed)
```bash
cd backend && cargo test
cd simulator && cargo test
```

### Frontend unit tests (Vitest)
```bash
cd frontend && npm ci && npm test
```

### Integration tests (requires `docker compose up` first)
```bash
pip install -r tests/requirements.txt
pytest tests/ -v
```

The integration suite contains **10 positive** and **10 negative** test cases covering:
- Backend health, fleet seeding, telemetry persistence
- SSE delivery, alert management, history API
- Simulator burst (backpressure), GPS glitch detection, out-of-order filtering
- Error cases: unknown truck, invalid UUID, missing params, double-acknowledge

---

## Project Structure

```
.
├── backend/           # Rust/Axum service (MQTT listener + processor + REST + SSE)
├── simulator/         # Rust simulator (5 trucks, 3 chaos modes, HTTP burst control)
├── frontend/          # React/Vite dashboard (Leaflet map, Zustand, Tailwind)
├── infra/             # init.sql (PostGIS schema + seed data), mosquitto.conf
├── tests/             # Python integration tests (pytest)
└── docker-compose.yml
```
