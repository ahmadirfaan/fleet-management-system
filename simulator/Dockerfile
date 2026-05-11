FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto/ proto/
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release 2>/dev/null; rm -rf src

COPY src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/fleet-simulator /usr/local/bin/fleet-simulator
CMD ["fleet-simulator"]
