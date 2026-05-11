# Mine Fleet Live Tracker

Real-time telemetry dashboard for five haul trucks cycling through an open-pit mine. Backend in Rust, frontend in React, single-command bring-up.

```bash
docker compose up --build
```

Open **http://localhost:3000** — the map will populate within a few seconds as the simulator starts publishing.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [How to Run](#2-how-to-run)
3. [Architecture Overview](#3-architecture-overview)
4. [Services & Ports](#4-services--ports)
5. [Dashboard Features](#5-dashboard-features)
6. [Simulator Behaviour](#6-simulator-behaviour)
7. [Health Classification](#7-health-classification)
8. [API Reference](#8-api-reference)
9. [Running Tests](#9-running-tests)
10. [Stack Decisions & Rationale](#10-stack-decisions--rationale)
11. [AI Usage Log](#11-ai-usage-log)
12. [What I Would Do Next](#12-what-i-would-do-next)

---

## 1. Prerequisites

| Tool | Minimum version | Purpose |
|---|---|---|
| Docker Desktop (or Docker Engine + Compose plugin) | 24.x / Compose 2.x | Single-command bring-up |
| — | — | Everything else runs inside containers |

**Optional — for running tests or developing locally without Docker:**

| Tool | Version used | Purpose |
|---|---|---|
| Rust + Cargo | 1.75+ | Backend & simulator |
| Node.js + npm | 20 LTS | Frontend |
| Python 3 | 3.11+ | Integration test suite |
| `protobuf-compiler` (`protoc`) | 3.x | Backend build (proto → Rust) |
| PostgreSQL client (`psql`) | any | Manual DB inspection |

> **macOS (Homebrew):**
> ```bash
> brew install docker docker-compose rustup-init node python3
> rustup-init -y
> ```
>
> **Ubuntu / Debian:**
> ```bash
> sudo apt-get install -y docker.io docker-compose-plugin rustup nodejs npm python3 python3-pip protobuf-compiler
> ```

---

## 2. How to Run

### 2a. Docker (recommended — everything included)

```bash
# Clone the repository
git clone <repo-url>
cd mini-fleet-management-system

# Build images and start all five services
docker compose up --build
```

The stack is health-checked and starts in dependency order:

```
postgres → mosquitto → backend → simulator → frontend
```

Wait for the log line:
```
fleet_backend  | Listening on 0.0.0.0:8080
```

Then open **http://localhost:3000**.

**Stop everything:**
```bash
docker compose down
```

**Stop and wipe the database:**
```bash
docker compose down -v
```

---

### 2b. Local development (without Docker)

You need PostgreSQL with the PostGIS extension and a running Mosquitto broker. The easiest path is to start only the infra containers:

```bash
docker compose up postgres mosquitto -d
```

Then run each service in its own terminal:

**Terminal 1 — Backend:**
```bash
cd backend
export DATABASE_URL="postgres://fleet:fleet_secret@localhost:5432/fleetdb"
export MQTT_HOST=localhost
export MQTT_PORT=1883
export SERVER_PORT=8080
export RUST_LOG=info
cargo run --release
```

**Terminal 2 — Simulator:**
```bash
cd simulator
export MQTT_HOST=localhost
export MQTT_PORT=1883
export RUST_LOG=info
cargo run --release
```

**Terminal 3 — Frontend:**
```bash
cd frontend
npm ci
VITE_API_BASE_URL=http://localhost:8080 npm run dev
```

Frontend dev server runs at **http://localhost:5173**.

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Simulator (Rust)                                               │
│  5 Tokio tasks — one per truck                                  │
│  Open-pit mine circuit: 16 waypoints, realistic physics         │
│  Publishes Protobuf over MQTT @ 1 msg/truck/sec                 │
└───────────────────┬─────────────────────────────────────────────┘
                    │ MQTT topic: fleet/telemetry/{call_sign}
                    │ Eclipse Mosquitto (port 1883)
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Backend (Rust / Axum)                                          │
│                                                                 │
│  Component A: MQTT Listener (single Tokio task)                 │
│    └── Decodes Protobuf → TelemetryReading                      │
│    └── Sends into mpsc channel (capacity 4096)                  │
│                                                                 │
│  Component B: Processor Worker (single Tokio task)              │
│    ├── Rejects out-of-order timestamps                          │
│    ├── Detects GPS glitches (haversine > 50 km or > 200 km/h)  │
│    ├── Classifies health alerts (over-rev, overspeed, fuel)     │
│    ├── Writes to PostgreSQL (PostGIS geometry)                  │
│    └── Broadcasts JSON over SSE broadcast channel (cap 512)     │
│                                                                 │
│  HTTP layer (Axum)                                              │
│    ├── GET  /api/stream          — SSE live feed                │
│    ├── GET  /api/fleets          — fleet list                   │
│    ├── GET  /api/alerts          — health alerts                │
│    ├── PUT  /api/alerts/:id/acknowledge                         │
│    ├── GET  /api/history         — per-truck telemetry history  │
│    └── GET  /api/history/page    — paginated history table      │
└───────────────────┬─────────────────────────────────────────────┘
                    │ SSE (JSON)
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (React 18 + TypeScript / Vite)                        │
│                                                                 │
│  useSseStream hook — EventSource → Zustand store                │
│  FleetMap (Leaflet)                                             │
│    ├── Live mode: 5 animated truck markers + per-truck trails   │
│    └── History mode: ghost route + scrubbed elapsed path        │
│  Sidebar — fleet status list + active alerts + ack controls     │
│  DrillDown drawer — 6-stat telemetry grid per selected truck    │
│  HistoryScrubber — SVG elevation chart + timeline scrubber      │
│  HistoryTable — paginated fleet list → per-truck data table     │
└─────────────────────────────────────────────────────────────────┘
```

**Data flows strictly one way: Simulator → MQTT → Backend → SSE → Frontend.**  
The frontend never writes to the database; it only reads via REST endpoints.

---

## 4. Services & Ports

| Service | Container | Host port | Description |
|---|---|---|---|
| Frontend | `fleet_frontend` | **3000** | React dashboard (nginx) |
| Backend API | `fleet_backend` | **8080** | Axum REST + SSE |
| Simulator control | `fleet_simulator` | **9090** | Burst endpoint only |
| MQTT broker | `fleet_mqtt` | **1883** | Eclipse Mosquitto |
| PostgreSQL/PostGIS | `fleet_postgres` | **5432** | Telemetry + alert storage |

---

## 5. Dashboard Features

### Live Fleet Map

- Dark CARTO tile map centred on the simulated mine in Kalimantan
- Animated truck SVG markers with heading rotation — colour-coded:
  - **Green**: normal operation
  - **Red**: critical condition (speed > 60 km/h or RPM > 2500)
  - **Gray**: stale (no data for > 30 seconds)
- Per-truck **live trail polyline** — the last 200 GPS positions streamed via SSE, drawn in a distinct colour per truck (blue / green / yellow / orange / purple). GPS-glitch positions are excluded from the trail
- Click any marker to open the **DrillDown drawer** (speed, RPM, payload, fuel, elevation, heading + anomaly badge)

### History Mode

Activated from the DrillDown drawer ("View History") or from an alert's "History" button.

- **Full ghost route** of last 500 telemetry points (dim, full extent)
- **Elapsed path** from start up to the scrub cursor (bright yellow)
- **SVG elevation chart** — click anywhere on the chart to jump the cursor; area fill + crosshair marker show current elevation
- **Timeline scrubber** with timestamps at both ends
- Stats at the scrub point: speed, state, RPM, elevation, fuel

### Fleet History Table

Opened via **Fleet History Table** button at the bottom of the left sidebar.

- **Level 1 — Fleet list**: all trucks with type, status, total record count, last-seen timestamp; paginated at 10 trucks per page
- **Level 2 — Truck detail**: full telemetry table with timestamp, operational state (colour-coded), speed, RPM, elevation, fuel, payload; paginated at 100 records per page via the `/api/history/page` backend endpoint

### Sidebar

- Fleet status list (click any truck to open DrillDown)
- Active alerts with severity badge, timestamp, Acknowledge and History buttons
- Acknowledged alert history (last 10)
- Collapsible — collapses to icon-only view with alert count badge

---

## 6. Simulator Behaviour

The simulator models **five haul trucks** following a closed 16-waypoint open-pit mine circuit:

```
Pit floor (loading zone) → Ramp switchback (climb) → Open haul road (arc)
→ Crusher plateau (dump) → Return road (arc) → Descent ramp → Pit floor
```

Each truck starts at a different point on the circuit so they look distinct from frame one.

### Operational States

| State | Speed range | RPM range | Notes |
|---|---|---|---|
| IDLE | 0 km/h | 700–850 | Waiting at loading zone |
| LOADING | 2–6 km/h | 1100–1350 | Creeping under excavator; payload +10–20 t/tick |
| HAULING (ramp) | 12–22 km/h | 2000–2400 | Grade resistance; high RPM |
| HAULING (road) | 28–45 km/h | 1700–2050 | Open haul road; faster |
| DUMPING | 2–5 km/h | 850–1050 | At crusher; payload −25–40 t/tick |
| RETURNING (descent) | 18–30 km/h | 1200–1400 | Engine braking |
| RETURNING (road) | 35–52 km/h | 1500–1800 | Empty, faster |

**Elevation**: interpolated from a 16-point profile (22 m pit floor → 80 m ramp top → 70 m crusher → 30 m descent) with ±2 m noise per tick.

**Fuel consumption** is state-aware: 0.008%/s idle → 0.030%/s loaded hauling.

### Chaos Modes

| Mode | Trigger | Backend response |
|---|---|---|
| GPS Glitch | Every 60 ticks (~1/min per truck) | `GPS_GLITCH` alert saved; frontend marker does NOT move; glitch position excluded from live trail |
| Out-of-Order timestamp | Every 90 ticks | Dropped from SSE broadcast; saved to DB for audit |
| Network Burst | `POST http://localhost:9090/sim/burst/:truck_id` | 500 messages published instantly; mpsc backpressure (cap 4096) absorbs without crash |

---

## 7. Health Classification

All rules are threshold-based and applied per telemetry reading in `use_cases/track_vehicle.rs`.

| Alert type | Rule | Severity |
|---|---|---|
| `GPS_GLITCH` | Implied speed between two consecutive readings > 200 km/h **or** haversine distance > 50 km | WARNING |
| `OUT_OF_ORDER` | `timestamp < last_seen_timestamp` for the same truck | WARNING |
| `SUSTAINED_OVER_REV` | `engine_rpm > 2500` | CRITICAL |
| `OVERSPEED` | `speed_kmh > 60` | CRITICAL |
| `FUEL_ANOMALY` | `fuel_level_percent < 10` | WARNING |

Alerts are stored in the `health_alerts` table and surfaced on the sidebar. Each alert can be acknowledged once (idempotent — double-acknowledge returns 404).

---

## 8. API Reference

All endpoints are served on port **8080**.

| Method | Path | Query params | Response |
|---|---|---|---|
| `GET` | `/api/health` | — | `{"status":"ok"}` |
| `GET` | `/api/stream` | — | SSE stream of `TelemetryEvent` JSON |
| `GET` | `/api/fleets` | — | `Fleet[]` |
| `GET` | `/api/alerts` | — | `HealthAlert[]` (latest 100) |
| `PUT` | `/api/alerts/:id/acknowledge` | — | 204 / 404 |
| `GET` | `/api/history` | `truck_id`, `limit` (default 500) | `HistoryPoint[]` latest-first |
| `GET` | `/api/history/page` | `truck_id`, `page` (0-indexed), `page_size` (default 100) | `HistoryPage` |

**SSE event shape:**
```jsonc
{
  "fleet_id": "HT-001",
  "timestamp": "2026-05-11T03:14:15.926Z",
  "latitude": -0.5210,
  "longitude": 116.8240,
  "elevation_meters": 72.4,
  "speed_kmh": 38.2,
  "engine_rpm": 1923,
  "fuel_level_percent": 74.3,
  "payload_weight_tons": 250.0,
  "heading_degrees": 47,
  "operational_state": "HAULING",
  "is_anomaly": false,
  "anomaly_type": null
}
```

**`HistoryPage` shape:**
```jsonc
{
  "truck_id": "HT-001",
  "page": 0,
  "page_size": 100,
  "total_count": 3812,
  "points": [ /* HistoryPoint[] */ ]
}
```

---

## 9. Running Tests

### Unit tests — Rust (no infrastructure required)

```bash
# Backend (4 tests: haversine geometry, GPS glitch threshold)
cd backend && cargo test

# Simulator (9 tests: circuit movement, non-straight path, elevation,
#            ramp RPM, chaos modes, fuel, payload)
cd simulator && cargo test
```

### Unit tests — Frontend (no infrastructure required)

```bash
cd frontend
npm ci
npm test
```

### Integration tests (requires the full stack running)

```bash
# Start the stack first
docker compose up --build -d

# Wait until healthy (or watch: docker compose ps)
sleep 30

# Install Python dependencies (one-time)
pip install -r tests/requirements.txt

# Run the 30-test suite
pytest tests/ -v
```

**What the suite covers (30 tests across 3 classes):**

| Class | Count | Coverage |
|---|---|---|
| `TestPositive` | 10 | Health check, fleet seeding, DB persistence, SSE delivery, history API, alerts endpoint, burst trigger, burst DB count, OOO persistence, normal SSE events |
| `TestNegative` | 10 | Unknown truck 404, nil UUID 404, invalid UUID 4xx, burst unknown truck 404, GPS glitch DB alert, GPS glitch SSE flag, OOO filtered from SSE, missing params 4xx, overspeed alerts, double-acknowledge 404 |
| `TestE2E` | 10 | Full pipeline burst→SSE→DB, fleet API matches DB |

---

## 10. Stack Decisions & Rationale

Every choice outside the mandatory Rust backend is justified here against at least one concrete alternative.

---

### Backend framework: Axum

**Picked:** Axum 0.7 (Tokio-native, tower middleware, `IntoResponse` trait)  
**Alternative:** Actix-web (actor model, slightly higher throughput benchmark)  
**Why Axum:** Axum's extractors and shared `State<Arc<T>>` compose cleanly with Tokio's async model — no actor system to reason about. SSE support (`Sse<impl Stream>`) is first-class. Actix's actor overhead adds complexity for no benefit at 5-truck scale.  
**At 10× scale (50,000 trucks):** Axum scales horizontally behind a load balancer; the SSE fan-out would move to a Redis pub/sub channel so any pod can serve any client.

---

### Transport between simulator and backend: MQTT + Protobuf

**Picked:** Eclipse Mosquitto (MQTT 3.1.1) + Protobuf (prost)  
**Alternative A:** HTTP POST per reading (simplest)  
**Alternative B:** WebSockets + JSON  
**Why MQTT + Protobuf:** MQTT is the de facto IoT protocol — pub/sub decouples producer and consumer, handles reconnects natively, and supports QoS levels. Protobuf binary encoding is ~3–5× smaller than equivalent JSON (important on constrained mine radio links). The combination mirrors the actual production stack (Rust on the backend, Protobuf on the wire).  
**Schema evolution:** Adding a new sensor field (e.g. `tire_pressure_kpa = 13`) is backwards-compatible. Old decoders ignore unknown tag 13; old encoders leave it absent (defaults to 0). Tag numbers are never reused.  
**At 10× scale:** Replace Mosquitto with EMQX or HiveMQ cluster; partition topics by mine site.

---

### Internal message bus: `tokio::sync::mpsc`

**Picked:** Bounded mpsc channel (capacity 4096) between MQTT listener and processor worker  
**Alternative:** Kafka, RabbitMQ  
**Why mpsc:** For a single-process service with one producer (MQTT listener) and one consumer (processor), an in-process channel is the right shape. No serialisation overhead, no network hop, no JVM/Erlang dependency. The capacity bound provides backpressure — if the processor falls behind, the listener's `send().await` yields instead of growing memory unbounded.  
**At 10× scale:** Promote the MQTT listener and processor to separate services communicating via Kafka. The mpsc channel becomes a Kafka topic. The processor becomes a Kafka consumer group — horizontally scalable.

---

### Live push transport: SSE (Server-Sent Events)

**Picked:** SSE via Axum's `Sse<impl Stream>`, backed by `tokio::sync::broadcast` channel (capacity 512)  
**Alternative:** WebSockets  
**Why SSE:** Telemetry is strictly unidirectional (server → browser). SSE is a thin layer over HTTP/1.1, automatically reconnects on disconnect, and is natively supported by every browser without a JS library. WebSockets add a handshake, a framing protocol, and two-way state that this problem does not need. When the broadcast channel lags (client too slow), lagged messages are dropped rather than blocking — the client receives the next fresh frame, not a 30-second-old one.  
**At 10× scale:** SSE fans out from the backend pod to all connected browsers on that pod. A Redis pub/sub layer would be added so any pod can forward any truck's event to any client.

---

### Database: PostgreSQL + PostGIS

**Picked:** PostgreSQL 15 with PostGIS 3.4 (`postgis/postgis` Docker image)  
**Alternative A:** InfluxDB (time-series native)  
**Alternative B:** Plain SQLite  
**Why PostGIS:** `GEOMETRY(Point, 4326)` columns let the database perform spatial queries directly — `ST_Within(geom, zone)` for geofence detection and `ST_DWithin` for proximity checks — without shipping coordinates to the application layer. The composite index `(fleet_id, timestamp DESC)` + GIST spatial index covers both time-range queries and spatial queries efficiently. InfluxDB would be better for aggregation queries at very high ingestion rates but loses the relational join capability needed for alerts.  
**At 10× scale:** Partition `telemetry_logs` by `timestamp` (monthly ranges) and shard by `fleet_id`. Consider TimescaleDB as a PostGIS-compatible drop-in that adds hypertable compression.

---

### State management (ORM / query layer): SQLx

**Picked:** SQLx 0.7 (compile-time-checked queries, async)  
**Alternative:** Diesel (sync, code-gen)  
**Why SQLx:** Raw SQL is more readable for the spatial queries (`ST_Y(geom) AS latitude`) than any ORM abstraction. SQLx's `query!` macro checks SQL syntax at compile time against a live database or an offline snapshot. Diesel's sync model requires `tokio::task::spawn_blocking` wrappers around every DB call.

---

### Frontend framework: React 18 + TypeScript + Vite

**Picked:** React 18 (concurrent mode), TypeScript 5, Vite 5  
**Alternative:** SvelteKit, Vue 3  
**Why React:** The real FMS stack uses React. Familiarity means faster, more intentional decisions. React 18 concurrent mode lets the map render without blocking the SSE handler. TypeScript catches the SSE-to-store schema mismatches that would otherwise appear as runtime errors.

---

### Map library: Leaflet + react-leaflet

**Picked:** Leaflet 1.9 with CARTO dark tiles  
**Alternative:** CesiumJS (3D terrain, the production stack)  
**Why Leaflet:** 2D Leaflet is a fraction of the bundle size of CesiumJS (~50 KB gzip vs ~3 MB). For a five-truck demo on a laptop, the 3D terrain advantage of Cesium is offset by its GPU requirement and setup complexity. The assessment explicitly lists CesiumJS as a stretch goal.  
**At 10× scale / production:** Replace with CesiumJS for accurate 3D terrain, pit walls, and elevation-aware path rendering.

---

### State management: Zustand

**Picked:** Zustand 4.5  
**Alternative:** Redux Toolkit, React Context  
**Why Zustand:** Zustand's selector-based subscriptions prevent re-renders: a component subscribed to `trucks["HT-001"]` only re-renders when HT-001 changes, not when HT-002 updates. Redux Toolkit would work but adds boilerplate. React Context re-renders all consumers on every change — unusable with 1 Hz per-truck updates.

---

### Elevation chart: hand-written SVG

**Picked:** SVG `<polyline>` + `<path>` drawn from scratch inside the component  
**Alternative:** Recharts, Chart.js, D3  
**Why SVG:** Adding any charting library for a single chart would increase the bundle by 200–500 KB. An SVG area chart is ~60 lines of geometry math and is exactly as interactive as needed (click-to-scrub). No dependency to audit or update.

---

### Simulator language: Rust

**Picked:** Rust (same crate workspace pattern as the backend)  
**Why Rust:** Sharing the compiled Protobuf types (`fleet-proto`) between simulator and backend via Cargo workspace eliminates the risk of schema drift — the same generated Rust types encode and decode, so a field rename is a compile error in both binaries simultaneously.

---

## 11. AI Usage Log

This section is required by the assessment. It is honest and specific.

### What was delegated to AI

- **Initial scaffolding:** Axum router skeleton, SQLx query boilerplate, Leaflet component wiring, Docker Compose health-check ordering, Protobuf `build.rs` setup — all AI-generated first drafts.
- **Boilerplate-heavy pieces:** The Python integration test matrix (30 test cases), Tailwind CSS class combinations, nginx config, Mosquitto conf.
- **Iterative rewrites:** The `HistoryTable` component structure and the SVG elevation chart were generated with a detailed spec and then refined through multiple rounds.

### What was written or heavily rewritten by me

- **The state machine design:** The decision to derive truck operational state from *circuit position* (waypoint index) rather than tick counters was mine. The AI's first version used a pure tick-counter FSM — it worked for a straight-line simulator but produced trucks that would "load" in the middle of the haul road if they had just returned. I rejected that and designed the position-driven FSM.
- **The backpressure model:** Choosing bounded mpsc (4096) for the MQTT→processor pipe and bounded broadcast (512) for SSE, and what the right drop-on-lag semantics are for each. The AI proposed an unbounded channel; I changed it and wrote the explanation.
- **GPS glitch handling in the frontend store:** The decision that glitch positions must *not* enter the live trail ring-buffer (so the polyline doesn't spike 150 km and back) — the AI's first draft appended every incoming position unconditionally. I caught this and added the `isGpsGlitch` guard in `applyTelemetry`.
- **The 16-waypoint mine circuit geometry:** The waypoint coordinates, elevation profile, and the per-state physics (ramp vs. open road RPM split, engine-braking on descent) — these required domain reasoning about real open-pit mine operations that the AI did not volunteer.

### One concrete case where the AI was wrong

**Problem:** When I asked the AI to add per-truck trail polylines to the live map, it emitted a `useEffect` inside `FleetMap` that called `fetch('/api/history?truck_id=...')` every time the SSE event fired — fetching 500 DB rows per truck per second just to draw the trail.

**How I caught it:** I read the generated code before running it. The effect had no debounce or dependency guard, and the fetch was inside the render-time component body rather than in the store. At 5 trucks × 1 Hz, this would have issued 5 DB queries/second and re-rendered the map on every response.

**What I did instead:** I added a `trail: LiveTrailPoint[]` ring-buffer directly to the Zustand store's `TruckLiveState`. The trail is populated in `applyTelemetry` as SSE events arrive — zero extra fetches, zero extra renders beyond the truck's natural update rate, and the 200-point cap keeps memory bounded. The history fetch only happens when the user explicitly clicks "View History."

---

## 12. What I Would Do Next

These are conscious scope decisions, not omissions.

### 1. CesiumJS 3D terrain view

The production FMS uses CesiumJS. Replacing Leaflet with a CesiumJS globe would enable accurate pit-wall geometry, elevation-accurate truck paths, and shadow simulation for visibility analysis. I did not ship this because CesiumJS requires a GPU and its Docker setup is non-trivial — a reviewer running this on a headless CI machine would have a broken experience.

### 2. Geofence enforcement

The database schema already seeds three geofences (LOADING_ZONE, CRUSHER, WORKSHOP) as PostGIS `POLYGON` geometries. The next step is a `ST_Within(tel.geom, geofence.geom)` check in the processor worker: if a truck reports HAULING state but its position is inside the crusher polygon, that is a state disagreement worth alerting on. The spatial index makes this query cheap.

### 3. Per-truck fuel depletion projection

With the current per-state fuel rate constants, a simple linear projection would compute "estimated empty at current consumption rate" and surface it on the DrillDown drawer. At < 15% fuel with no refuel event in the last N minutes, trigger a `FUEL_ANOMALY` alert before the truck runs dry mid-haul — more useful to a dispatcher than a < 10% threshold.

---

## Project Structure

```
.
├── backend/
│   ├── src/
│   │   ├── main.rs                  # Composition root — wires all layers
│   │   ├── config/                  # AppConfig from env vars
│   │   ├── domain/
│   │   │   ├── models.rs            # Fleet, TelemetryReading, HealthAlert, HistoryPoint, …
│   │   │   └── errors.rs            # DomainError + AppError (implements IntoResponse)
│   │   ├── infrastructure/
│   │   │   ├── messaging.rs         # MQTT listener (rumqttc) → mpsc channel
│   │   │   └── persistence.rs       # FleetRepository (SQLx) — all DB ops
│   │   ├── use_cases/
│   │   │   ├── track_vehicle.rs     # Processor: OOO, GPS glitch, health rules, SSE broadcast
│   │   │   └── manage_fleet.rs      # Read queries + alert acknowledgement
│   │   └── interface/
│   │       └── http.rs              # Axum router + 7 route handlers
│   ├── proto/telemetry.proto        # Protobuf schema (12 fields + optional ExcavatorData)
│   └── build.rs                     # prost-build: .proto → Rust at compile time
│
├── simulator/
│   └── src/
│       ├── main.rs                  # Spawns 5 Tokio tasks + HTTP burst control (port 9090)
│       └── truck.rs                 # 16-waypoint mine circuit, state machine, chaos modes
│
├── frontend/
│   └── src/
│       ├── App.tsx                  # Root layout
│       ├── hooks/useSseStream.ts    # EventSource → Zustand; alert polling; stale marking
│       ├── store/fleetStore.ts      # Zustand store: trucks map, live trail ring-buffer, history
│       ├── types/index.ts           # TypeScript interfaces for all wire/domain types
│       └── components/
│           ├── FleetMap.tsx         # Leaflet map: live trails + markers; history polyline
│           ├── Sidebar.tsx          # Fleet list, alerts, ack controls, history table button
│           ├── DrillDown.tsx        # Per-truck stats drawer
│           ├── HistoryScrubber.tsx  # SVG elevation chart + timeline scrubber
│           └── HistoryTable.tsx     # Two-level paginated history browser
│
├── infra/
│   ├── init.sql                     # PostGIS schema + seed data (fleets, geofences)
│   └── mosquitto.conf               # Mosquitto broker config
│
├── tests/
│   ├── test_integration.py          # 30 pytest tests (positive, negative, E2E)
│   └── requirements.txt
│
└── docker-compose.yml               # Five services with health-checked dependency ordering
```
