# Loft

WebRTC link-rum — opret et loft, del linket over Matrix, XMPP eller mail, og
lad andre joine med lyd, video og skærmdeling. Ingen konti, ingen app-install:
linket er adgangsnøglen, og deltageren vælger selv et navn.

> Status: under refokusering fra en generel chat-app til Loft samt migrering
> fra Caddy + Docker Compose til k3s. M1–M3 er færdige (backend, frontend og
> Playwright-e2e) og testet lokalt; M4 — deploy af testmiljøet — er i gang. Se
> [MIGRATION.md](MIGRATION.md), [TODO.md](TODO.md) og
> [runbooks/loft-deploy.md](runbooks/loft-deploy.md).

## Koncept

- **Plan B-modellen:** et loft lever videre efter at skaberen går, indtil det
  lukkes af ejeren eller udløber via TTL (default 168 timer uden aktivitet).
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
                                  └── turn.gihc.online (platformens delte coturn)
```

Media går direkte browser-til-browser (mesh); serveren relayer kun signaler og
ved aldrig, hvad der tales/ses. TURN bruges kun når direkte forbindelse fejler
bag NAT — og den instans er platformens (`turn.gihc.online`), ikke appens, fordi
der kun kan køre én coturn pr. node.

## Repo-struktur

```
frontend/            Statisk frontend (loft.html + rtc.js; legacy chat-sider til M5)
chat/                Rust/Axum-backend (Loft-kernen)
e2e/                 Playwright-tests (mod lokal backend eller deployet miljø)
k8s/test/            Manifester til test-miljø (loft-test)
k8s/prod/            Manifester til prod (forberedt, ikke deployet)
scripts/             DNS-record, smoke-test og fremtidige driftsscripts
adr-drafts/          ADR-udkast (0027 link-rum/k3s, 0028 mesh)
MIGRATION.md         Migrerings- og refokeringsplan
TODO.md              Backlog (M0–M5)
```

`ansible/`, `caddy/`, `docker-compose*.yml` og `.woodpecker.yaml` er rester af
Caddy/compose-tiden og slettes i M5 — flowet dengang er beskrevet i
[MIGRATION.md](MIGRATION.md) under "Historisk: sådan deployede vi før GHCR".

## API

| Metode | Sti | Beskrivelse |
|--------|-----|-------------|
| GET | `/healthz` | Liveness til k8s-probes |
| POST | `/v1/lofts` | Opret loft → `{ id, name, url }` (rate-limited) |
| GET | `/v1/lofts/:id` | Metadata (findes loftet?) — 404 hvis væk |
| DELETE | `/v1/lofts/:id` | Luk loftet permanent — kræver `X-Owner-Token` |
| WS | `/v1/ws/:loft_id` | WebSocket: join/roster/signalering |

Oprettelse:

```bash
curl -X POST https://<origin>/v1/lofts \
  -H 'Content-Type: application/json' \
  -d '{"name": "Morgenmøde"}'
# → {"id":"…","name":"Morgenmøde","url":"/loft.html?id=…","owner_token":"…"}
```

`owner_token` er kun til skaberen og gemmes i browseren (localStorage) — den
indgår aldrig i det link du deler, og GET lækker den ikke. Lukning:

```bash
curl -X DELETE https://<origin>/v1/lofts/<id> \
  -H 'X-Owner-Token: <owner_token>'
```

Aktivt forbundne klienter modtager `{"type":"closed"}` og vender tilbage til
lobbyen.

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
DATABASE_URL=postgres://postgres:postgres@localhost:5432 PORT=8081 cargo run
```

Backenden lytter på `0.0.0.0:$PORT` — brug **8081** lokalt, fordi en kørende
IPFS-daemon (kubo) har sin gateway på `127.0.0.1:8080` som standard og dermed
holder porten. `frontend/config.js` skal pege på samme port
(`http://localhost:8081` og `ws://localhost:8081`); peger den på 8080, rammer
browseren IPFS-gatewayen i stedet for API'et og får 404/400.

## Test

```bash
# Enhedstests (ingen database)
cd chat && cargo test --lib

# Integrationstests (kræver PostgreSQL-superuser uden databasenavn)
cd chat && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo test

# E2e (kræver kørende backend)
cd e2e && TEST_API_URL=http://localhost:8081 npx playwright test

# E2e i Firefox (kræver `npx playwright install firefox` én gang; CI kører kun
# Chromium). Brugeren kører Firefox på alle enheder, så den holdes ved lige.
cd e2e && npm run test:test:firefox

# Smoke-test af et deployet miljø (REST + WebSocket + DB-rækken væk)
./scripts/smoke-test.sh https://loft.test.gihc.online --namespace loft-test

# TURN-relay mod et deployet miljø (relay-only ICE gennem platformens coturn)
cd e2e && TURN_URL="$(~/projects/infra/scripts/turn-config.sh --url)" \
  TURN_SECRET="$(pass turn/static-auth-secret)" \
  BASE_URL=https://loft.test.gihc.online npx playwright test tests/turn.spec.ts
```

`#[sqlx::test]` opretter en midlertidig database pr. test og dropper den igen.

### Lyd-routing på telefoner

Åbent fund fra accepttesten 2026-09-12: WebRTC-lyden gik ud af telefonens
højttaler i stedet for Bluetooth-earpluggene (podcast/medieafspilning går fint
i earpluggene). `frontend/audio-debug.html` er diagnostikværktøjet til sagen —
åbn det på telefonen (fx `https://loft.test.gihc.online/audio-debug.html`). Siden
laver sin egen `RTCPeerConnection`-loopback, så fjernlyden går gennem samme sti
som i Loft, og den kan: liste enheder, spille en tone (med/uden videospor),
slukke/frigive mikrofonen, skifte output-enhed via `setSinkId` og skrive en
JSON-rapport til copy/paste. Planen og de verificerede desktop-fund står i
[TODO.md](TODO.md#lyd-routing-på-telefoner-fundet-2026-09-12).

Loft har selv en "Lyd ud"-vælger i værktøjslinjen; den vises kun når browseren
både har `HTMLMediaElement.setSinkId` og eksponerer mindst én `audiooutput`-enhed
(desktop-Firefox viser dem først efter mikrofon-tilladelse, Android typisk slet
ikke). Valget gemmes i `localStorage` under `loft.sinkId`.

På Android, hvor der ikke findes noget output-vælger-API (se TODO), er
mekanismen i stedet adfærd: **"Sluk mikrofon" frigiver capture helt**
(`track.stop()` og sporet fjernes fra peer-forbindelserne), fordi Android holder
audio-mode i kommunikationstilstand så længe et mikrofon-spor er aktivt. Målingen
bag står i [TODO.md](TODO.md#telefon-måling-2026-09-13-android-15-firefox-155).

Af samme grund **starter mikrofonen ikke ved join** ("join muted"): et aktivt
capture flytter hele telefonens medierute til opkaldsprofil — også for andre
apps (en podcast bliver tavs) — og fjernlyden ender i telefonens højttaler.
Mikrofonen tændes med **🎙 Tænd mikrofon**, og et tryk på **Sluk mikrofon**
frigiver sporet igen, hvorefter lyden er tilbage i earpluggene. Mens man taler,
vælger Android stadig kommunikationsruten; det er en platformsegenskab, ikke
noget frontenden kan omgå (Firefox sætter ikke Bluetooth-headsettet som
communication device).

```bash
# Kun diagnostik-tests (ingen backend nødvendig)
cd e2e && npx playwright test tests/audio-debug.spec.ts
cd e2e && PW_FIREFOX=1 npx playwright test tests/audio-debug.spec.ts --project=firefox
```

### Manuel test på én maskine

Loft er bygget til at testes med to vinduer på samme maskine, men der er to
fælder:

- **Feedback:** to vinduer med højttalere + mikrofon i samme rum skaber en
  lydsløjfe (konstant summen/hylen), uanset hvor god mikrofonen er. Brug
  headset som input (eller helt luk højttalerne af) under testen. Headset som
  input og højttalere som output fungerer fint.
- **Forskellige deltagere:** vinduer i samme browser-profil deler localStorage
  (navn, ejer-nøgle, tilladelser). Åbn vindue 2 i et inkognitovindue eller en
  anden browser, så I kan optræde som to forskellige deltagere.

Den støj- og tilladelsesfri test kommer fra Playwright-e2e med falske medier —
se `e2e/`.

## Deploy (k3s)

Planen står i [MIGRATION.md](MIGRATION.md). Kort fortalt:

- Domæner: `loft.test.gihc.online` (test), `loft.gihc.online` (prod) — ét
  origin pr. miljø, så CORS/CSP-par-koblingen udgår.
- Images: `ghcr.io/gihc-org/loft` (API) og `ghcr.io/gihc-org/loft-web`
  (frontend), bygget i GitHub Actions med SHA-tags.
- Manifester: [k8s/test/](k8s/test/README.md) — namespace `loft-test`, postgres
  + PVC, api/web og ingress med WS-timeouts. TURN ligger i platformen
  (`~/projects/infra`, namespace `coturn`).
- Prod-manifester (forberedt, ikke deployet): [k8s/prod/](k8s/prod/README.md).
- Trin-for-trin: [runbooks/loft-deploy.md](runbooks/loft-deploy.md) — DNS,
  secret, apply, cert (staging → prod), smoke-test, rollback.
- Secrets: `pass` → `kubectl create secret loft-secrets` (ingen hemmeligheder
  i git). `TURN_SECRET` kommer fra platformens `pass turn/static-auth-secret`
  (reelt offentlig, HMAC-baserede credentials) og rendres ind i `config.js`.
- Historisk: indtil M4 blev image'et bygget på selve VPS'en af
  `docker compose … up -d --build` (intet registry) og frontenden lagt på
  IPFS bag DNSLink. M4 er derfor første gang der bygges og publiceres images i
  CI — se "Historisk: sådan deployede vi før GHCR" i
  [MIGRATION.md](MIGRATION.md).

## Sikkerhed og GDPR

- Ingen konti/emails → ingen persondata i databasen; deltagernavne findes kun i
  hukommelsen under sessionen.
- Rate limiting på loft-oprettelse (tower_governor, XFF-baseret).
- Containerhærdning: API kører non-root (uid 10001) med read-only rootfs;
  platformens coturn kører som `nobody` med read-only rootfs, `no_new_privs` og
  kun `NET_BIND_SERVICE` i bounding-settet.
- TURN er eksponeret mod internettet, og `TURN_SECRET` er offentlig (den ligger
  i frontendens `config.js`). Modvægten er platformens `--denied-peer-ip` for
  RFC1918/loopback/link-local/CGNAT og kvoter pr. session og i alt.
- Signal-beskeder gemmes aldrig; mediestrømme er P2P og DTLS-krypterede.

## Roadmap

Se [TODO.md](TODO.md):

- **M1 (færdig):** backend — lofts, WS-protokol, /healthz, TTL, non-root image
- **M2 (færdig):** frontend — `loft.html` + `rtc.js`-mesh, deling/invite
- **M3 (færdig):** Playwright-e2e + CI
- **M4 (færdig):** k3s-deploy af test-miljø — inkl. accepteret relay-sti mod
  platformens delte TURN (`turn.gihc.online`)
- **M5:** oprydning af chat-stak (compose/ansible/caddy/IPFS) og gamle domæner
- **Åbent:** lyd-routing på telefoner (Bluetooth-earplugs vs. højttaler —
  diagnostikside og `setSinkId`-vælger er klar, selve telefon-testen mangler) og
  prod-deploy — se [TODO.md](TODO.md)
