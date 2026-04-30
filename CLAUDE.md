# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

IPFS-hosted static frontend combined with a self-hosted backend on a VPS. The frontend is served via DNSLink (own domain mapped to an IPFS CID), which gives a stable origin for CORS.

## Current stack: Chat

```
frontend/          Vanilla JS static site — deployed to IPFS
chat/              Rust (Axum) backend — WebSocket + REST
caddy/             Reverse proxy — TLS (Let's Encrypt) + routing
ansible/           Server provisioning and deployment
docker-compose.yml Full stack definition
.env.example       Environment variable template
```

### Architecture

```
IPFS frontend (DNSLink → app.gihc.online)
    │
    └── api.gihc.online  →  Caddy  →  chat:8001 (Axum, Rust)
                                           │
                                       PostgreSQL
```

### Chat backend (`chat/`)

- **Framework:** Axum 0.7 with `ws` feature
- **Database:** SQLx 0.8 + PostgreSQL (runtime, not compile-time checked queries)
- **Auth:** Argon2 password hashing, JWT via `jsonwebtoken`
- **WebSocket:** one `tokio::sync::broadcast` channel per room, stored in `AppState.rooms: RoomMap`
- **Migrations:** SQLx migrate (`sqlx::migrate!("./migrations")`) — runs on startup automatically
- **CORS:** `tower_http::cors::CorsLayer`, configured via `ALLOWED_ORIGIN` env var

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

Three pages: `index.html` (login/register), `rooms.html` (room list), `chat.html` (WebSocket chat). No framework — vanilla JS. API base URL and WebSocket URL are in `config.js`.

`config.js` indeholder produktions-URL'erne (`api.gihc.online`). Til lokal udvikling skiftes til `http://localhost:8001` og `ws://localhost:8001`.

### Ansible (`ansible/`)

Provisioner og deployer backend til VPS (65.109.233.92).

```bash
# Første gang
ansible-galaxy collection install -r ansible/requirements.yml

# Deploy
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml --ask-vault-pass
```

- `group_vars/all/vars.yml` — ikke-hemmelige variable (domæne, e-mail osv.)
- `group_vars/all/vault.yml` — krypterede hemmeligheder (postgres password, JWT secret)
- `templates/Caddyfile.j2` — Caddyfile renderet med domæne (Caddy understøtter ikke `{env.VAR}` i site-adresser)
- `templates/env.j2` — `.env` fil til Docker Compose

## Running locally

```bash
cp .env.example .env
# Sæt POSTGRES_PASSWORD og JWT_SECRET
docker compose up --build
```

`docker-compose.override.yml` bruges automatisk lokalt: HTTP-only Caddy, chat eksponeret på port 8001.

I produktion (ingen override):
```bash
docker compose -f docker-compose.yml up -d --build
```

Rust-imaget bruger two-stage build (`rust:1-slim` builder, `debian:bookworm-slim` runtime). Første build er langsom pga. dependency-kompilering.

### Deploying frontend til IPFS

```bash
# Opdater config.js til produktions-URL'er
ipfs add -r frontend/
# Opdater TXT-posten _dnslink.app.gihc.online hos simply.com:
# dnslink=/ipfs/<nyt CID>
```

## Key decisions

- **Axum over Actix-web:** better async ergonomics, clean extractor model
- **SQLx runtime API** (not `query!` macro): avoids needing `DATABASE_URL` at compile time inside Docker
- **Broadcast channel per room:** simple in-memory fan-out; restarting the server drops active connections (acceptable)
- **DNSLink on frontend domain:** gives a single stable `ALLOWED_ORIGIN` instead of a wildcard in production
- **Ansible templates Caddyfile:** Caddy does not support `{env.VAR}` in site addresses, so the domain is rendered by Ansible at deploy time
