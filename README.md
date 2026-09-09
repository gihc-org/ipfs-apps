# Loft

WebRTC link-rum — opret et loft, del linket over Matrix, XMPP eller mail, og
lad andre joine med lyd, video og skærmdeling. Ingen konti, ingen app-install:
linket er adgangsnøglen, og deltageren vælger selv et navn.

> Status: under refokusering fra en generel chat-app til Loft samt migrering
> fra Caddy + Docker Compose til k3s. Backend (M1) er implementeret og testet;
> frontend (M2) er ikke påbegyndt. Se [MIGRATION.md](MIGRATION.md) og
> [TODO.md](TODO.md).

## Koncept

- **Plan B-modellen:** et loft lever videre efter at skaberen går, indtil det
  lukkes eller udløber via TTL (default 168 timer uden aktivitet).
- **Gæsteadgang:** ingen brugerkonti, JWT, email eller CAPTCHA. Et loft-id
  (tilfældig UUID) er tilstrækkelig adgangsnøgle — del det kun med dem der skal
  ind.
- **Matrix/XMPP/mail er bare transport:** lofts deles som URL'er. Der er
  ingen protokol-integration i MVP; `navigator.share`, copy-link og QR dækker
  deling.
- **Minimal data:** databasen gemmer kun lofts (navn + tidsstempler).
  Deltagere, presence og signalering lever i hukommelsen → præcis 1 replica.

## Arkitektur

```
Browser (loft.gihc.online — frontend + /v1 + WS på samme origin)
        │
        └── k3s ingress-nginx ──► loft-api (Axum, Rust) ──► PostgreSQL
                                  │        │
                                  │        └── (lofts-tabel, TTL-cleanup)
                                  └── coturn (hostNetwork: TURN til WebRTC)
```

Media går direkte browser-til-browser (mesh); serveren relayer kun signaler og
ved aldrig, hvad der tales/ses. TURN (coturn) bruges kun når direkte forbindelse
fejler bag NAT.

## Repo-struktur

```
frontend/            Statisk frontend (legacy chat-sider indtil M2)
chat/                Rust/Axum-backend (Loft-kernen)
k8s/test/            Manifester til test-miljø (loft-test)
scripts/             DNS-record og fremtidige driftsscripts
adr-drafts/          ADR-udkast (0027 link-rum/k3s, 0028 mesh)
MIGRATION.md         Migrerings- og refokeringsplan
TODO.md              Backlog (M0–M5)
```

## API

| Metode | Sti | Beskrivelse |
|--------|-----|-------------|
| GET | `/healthz` | Liveness til k8s-probes |
| POST | `/v1/lofts` | Opret loft → `{ id, name, url }` (rate-limited) |
| GET | `/v1/lofts/:id` | Metadata (findes loftet?) — 404 hvis væk |
| WS | `/v1/ws/:loft_id` | WebSocket: join/roster/signalering |

Oprettelse:

```bash
curl -X POST https://<origin>/v1/lofts \
  -H 'Content-Type: application/json' \
  -d '{"name": "Morgenmøde"}'
# → {"id":"…","name":"Morgenmøde","url":"/loft.html?id=…"}
```

### WebSocket-protokol

Klienten forbinder til `/v1/ws/:loft_id` og sender `join` som første besked:

```json
{"type": "join", "name": "Alice"}
```

Serveren svarer med `roster` (fuld deltagerliste) og broadcastes `join`/
`leave` til loftet:

```json
{"type": "roster",
 "self": {"id": "…", "name": "Alice"},
 "participants": [{"id": "…", "name": "Alice"}]}
{"type": "join", "participant": {"id": "…", "name": "Alice"}}
{"type": "leave", "participant": {"id": "…", "name": "Alice"}}
```

WebRTC-signaler sendes med `to` (deltager-id). Serveren validerer modtageren
og sætter altid `from` selv — klienter kan ikke forfalske afsender:

```json
{"type": "signal", "to": "<deltager-id>", "signal": {"type": "offer", "sdp": "…"}}
```

Andre beskedtyper (fx `media-state` med `audio`/`video`/`screen`-flag) relayes
til loftet med server-stemplet `from`. Én deltager pr. WebSocket-forbindelse;
`roster` er kilden til sandheden (inkl. `self` så klienten kender sit eget id),
og klienter fjerner efterladte peers via `leave`.

## Lokal udvikling

Forudsætninger: Rust, en kørende PostgreSQL (fx via podman/docker).

```bash
# 1. PostgreSQL
podman run -d --name loft-dev-pg \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  docker.io/library/postgres:16-alpine

# 2. Miljø (justér database-url hvis nødvendigt)
cp .env.example .env

# 3. Kør backend — migrationer kører automatisk ved start
cd chat
DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo run
```

Backenden lytter på `0.0.0.0:8080`. Frontend-siderne i `frontend/` er stadig
de gamle chat-sider (M2 erstatter dem); til lokal afprøvning skal
`frontend/config.js` pege på `http://localhost:8080` (API/WS).

## Test

```bash
# Enhedstests (ingen database)
cd chat && cargo test --lib

# Integrationstests (kræver PostgreSQL-superuser uden databasenavn)
cd chat && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo test
```

`#[sqlx::test]` opretter en midlertidig database pr. test og dropper den igen.

## Deploy (k3s)

Planen står i [MIGRATION.md](MIGRATION.md). Kort fortalt:

- Domæner: `loft.test.gihc.online` (test), `loft.gihc.online` (prod) — ét
  origin pr. miljø, så CORS/CSP-par-koblingen udgår.
- Images: `ghcr.io/gihc-org/loft` (API) og `ghcr.io/gihc-org/loft-web`
  (frontend), bygget i GitHub Actions med SHA-tags.
- Manifester: [k8s/test/](k8s/test/README.md) — namespace `loft-test`, postgres
  + PVC, api/web, coturn (`hostNetwork`), ingress med WS-timeouts.
- Secrets: `pass` → `kubectl create secret loft-secrets` (ingen hemmeligheder
  i git). `TURN_SECRET` er reelt offentlig (HMAC, 24 t TTL) og ligger i
  ConfigMap for test.

## Sikkerhed og GDPR

- Ingen konti/emails → ingen persondata i databasen; deltagernavne findes kun i
  hukommelsen under sessionen.
- Rate limiting på loft-oprettelse (tower_governor, XFF-baseret).
- Containerhærdning: non-root (uid 10001), read-only rootfs i k8s, `drop:
  ALL`.
- Signal-beskeder gemmes aldrig; mediestrømme er P2P og DTLS-krypterede.

## Roadmap

Se [TODO.md](TODO.md):

- **M1 (færdig):** backend — lofts, WS-protokol, /healthz, TTL, non-root image
- **M2:** frontend — `loft.html` + `rtc.js`-mesh, deling/invite
- **M3:** Playwright-e2e + CI
- **M4:** k3s-deploy af test-miljø
- **M5:** oprydning af chat-stak (compose/ansible/caddy/IPFS) og gamle domæner
