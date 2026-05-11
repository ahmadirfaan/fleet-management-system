-- Enable PostGIS
CREATE EXTENSION IF NOT EXISTS postgis;

-- ─────────────────────────────────────────────
-- fleets (master data)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fleets (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    call_sign   VARCHAR(20)  NOT NULL UNIQUE,
    fleet_type  VARCHAR(20)  NOT NULL CHECK (fleet_type IN ('HAUL_TRUCK', 'EXCAVATOR')),
    make_model  VARCHAR(100) NOT NULL,
    status      VARCHAR(20)  NOT NULL DEFAULT 'ACTIVE'
                    CHECK (status IN ('ACTIVE', 'MAINTENANCE', 'BREAKDOWN'))
);

-- Seed 5 haul trucks
INSERT INTO fleets (id, call_sign, fleet_type, make_model, status) VALUES
    ('11111111-1111-1111-1111-111111111111', 'HT-001', 'HAUL_TRUCK', 'Caterpillar 797F', 'ACTIVE'),
    ('22222222-2222-2222-2222-222222222222', 'HT-002', 'HAUL_TRUCK', 'Komatsu 930E',     'ACTIVE'),
    ('33333333-3333-3333-3333-333333333333', 'HT-003', 'HAUL_TRUCK', 'Caterpillar 797F', 'ACTIVE'),
    ('44444444-4444-4444-4444-444444444444', 'HT-004', 'HAUL_TRUCK', 'Komatsu 930E',     'ACTIVE'),
    ('55555555-5555-5555-5555-555555555555', 'HT-005', 'HAUL_TRUCK', 'Liebherr T 284',   'ACTIVE')
ON CONFLICT (call_sign) DO NOTHING;

-- ─────────────────────────────────────────────
-- telemetry_logs (time-series)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS telemetry_logs (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    fleet_id            UUID         NOT NULL REFERENCES fleets(id),
    timestamp           TIMESTAMPTZ  NOT NULL,
    geom                GEOMETRY(Point, 4326) NOT NULL,
    elevation_meters    FLOAT,
    speed_kmh           FLOAT,
    engine_rpm          INTEGER,
    fuel_level_percent  FLOAT,
    payload_weight_tons FLOAT,
    heading_degrees     INTEGER,
    operational_state   VARCHAR(20)  CHECK (operational_state IN ('LOADING','HAULING','DUMPING','IDLE','RETURNING')),
    is_anomaly          BOOLEAN      NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_telemetry_fleet_time
    ON telemetry_logs (fleet_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_telemetry_geom
    ON telemetry_logs USING GIST (geom);

-- ─────────────────────────────────────────────
-- excavator_telemetry (extension)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS excavator_telemetry (
    telemetry_id        UUID  PRIMARY KEY REFERENCES telemetry_logs(id),
    boom_angle_degrees  FLOAT,
    arm_angle_degrees   FLOAT,
    bucket_angle_degrees FLOAT,
    swing_speed_rpm     FLOAT
);

-- ─────────────────────────────────────────────
-- geofences (spatial boundaries)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS geofences (
    id          UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(100) NOT NULL,
    zone_type   VARCHAR(20)  NOT NULL CHECK (zone_type IN ('LOADING_ZONE','CRUSHER','WORKSHOP')),
    geom        GEOMETRY(Polygon, 4326) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_geofences_geom
    ON geofences USING GIST (geom);

-- Seed geofences (approx Kalimantan mine coords for demo)
INSERT INTO geofences (name, zone_type, geom) VALUES
    ('Loading Zone A', 'LOADING_ZONE',
     ST_GeomFromText('POLYGON((116.80 -0.50, 116.82 -0.50, 116.82 -0.52, 116.80 -0.52, 116.80 -0.50))', 4326)),
    ('Crusher North',  'CRUSHER',
     ST_GeomFromText('POLYGON((116.85 -0.48, 116.87 -0.48, 116.87 -0.50, 116.85 -0.50, 116.85 -0.48))', 4326)),
    ('Workshop',       'WORKSHOP',
     ST_GeomFromText('POLYGON((116.78 -0.54, 116.80 -0.54, 116.80 -0.56, 116.78 -0.56, 116.78 -0.54))', 4326))
ON CONFLICT DO NOTHING;

-- ─────────────────────────────────────────────
-- health_alerts (anomaly log)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS health_alerts (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    fleet_id            UUID         NOT NULL REFERENCES fleets(id),
    alert_type          VARCHAR(30)  NOT NULL
                            CHECK (alert_type IN ('GPS_GLITCH','SUSTAINED_OVER_REV','OVERSPEED','OUT_OF_ORDER','FUEL_ANOMALY')),
    severity            VARCHAR(10)  NOT NULL CHECK (severity IN ('WARNING','CRITICAL')),
    start_timestamp     TIMESTAMPTZ  NOT NULL,
    end_timestamp       TIMESTAMPTZ,
    is_acknowledged     BOOLEAN      NOT NULL DEFAULT FALSE,
    telemetry_snapshot  JSONB
);
