# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

IPFS-hosted static frontend combined with a self-hosted backend on a VPS. The frontend is served via DNSLink (own domain mapped to an IPFS CID), which gives a stable origin for CORS.

## Current stack: Chat

```
frontend/          Vanilla JS static site — deployed to IPFS
chat/              Rust (Axum) backend — WebSocket + REST
caddy/             Reverse proxy — TLS (Let's Encrypt) + routing
docker-compose.yml Full stack definition
.env.example       Environment variable template
```

### Architecture

```
IPFS frontend (DNSLink → app.<domain>)
    │
    └── api.<domain>  →  Caddy  →  chat:8001 (Axum, Rust)
                                        │
                                    PostgreSQL
```

### Chat backend (`chat/`)

- **Framework:** Axum 0.7 with `ws` feature
- **Database:** SQLx 0.8 + PostgreSQL (runtime, not compile-time checked queries)
- **Auth:** Argon2 password hashing, JWT via `jsonwebtoken`
- **WebSocket:** one `tokio::sync::broadcast` channel per room, stored in `AppState.rooms: RoomMap`
- **Migrations:** SQLx migrate (`sqlx::migrate!("./migrations")`) — runs on startup automatically
- **CORS:** `tower_http::cors::CorsLayer`, configured via `ALLOWED_ORIGIN` env var (`*` = wildcard for local dev)

#### API surface
| Method | Path | Auth |
|--------|------|------|
| POST | /auth/register | — |
| POST | /auth/token | — |
| GET | /auth/me | Bearer |
| GET | /rooms | Bearer |
| POST | /rooms | Bearer |
| GET | /rooms/:id/messages | Bearer |
| WS | /ws/:room_id?token=\<jwt\> | query param |

#### Auth
`auth::authenticate(&state, &headers).await?` validates the Bearer token and returns the `User`. Call it at the top of any handler that requires authentication.

### Frontend (`frontend/`)

Three pages: `index.html` (login/register), `rooms.html` (room list), `chat.html` (WebSocket chat). No framework — vanilla JS. API base URL and WebSocket URL are in `config.js` and must be updated before deploying to IPFS.

## Running locally

```bash
cp .env.example .env
# Edit .env — set POSTGRES_PASSWORD and JWT_SECRET at minimum
docker compose up --build
```

The Rust image uses a two-stage build (builder: `rust:1-slim`, runtime: `debian:bookworm-slim`). First build is slow due to dependency compilation; subsequent builds use the layer cache.

`docker-compose.override.yml` is picked up automatically and configures local dev (HTTP-only Caddy, chat exposed on port 8001). In production, use `docker compose -f docker-compose.yml up` to skip the override.

To develop the backend without Docker:
```bash
cd chat
DATABASE_URL=postgres://chatuser:password@localhost:5432/chatdb \
JWT_SECRET=dev-secret \
ALLOWED_ORIGIN='*' \
cargo run
```

## Key decisions

- **Axum over Actix-web:** better async ergonomics, clean extractor model
- **SQLx runtime API** (not `query!` macro): avoids needing `DATABASE_URL` at compile time inside Docker
- **Broadcast channel per room:** simple in-memory fan-out; restarting the server drops active connections (acceptable)
- **DNSLink on frontend domain:** gives a single stable `ALLOWED_ORIGIN` instead of a wildcard in production
