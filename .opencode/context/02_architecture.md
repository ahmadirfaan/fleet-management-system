# SYSTEM ARCHITECTURE & DATA FLOW

We are building a Monolith approach combining Edge & Cloud logic to save local resources, making it easy for the reviewer to run on a single machine.

## Data Flow
1. **Simulator (Rust):** Publishes mock telemetry to Mosquitto MQTT.
2. **Component A (MQTT Listener):** A Tokio task inside the Axum backend that subscribes to Mosquitto and pushes the payload into a `tokio::sync::mpsc` channel.
3. **Component B (Processor Worker):** Consumes from the `mpsc` channel, validates edge cases (glitches, timestamps), saves valid logs to PostgreSQL, and broadcasts to the Frontend via Server-Sent Events (SSE).
4. **Frontend (React):** Listens to SSE for live map updates and fetches REST APIs for historical data.

## Trade-offs (To be highlighted in README)
- **Why `tokio::sync::mpsc` instead of Kafka/RabbitMQ?** Running JVM/Erlang based message brokers locally takes heavy RAM. For this MVP, an in-memory channel perfectly mocks a message broker. At a 10,000 truck scale, Component A and B would be split into separate microservices communicating via Apache Kafka.
- **Why Server-Sent Events (SSE) instead of WebSockets?**
  Telemetry tracking is unidirectional (Server to Client). SSE is lighter, natively supported by browsers, and easier to implement for simple real-time broadcasts. WebSockets would be used if the frontend needed to send commands back to the trucks.