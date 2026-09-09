FROM rust:1-slim AS builder

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs
RUN cargo build --release && rm -rf src

# Build application
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# CIS/ADR-0024: kør som non-root. Loft skriver ikke til filsystemet, så
# read-only rootfs (k8s securityContext) og ingen uploads-sti er nødvendig.
RUN useradd --uid 10001 --create-home appuser

WORKDIR /app
COPY --from=builder /app/target/release/chat .

USER appuser

CMD ["./chat"]
