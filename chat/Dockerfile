FROM rust:1-slim AS builder

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src

# Build application
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/chat .

CMD ["./chat"]
