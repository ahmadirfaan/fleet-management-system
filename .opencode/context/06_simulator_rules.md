# SIMULATOR & EDGE CASES

The Rust simulator must publish normal data every 1 second, but it must occasionally trigger specific edge cases to prove our backend processor is robust.

Implement these 3 Chaos Modes in the Simulator:

1. **Sensor Glitch (GPS Jump):**
   - Once every minute, simulate a random truck sending a coordinate that is 100km away, resulting in an impossible speed.
   - *Backend Expectation:* The backend must flag this as `GPS_GLITCH`, save it to DB, but the frontend map must not move the truck.

2. **Out-of-Order Messages:**
   - Occasionally send a payload with a `timestamp` that is 5 minutes older than the current time.
   - *Backend Expectation:* Backend detects it's older than the `last_timestamp` in memory. It saves to DB but drops the SSE broadcast to prevent the map from moving backward.

3. **Network Burst:**
   - Add an endpoint or CLI flag to trigger a burst of 500 messages instantly for one truck.
   - *Backend Expectation:* The `tokio::sync::mpsc` channel handles the backpressure without dropping messages or crashing the SQLx connection pool.