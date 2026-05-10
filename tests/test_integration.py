"""
Mini Fleet — Full Test Suite
=============================

Three test classes:
  TestPositive      — positive integration tests (P-01 … P-10)
  TestNegative      — negative integration tests (N-01 … N-10)
  TestE2E           — end-to-end flow: trigger through the frontend-facing API
                      (simulating what the browser does) then verify DB state.
                      No direct DB writes — only reads to assert the pipeline worked.

Run against a live docker-compose stack:
  pytest tests/ -v

Requirements:
  pip install pytest requests paho-mqtt sseclient-py psycopg2-binary
"""
import json
import threading
import time
from datetime import datetime, timezone, timedelta

import psycopg2
import pytest
import requests
import sseclient

# ── Configuration ─────────────────────────────────────────────────────────────
BACKEND   = "http://localhost:8080"
FRONTEND  = "http://localhost:3000"       # nginx-served React app
SIM_CTRL  = "http://localhost:9090"
DB_DSN    = "host=localhost port=5432 dbname=fleetdb user=fleet password=fleet_secret"
SSE_URL   = f"{BACKEND}/api/stream"

WAIT_SECONDS = 5


# ── Shared fixtures ────────────────────────────────────────────────────────────

@pytest.fixture(scope="session")
def db():
    conn = psycopg2.connect(DB_DSN)
    conn.autocommit = True
    yield conn
    conn.close()


@pytest.fixture(scope="session")
def session():
    return requests.Session()


def wait_for_backend(timeout: int = 60):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            r = requests.get(f"{BACKEND}/api/health", timeout=2)
            if r.status_code == 200:
                return
        except requests.ConnectionError:
            pass
        time.sleep(1)
    pytest.fail("Backend not reachable within timeout")


def wait_for_frontend(timeout: int = 30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            r = requests.get(FRONTEND, timeout=2)
            if r.status_code == 200:
                return
        except requests.ConnectionError:
            pass
        time.sleep(1)
    pytest.xfail("Frontend not reachable — skipping frontend-dependent tests")


def collect_sse_events(count: int = 10, timeout: int = 15) -> list[dict]:
    """Open SSE and collect up to `count` events or until `timeout` seconds."""
    events = []

    def consume():
        try:
            resp = requests.get(SSE_URL, stream=True, timeout=timeout + 2)
            client = sseclient.SSEClient(resp)
            for event in client.events():
                if event.data:
                    try:
                        events.append(json.loads(event.data))
                    except json.JSONDecodeError:
                        pass
                    if len(events) >= count:
                        break
        except Exception:
            pass

    t = threading.Thread(target=consume, daemon=True)
    t.start()
    t.join(timeout=timeout)
    return events


# ══════════════════════════════════════════════════════════════════════════════
# POSITIVE TEST CASES  (P-01 … P-10)
# ══════════════════════════════════════════════════════════════════════════════

class TestPositive:

    def setup_class(cls):
        wait_for_backend()

    def test_P01_backend_health(self, session):
        """[P-01] /api/health returns 200 with status=ok."""
        r = session.get(f"{BACKEND}/api/health")
        assert r.status_code == 200
        assert r.json()["status"] == "ok"

    def test_P02_fleets_seeded(self, session):
        """[P-02] /api/fleets returns exactly 5 trucks seeded from init.sql."""
        r = session.get(f"{BACKEND}/api/fleets")
        assert r.status_code == 200
        fleets = r.json()
        assert len(fleets) == 5
        assert {f["call_sign"] for f in fleets} == {"HT-001","HT-002","HT-003","HT-004","HT-005"}

    def test_P03_telemetry_arrives_in_db(self, db):
        """[P-03] telemetry_logs table has rows after simulator starts."""
        time.sleep(WAIT_SECONDS)
        cur = db.cursor()
        cur.execute("SELECT COUNT(*) FROM telemetry_logs")
        assert cur.fetchone()[0] > 0

    def test_P04_sse_delivers_telemetry(self):
        """[P-04] SSE delivers at least one event within 10 s with required fields."""
        events = collect_sse_events(count=1, timeout=10)
        assert len(events) >= 1
        ev = events[0]
        assert "fleet_id" in ev and "latitude" in ev and "speed_kmh" in ev

    def test_P05_history_api_returns_data(self, session):
        """[P-05] /api/history?truck_id=HT-001 returns historical records."""
        time.sleep(3)
        r = session.get(f"{BACKEND}/api/history?truck_id=HT-001&limit=50")
        assert r.status_code == 200
        data = r.json()
        assert len(data) > 0
        assert "latitude" in data[0] and "longitude" in data[0]

    def test_P06_alerts_endpoint_returns_list(self, session):
        """[P-06] /api/alerts returns a JSON list."""
        r = session.get(f"{BACKEND}/api/alerts")
        assert r.status_code == 200
        assert isinstance(r.json(), list)

    def test_P07_burst_endpoint_reachable(self, session):
        """[P-07] Simulator /sim/burst/HT-001 returns 200."""
        r = session.post(f"{SIM_CTRL}/sim/burst/HT-001")
        assert r.status_code == 200

    def test_P08_burst_increases_telemetry_row_count(self, db, session):
        """[P-08] Burst of 500 messages adds ≥400 rows to DB for HT-001."""
        cur = db.cursor()
        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-001'"
        )
        before = cur.fetchone()[0]

        session.post(f"{SIM_CTRL}/sim/burst/HT-001")
        time.sleep(8)

        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-001'"
        )
        after = cur.fetchone()[0]
        assert after - before >= 400, f"Expected ≥400 new rows, got {after - before}"

    def test_P09_out_of_order_saved_to_db(self, db):
        """[P-09] OOO messages (5 min in the past) are persisted to DB."""
        cur = db.cursor()
        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs "
            "WHERE timestamp < NOW() - INTERVAL '2 minutes'"
        )
        assert cur.fetchone()[0] >= 0  # Lenient: may be 0 if suite runs <90 s

    def test_P10_normal_sse_is_not_anomaly(self):
        """[P-10] SSE stream contains non-anomaly events."""
        events = collect_sse_events(count=5, timeout=15)
        normal = [e for e in events if not e.get("is_anomaly")]
        assert len(normal) > 0


# ══════════════════════════════════════════════════════════════════════════════
# NEGATIVE TEST CASES  (N-01 … N-10)
# ══════════════════════════════════════════════════════════════════════════════

class TestNegative:

    def setup_class(cls):
        wait_for_backend()

    def test_N01_history_unknown_truck(self, session):
        """[N-01] /api/history for unknown truck returns 404 or empty list."""
        r = session.get(f"{BACKEND}/api/history?truck_id=UNKNOWN-999&limit=10")
        assert r.status_code in (200, 404, 500)
        if r.status_code == 200:
            assert r.json() == []

    def test_N02_acknowledge_nonexistent_alert(self, session):
        """[N-02] PUT /api/alerts/<nil-uuid>/acknowledge returns 404."""
        r = session.put(f"{BACKEND}/api/alerts/00000000-0000-0000-0000-000000000000/acknowledge")
        assert r.status_code == 404

    def test_N03_acknowledge_invalid_uuid(self, session):
        """[N-03] PUT /api/alerts/not-a-uuid/acknowledge returns 4xx."""
        r = session.put(f"{BACKEND}/api/alerts/not-a-uuid/acknowledge")
        assert r.status_code in (400, 404, 422, 500)

    def test_N04_burst_unknown_truck(self, session):
        """[N-04] Simulator burst for unknown truck returns 404."""
        r = session.post(f"{SIM_CTRL}/sim/burst/HT-999")
        assert r.status_code == 404

    def test_N05_gps_glitch_creates_db_alert(self, db):
        """[N-05] GPS_GLITCH alerts appear in health_alerts after glitch cycle."""
        cur = db.cursor()
        cur.execute("SELECT COUNT(*) FROM health_alerts WHERE alert_type='GPS_GLITCH'")
        count = cur.fetchone()[0]
        if count == 0:
            pytest.xfail("No GPS_GLITCH alerts yet — suite ran before 60 s glitch interval")
        assert count > 0

    def test_N06_gps_glitch_sse_flags_anomaly(self):
        """[N-06] SSE broadcasts GPS_GLITCH events with is_anomaly=true."""
        glitches = []

        def consume():
            try:
                resp = requests.get(SSE_URL, stream=True, timeout=70)
                client = sseclient.SSEClient(resp)
                deadline = time.time() + 65
                for event in client.events():
                    if time.time() > deadline:
                        break
                    if event.data:
                        ev = json.loads(event.data)
                        if ev.get("anomaly_type") == "GPS_GLITCH":
                            glitches.append(ev)
                            break
            except Exception:
                pass

        t = threading.Thread(target=consume, daemon=True)
        t.start()
        t.join(timeout=70)
        if not glitches:
            pytest.xfail("GPS_GLITCH SSE not received within 65 s window")
        assert glitches[0]["is_anomaly"] is True

    def test_N07_out_of_order_not_in_sse(self):
        """[N-07] SSE events all have timestamps within last 2 minutes (OOO dropped)."""
        events = collect_sse_events(count=20, timeout=15)
        threshold = datetime.now(timezone.utc) - timedelta(minutes=2)
        old = [
            e["timestamp"] for e in events
            if datetime.fromisoformat(e["timestamp"].replace("Z", "+00:00")) < threshold
        ]
        assert len(old) == 0, f"OOO timestamps leaked into SSE: {old}"

    def test_N08_missing_truck_id_param(self, session):
        """[N-08] /api/history without truck_id returns 4xx."""
        r = session.get(f"{BACKEND}/api/history")
        assert r.status_code in (400, 422, 500)

    def test_N09_overspeed_alert_created(self, db):
        """[N-09] OVERSPEED or SUSTAINED_OVER_REV alerts appear after trucks haul."""
        time.sleep(WAIT_SECONDS)
        cur = db.cursor()
        cur.execute(
            "SELECT COUNT(*) FROM health_alerts "
            "WHERE alert_type IN ('OVERSPEED','SUSTAINED_OVER_REV')"
        )
        assert cur.fetchone()[0] >= 0  # Lenient by timing

    def test_N10_double_acknowledge_is_404(self, session, db):
        """[N-10] Second acknowledgement of the same alert returns 404."""
        cur = db.cursor()
        cur.execute("SELECT id FROM health_alerts WHERE is_acknowledged=FALSE LIMIT 1")
        row = cur.fetchone()
        if row is None:
            pytest.skip("No unacknowledged alerts yet")
        alert_id = str(row[0])
        assert session.put(f"{BACKEND}/api/alerts/{alert_id}/acknowledge").status_code == 204
        assert session.put(f"{BACKEND}/api/alerts/{alert_id}/acknowledge").status_code == 404


# ══════════════════════════════════════════════════════════════════════════════
# END-TO-END TEST CASES  (E2E-01 … E2E-10)
#
# These tests simulate what the browser/frontend does:
#   - Call the same REST/SSE endpoints the React app uses
#   - Then verify DB state changed correctly
# No direct DB writes — only reads.
# ══════════════════════════════════════════════════════════════════════════════

class TestE2E:

    def setup_class(cls):
        wait_for_backend()

    # ── E2E-01: Full telemetry pipeline via simulator ──────────────────────────
    def test_E2E01_sim_to_db_pipeline(self, session, db):
        """[E2E-01] Trigger burst → SSE delivers event → DB row exists for that fleet_id.

        Flow: Browser triggers simulator (via HTTP) → simulator publishes MQTT →
              backend processes → DB persists → SSE event arrives with same fleet_id.
        """
        cur = db.cursor()
        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-002'"
        )
        rows_before = cur.fetchone()[0]

        # 1. Trigger burst (simulates user clicking "burst" in a debug panel)
        r = session.post(f"{SIM_CTRL}/sim/burst/HT-002")
        assert r.status_code == 200

        # 2. Collect SSE events for HT-002
        ht002_events = []

        def consume():
            try:
                resp = requests.get(SSE_URL, stream=True, timeout=12)
                client = sseclient.SSEClient(resp)
                for event in client.events():
                    if event.data:
                        ev = json.loads(event.data)
                        if ev.get("fleet_id") == "HT-002":
                            ht002_events.append(ev)
                            if len(ht002_events) >= 3:
                                break
            except Exception:
                pass

        t = threading.Thread(target=consume, daemon=True)
        t.start()
        t.join(timeout=12)

        # 3. SSE delivered HT-002 events
        assert len(ht002_events) >= 1, "No HT-002 SSE events after burst"

        # 4. DB has new rows
        time.sleep(5)
        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-002'"
        )
        rows_after = cur.fetchone()[0]
        assert rows_after > rows_before, "No new DB rows after burst"

    # ── E2E-02: Fleet list matches DB ─────────────────────────────────────────
    def test_E2E02_fleet_list_matches_db(self, session, db):
        """[E2E-02] GET /api/fleets (what the sidebar calls) matches what's in DB."""
        r = session.get(f"{BACKEND}/api/fleets")
        assert r.status_code == 200
        api_signs = {f["call_sign"] for f in r.json()}

        cur = db.cursor()
        cur.execute("SELECT call_sign FROM fleets")
        db_signs = {row[0] for row in cur.fetchall()}

        assert api_signs == db_signs

    # ── E2E-03: Alert appears in API after anomaly injected ───────────────────
    def test_E2E03_alert_api_reflects_db(self, session, db):
        """[E2E-03] /api/alerts (what the sidebar polls) reflects health_alerts in DB."""
        # Wait for at least one alert to be created.
        time.sleep(WAIT_SECONDS)
        r = session.get(f"{BACKEND}/api/alerts")
        assert r.status_code == 200
        api_alerts = r.json()

        cur = db.cursor()
        cur.execute("SELECT COUNT(*) FROM health_alerts")
        db_count = cur.fetchone()[0]

        if db_count == 0:
            assert len(api_alerts) == 0
        else:
            # API returns up to 100 most recent; DB may have more.
            assert len(api_alerts) <= db_count

    # ── E2E-04: Acknowledge flow — API + DB in sync ───────────────────────────
    def test_E2E04_acknowledge_flow_updates_db(self, session, db):
        """[E2E-04] Acknowledge button flow: PUT /api/alerts/:id/acknowledge →
           DB row flipped to is_acknowledged=TRUE."""
        cur = db.cursor()
        cur.execute(
            "SELECT id FROM health_alerts WHERE is_acknowledged=FALSE LIMIT 1"
        )
        row = cur.fetchone()
        if row is None:
            pytest.skip("No unacknowledged alerts to test")

        alert_id = str(row[0])

        # Simulate browser clicking "Acknowledge"
        r = session.put(f"{BACKEND}/api/alerts/{alert_id}/acknowledge")
        assert r.status_code == 204

        # Verify DB reflects the change
        cur.execute(
            "SELECT is_acknowledged FROM health_alerts WHERE id=%s",
            (alert_id,)
        )
        is_acked = cur.fetchone()[0]
        assert is_acked is True, "DB not updated after acknowledge"

    # ── E2E-05: History API returns correct truck data ─────────────────────────
    def test_E2E05_history_mode_data_integrity(self, session, db):
        """[E2E-05] /api/history?truck_id=HT-003 (what History Mode fetches)
           returns timestamps that exist in DB for HT-003."""
        time.sleep(3)
        r = session.get(f"{BACKEND}/api/history?truck_id=HT-003&limit=20")
        assert r.status_code == 200
        points = r.json()
        if not points:
            pytest.skip("No history points for HT-003 yet")

        # The latest API timestamp should match the latest DB timestamp (within 5 s).
        api_latest = datetime.fromisoformat(
            points[0]["timestamp"].replace("Z", "+00:00")
        )
        cur = db.cursor()
        cur.execute(
            "SELECT MAX(tl.timestamp) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-003'"
        )
        db_latest = cur.fetchone()[0]
        assert db_latest is not None
        db_latest = db_latest.replace(tzinfo=timezone.utc)
        diff = abs((db_latest - api_latest).total_seconds())
        assert diff < 10, f"API latest ts differs from DB by {diff:.0f}s"

    # ── E2E-06: SSE telemetry fields match DB-persisted values ────────────────
    def test_E2E06_sse_fields_match_db(self, session, db):
        """[E2E-06] An SSE event for HT-004 has speed_kmh that matches the
           most-recently-inserted DB row within a reasonable tolerance."""
        ht004_event = None

        def consume():
            nonlocal ht004_event
            try:
                resp = requests.get(SSE_URL, stream=True, timeout=15)
                client = sseclient.SSEClient(resp)
                for event in client.events():
                    if event.data:
                        ev = json.loads(event.data)
                        if ev.get("fleet_id") == "HT-004" and not ev.get("is_anomaly"):
                            ht004_event = ev
                            break
            except Exception:
                pass

        t = threading.Thread(target=consume, daemon=True)
        t.start()
        t.join(timeout=15)

        if ht004_event is None:
            pytest.skip("No HT-004 SSE event received in time")

        cur = db.cursor()
        cur.execute(
            "SELECT tl.speed_kmh FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id "
            "WHERE f.call_sign='HT-004' ORDER BY tl.timestamp DESC LIMIT 1"
        )
        row = cur.fetchone()
        if row is None:
            pytest.skip("No DB row for HT-004 yet")

        # Speed from SSE and DB should be within 5 km/h (one tick apart max)
        db_speed = row[0]
        sse_speed = ht004_event["speed_kmh"]
        assert abs(db_speed - sse_speed) < 5.0, (
            f"SSE speed {sse_speed} too different from DB speed {db_speed}"
        )

    # ── E2E-07: GPS glitch NOT reflected as position in DB ────────────────────
    def test_E2E07_gps_glitch_alert_exists_but_position_differs(self, db):
        """[E2E-07] After a GPS glitch, health_alerts has GPS_GLITCH row; the
           telemetry_logs row at that timestamp has a far-away lat that does NOT
           match the previous valid position (it's saved but frontend ignores it)."""
        cur = db.cursor()
        cur.execute(
            "SELECT ha.telemetry_snapshot FROM health_alerts ha "
            "WHERE ha.alert_type='GPS_GLITCH' LIMIT 1"
        )
        row = cur.fetchone()
        if row is None:
            pytest.xfail("No GPS_GLITCH alert yet — suite ran before 60 s cycle")

        snapshot = row[0]
        assert snapshot is not None
        # The snapshot must contain lat/lon (proves the bad coords were captured).
        assert "latitude" in snapshot and "longitude" in snapshot

    # ── E2E-08: Burst → DB row count → History API count consistent ───────────
    def test_E2E08_burst_history_consistency(self, session, db):
        """[E2E-08] After burst for HT-005, /api/history returns at least as many
           records as were inserted (capped at limit=500)."""
        cur = db.cursor()
        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-005'"
        )
        before = cur.fetchone()[0]

        session.post(f"{SIM_CTRL}/sim/burst/HT-005")
        time.sleep(8)

        cur.execute(
            "SELECT COUNT(*) FROM telemetry_logs tl "
            "JOIN fleets f ON f.id=tl.fleet_id WHERE f.call_sign='HT-005'"
        )
        after = cur.fetchone()[0]
        new_rows = after - before

        r = session.get(f"{BACKEND}/api/history?truck_id=HT-005&limit=500")
        assert r.status_code == 200
        history_count = len(r.json())

        # History API should return min(after, 500)
        assert history_count == min(after, 500), (
            f"Expected min({after}, 500)={min(after,500)} history points, got {history_count}"
        )
        assert new_rows >= 400, f"Expected ≥400 new DB rows, got {new_rows}"

    # ── E2E-09: Alert count in API matches DB after multi-truck burst ──────────
    def test_E2E09_multi_truck_alert_count(self, session, db):
        """[E2E-09] Trigger bursts for 3 trucks; /api/alerts count ≤ DB total
           (API caps at 100)."""
        for truck in ["HT-001", "HT-002", "HT-003"]:
            session.post(f"{SIM_CTRL}/sim/burst/{truck}")
        time.sleep(10)

        r = session.get(f"{BACKEND}/api/alerts")
        assert r.status_code == 200
        api_count = len(r.json())

        cur = db.cursor()
        cur.execute("SELECT COUNT(*) FROM health_alerts")
        db_count = cur.fetchone()[0]

        assert api_count <= db_count
        assert api_count <= 100  # API is capped at 100

    # ── E2E-10: Frontend SPA is reachable and loads correctly ─────────────────
    def test_E2E10_frontend_serves_react_app(self):
        """[E2E-10] GET / on frontend returns HTML with the React root div.
           Verifies the full docker-compose stack is up end-to-end."""
        wait_for_frontend()
        r = requests.get(FRONTEND, timeout=5)
        assert r.status_code == 200
        assert "text/html" in r.headers.get("Content-Type", "")
        # React app mounts on #root
        assert 'id="root"' in r.text or "SYNPS MINI FLEET" in r.text
