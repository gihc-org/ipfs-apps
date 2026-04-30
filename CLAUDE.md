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
IPFS frontend (DNSLink → chat.apps.gihc.online)
    │
    └── api.gihc.online  →  Caddy  →  chat:8001 (Axum, Rust)
                                           │
                                       PostgreSQL
```

### Chat backend (`chat/`)

- **Framework:** Axum 0.7 with `ws` feature
- **Database:** SQLx 0.8 + PostgreSQL (runtime API, not compile-time `query!` macro)
- **Auth:** Argon2id password hashing, JWT via `jsonwebtoken` (HS256)
- **Email verification:** Required before login — Resend API sends a link to `GET /auth/verify?token=<uuid>`
- **CAPTCHA:** Cloudflare Turnstile on registration — `chat/src/captcha.rs` validates against `https://challenges.cloudflare.com/turnstile/v0/siteverify` (v0, not v1)
- **External HTTP:** `reqwest 0.12` with `rustls-tls` feature — client stored in `AppState.http`
- **WebSocket:** one `tokio::sync::broadcast` channel per room, stored in `AppState.rooms: RoomMap`
- **Migrations:** `sqlx::migrate!("./migrations")` — embedded at compile time, run on startup
- **CORS:** `tower_http::cors::CorsLayer`, configured via `ALLOWED_ORIGIN` env var
- **TLS:** uses `rustls` (not `native-tls`) — no OpenSSL dependency needed to compile

#### Crate structure

`chat/src/lib.rs` is the library root — it owns all module declarations and exports `AppState`, `Config`, `RoomMap`, `build_app`, and `build_cors`. `src/main.rs` is a thin binary entrypoint that calls into the lib. This split lets integration tests import the crate.

#### Modules

| Module | Purpose |
|--------|---------|
| `auth` | JWT creation/validation, Argon2id hashing, `authenticate()` helper |
| `captcha` | Turnstile token validation (skipped if `TURNSTILE_SECRET` is empty) |
| `email` | Resend API wrapper for verification emails (skipped if `RESEND_API_KEY` is empty) |
| `models` | `User` struct with email verification fields |
| `routes/auth` | register, login, me, verify handlers |
| `routes/rooms` | room list and creation |
| `ws` | WebSocket handler |

#### API surface
| Method | Path | Auth |
|--------|------|------|
| POST | /auth/register | — |
| POST | /auth/token | — |
| GET | /auth/me | Bearer |
| GET | /auth/verify?token=\<uuid\> | — |
| GET | /rooms | Bearer |
| POST | /rooms | Bearer |
| GET | /rooms/:id/messages | Bearer |
| WS | /ws/:room_id?token=\<jwt\> | query param |

#### Auth
`auth::authenticate(&state, &headers).await?` validates the Bearer token and returns the `User`. Call it at the top of any handler that requires authentication.

The WebSocket handler uses a query param (`?token=<jwt>`) instead of a header because the browser WebSocket API cannot set custom headers on the upgrade request.

#### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `JWT_SECRET` | required | HMAC key for JWT signing |
| `ALLOWED_ORIGIN` | `*` | CORS allowed origin |
| `TURNSTILE_SECRET` | `` | Cloudflare Turnstile secret key (empty = skip CAPTCHA) |
| `RESEND_API_KEY` | `` | Resend email API key (empty = auto-verify accounts) |
| `RESEND_FROM` | `noreply@example.com` | Sender address for verification emails |
| `BASE_URL` | `http://localhost:8001` | Public URL of the API (used in email links) |
| `FRONTEND_URL` | `http://localhost:8001` | Public URL of the frontend (used in redirect after email verify) |

#### Tests

```bash
# Unit tests (no database needed)
cargo test --lib

# Integration tests (requires a running PostgreSQL instance)
DATABASE_URL=postgres://user:password@localhost:5432 cargo test
```

Unit tests live in `src/auth.rs` (JWT, hashing) and `src/ws.rs` (broadcast channels).
Integration tests in `tests/api.rs` use `#[sqlx::test]` which creates and tears down a temporary database per test.

### Frontend (`frontend/`)

Three pages: `index.html` (login/register), `rooms.html` (room list), `chat.html` (WebSocket chat). No framework — vanilla JS. API base URL and WebSocket URL are in `config.js`.

`config.js` indeholder produktions-URL'erne (`api.gihc.online`). Til lokal udvikling skiftes til `http://localhost:8001` og `ws://localhost:8001`.

Turnstile widget bruger `https://challenges.cloudflare.com/turnstile/v0/api.js` (v0). Submit-knappen er disabled indtil Turnstile `callback` fyrer — undgår race condition ved async script-load.

### Ansible (`ansible/`)

Provisioner og deployer backend til VPS (65.109.233.92). Deploy er fuldt automatiseret — inkl. IPFS-upload og DNS-opdatering hos Simply.com.

```bash
# Første gang
ansible-galaxy collection install -r ansible/requirements.yml

# Deploy
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml --ask-vault-pass
```

Playbook'en:
1. Synkroniserer projektet til VPS
2. Bygger og starter Docker Compose-services
3. Uploader `frontend/` til IPFS og gemmer CID
4. Kalder Simply.com API og opdaterer `_dnslink.chat.apps.gihc.online` automatisk

- `group_vars/all/vars.yml` — ikke-hemmelige variable (domæne, e-mail, simply_account osv.)
- `group_vars/all/vault.yml` — krypterede hemmeligheder: `vault_postgres_password`, `vault_jwt_secret`, `vault_turnstile_secret`, `vault_resend_api_key`, `vault_simply_account`, `vault_simply_api_key`
- `templates/Caddyfile.j2` — Caddyfile renderet med domæne (Caddy understøtter ikke `{env.VAR}` i site-adresser)
- `templates/env.j2` — `.env` fil til Docker Compose

#### Simply.com DNS API
Simply.com eksponerer en REST API på `https://api.simply.com/2/` med HTTP Basic Auth (kontonummer + API-nøgle). Playbook'en bruger `uri`-modulet til at GET records, finde den eksisterende `_dnslink`-record og PUT den nye CID. Credentials hentes fra vault (`vault_simply_account`, `vault_simply_api_key`).

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

Rust-imaget bruger two-stage build (`rust:1-slim` builder, `debian:bookworm-slim` runtime). Første build er langsom pga. dependency-kompilering. Ingen OpenSSL-afhængighed — rustls compileres statisk ind.

### Deploying frontend til IPFS

Sker automatisk via Ansible-playbook. Manuelt ved lokal test:

```bash
ipfs add -r frontend/
# DNS opdateres nu automatisk af playbook'en via Simply.com API
```

## Key decisions

- **Axum over Actix-web:** better async ergonomics, clean extractor model
- **SQLx runtime API** (not `query!` macro): avoids needing `DATABASE_URL` at compile time inside Docker
- **Broadcast channel per room:** simple in-memory fan-out; restarting the server drops active connections (acceptable)
- **DNSLink on frontend domain:** gives a single stable `ALLOWED_ORIGIN` instead of a wildcard in production
- **Ansible templates Caddyfile:** Caddy does not support `{env.VAR}` in site addresses, so the domain is rendered by Ansible at deploy time
- **rustls over native-tls:** no `libssl-dev` or `pkg-config` needed to compile — simpler Dockerfile and local dev setup
- **lib + bin split:** `src/lib.rs` exports the router and state so integration tests can import the crate without duplicating setup
- **`authenticate` as plain async fn:** Rust 1.88 tightened lifetime rules for async fns in traits, breaking the `FromRequestParts` extractor approach
- **Turnstile v0 URL:** Cloudflare Turnstile uses `/v0/` paths for both `api.js` and `siteverify` — `/v1/` returnerer 404/405 og fejler lydløst
- **`reqwest` without `form` feature:** reqwest 0.12 removed the `form` feature flag — `.form()` metoden er altid tilgængelig uden at angive den
- **Simply.com API for DNS:** Ansible `uri`-modul kalder Simply.com REST API direkte — ingen ekstern Terraform-provider eller Ansible-collection nødvendig
