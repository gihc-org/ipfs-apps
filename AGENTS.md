# AGENTS.md

This file provides guidance to AI coding agents working in this repository.

## Generelle guidelines

@.guidelines/security.md
@.guidelines/rust-axum.md
@.guidelines/webrtc.md
@.guidelines/web-frontend.md
@.guidelines/testing-and-docs.md
@.guidelines/process.md

## Workflow

`scripts/check.sh` kører semantisk review af staged ændringer mod guidelines og ADRs ovenfor (`.guidelines` er et soft-link til `~/projects/guidelines`). Det er tænkt som pre-commit hook ved hvert commit — hooket er ikke installeret i dette checkout, så kør det manuelt indtil da.

Kør manuelt: `bash scripts/check.sh`

## Nuværende tilstand (2026-09-16)

Loft er etableret: WebRTC link-rum med gæsteadgang, kørende på k3s i både test
(`loft.test.gihc.online`, namespace `loft-test`) og prod (`loft.gihc.online`,
namespace `loft-prod`, pinnet CI-SHA `d082905…`, cert fra `letsencrypt-prod`).
M0–M4 og prod-deployet er færdige, og M5-oprydningen er gennemført i repoet:
`chat/` hedder nu `loft/`, og compose-, ansible-, caddy- og IPFS-resterne er
slettet. ADR 0027 og 0028 er accepteret og flyttet til `~/projects/adrs/`.
Tilbage er Android-lyd for **talende** brugere (kendt platformsegenskab, se
[TODO.md](TODO.md)).

Den gamle chat-arkitektur (konti/JWT, DM, filoverførsel, Caddy + Docker Compose,
IPFS/DNSLink) er ude af den aktive sti; historikken står i
[MIGRATION.md](MIGRATION.md).

Læs først: [README.md](README.md), [MIGRATION.md](MIGRATION.md),
[TODO.md](TODO.md), [runbooks/loft-deploy.md](runbooks/loft-deploy.md) og
Loft-ADR'erne i `~/projects/adrs/` (0027 og 0028).

## Project Overview

Man opretter et loft, deler URL'en (Matrix/XMPP/mail), og modtageren joiner med
lyd, video og skærmdeling uden konto. Loft-id'et er adgangsnøglen, og
deltageren vælger selv et vist navn. Et loft lever videre efter at skaberen går,
indtil det lukkes af ejeren eller udløber via TTL.

```
frontend/          Statisk frontend (nginx-image): loft.html + rtc.js
loft/              Rust (Axum) backend — REST + WebSocket (omdøbt fra chat/ i M5)
e2e/               Playwright-tests (lokal backend eller deployet miljø)
k8s/test/          Manifester for loft.test.gihc.online (namespace loft-test)
k8s/prod/          Manifester for loft.gihc.online (namespace loft-prod, SHA-pin)
scripts/           DNS-record, smoke-test (REST+WS+DB), semantisk review
runbooks/          Drift, fejlfinding og deploy
.github/workflows/ CI: bygger begge images og kører e2e på Chromium
```

Arkitekturbeslutninger ligger i det delte ADR-repo (`~/projects/adrs/`) — for
Loft især 0027 (link-rum/k3s) og 0028 (mesh).

### Arkitektur

```
Browser (loft.gihc.online — frontend, /v1 og WS på samme origin)
    │
    └── ingress-nginx ──► loft-api (Axum, Rust) ──► PostgreSQL (kun lofts-tabellen)
                              │
                              └── in-memory deltagere/presence → præcis 1 replica

Browser ◄──── WebRTC mesh (lyd/video/skærm, P2P) ────► Browser
    └── turn.gihc.online (platformens delte coturn, se ~/projects/infra ADR 0003)
```

Serveren relayer kun signaler og ved aldrig, hvad der tales/ses. TURN bruges kun,
når en direkte forbindelse fejler bag NAT.

### Backend (`loft/`)

- **Framework:** Axum 0.7 med `ws`-feature; `tower_governor` til rate limiting på loft-oprettelse
- **Database:** SQLx 0.8 + PostgreSQL (runtime API, ikke compile-time `query!` — ADR 0002)
- **Migrationer:** `sqlx::migrate!("./migrations")` — embedded ved compile time, kører ved start
- **WebSocket:** én `tokio::sync::broadcast`-kanal pr. loft (ADR 0006)
- **TLS/HTTP:** rustls, ingen OpenSSL-afhængighed (ADR 0003)
- **CORS:** `ALLOWED_ORIGIN` er kun relevant ved lokal kørsel — i k8s deler frontend, `/v1` og WS origin

#### Crate-struktur

`loft/src/lib.rs` er biblioteksroden og ejer module erklæringer samt `AppState`,
`Config`, `LoftRegistry`, `LoftChannelMap`, `build_app` og
`cleanup_expired_lofts`; `src/main.rs` er en tynd binær indgang, der læser
`PORT` og kalder `build_app`. Splittet gør integrationstests mulige (ADR 0004).
Kraten og binæren hedder `loft`.

| Modul | Formål |
|-------|--------|
| `config` | env-konfiguration: `DATABASE_URL`, `ALLOWED_ORIGIN`, `LOFT_TTL_HOURS` |
| `models` | `Loft`-rækken (id, navn, created_at, last_active) |
| `routes/health` | `/healthz` til k8s-probes |
| `routes/lofts` | `POST`/`GET`/`DELETE /v1/lofts`, `cleanup_expired_lofts` og `ws_handler` |
| `state` | in-memory loft-registry: broadcast-kanaler og deltagere (unit-tests her) |

#### API-flade

| Metode | Sti | Auth |
|--------|-----|------|
| GET | `/healthz` | — |
| POST | `/v1/lofts` | — (rate-limited) |
| GET | `/v1/lofts/:id` | — |
| DELETE | `/v1/lofts/:id` | `X-Owner-Token` |
| WS | `/v1/ws/:loft_id` | loft-id'et i stien |

Der er ingen konti: loft-id'et (tilfældig UUID) er capability. `owner_token`
returneres kun ved oprettelse, lækkes ikke af `GET`, og bruges til at lukke
loftet. WebSocket-protokollen (`join`, `roster`, `join`/`leave`, `media-state`,
`signal` med server-stemplet `from`, `closed`) er beskrevet i
[README.md](README.md).

#### Miljøvariabler

| Variabel | Default | Formål |
|----------|---------|--------|
| `DATABASE_URL` | required | PostgreSQL-forbindelse |
| `ALLOWED_ORIGIN` | `*` | CORS — kun relevant ved lokal kørsel |
| `LOFT_TTL_HOURS` | `168` | TTL før inaktive lofts ryddes |
| `PORT` | `8080` | Lytte-port (lokal udvikling bruger 8081, se README) |

Hemmeligheder kommer fra `pass` og oprettes som k8s-secretet `loft-secrets`
(`postgres-password`, `database-url`) — aldrig i git.

#### Tests

```bash
# Enhedstests (ingen database)
cd loft && cargo test --lib

# Integrationstests (kræver en kørende PostgreSQL)
cd loft && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo test
```

Unit-testene ligger i `loft/src/state.rs` (broadcast-kanaler pr. loft).
Integrationstestene i `loft/tests/api.rs` bruger `#[sqlx::test]`, som opretter og
river en midlertidig database ned pr. test. **Cargo-testene kører ikke i CI** —
CI-jobbet dækker kun e2e på Chromium, så kør dem manuelt.

### Frontend (`frontend/`)

`loft.html` (lobby + rum), `rtc.js` (mesh med perfect negotiation — ADR-draft
0028) og `audio-debug.html` (telefon-diagnostik). Ingen framework — vanilla JS.
`index.html` redirecter til `loft.html` og bevarer query-strengen.
`config.js` indeholder API-, WS- og TURN-URL'er og **overskrives i k8s** af
ConfigMap'en (`k8s/{test,prod}/configmap.yaml`); repo-filen gælder kun lokalt.

Mikrofonen starter **ikke** ved join ("join muted"), og "Sluk mikrofon" frigiver
capture helt (`track.stop()` og sporet fjernes fra peer-forbindelserne): et
aktivt mikrofon-spor flytter hele telefonens medierute til opkaldsprofil. På
Android findes ingen output-device-API, så en **talende** bruger får modpartens
lyd i telefonens højttaler — det kan frontenden ikke omgå. Afbøderinger og
målinger står i [README.md](README.md) og [TODO.md](TODO.md).

### Deploy (k3s)

- Images: `ghcr.io/gihc-org/loft` (API) og `ghcr.io/gihc-org/loft-web`
  (frontend), bygget af `.github/workflows/build.yml` ved push til `trunk`
  (SHA-tags + `latest`).
- Manifester: `k8s/test/` (image `:latest`, `Always`) og `k8s/prod/` (pinnet
  SHA-tag, `IfNotPresent`). Apply-rækkefølge, cert-skift (staging → verificér →
  `letsencrypt-prod`) og rollback står i
  [runbooks/loft-deploy.md](runbooks/loft-deploy.md).
- Verifikation: `scripts/smoke-test.sh <origin> --namespace <ns>` dækker REST,
  WebSocket, lukning med ejer-nøgle og at DB-rækken er væk. Playwright kører mod
  deployet miljø med `npm run test:test` (test), `test:beta` (prod) og
  `test:test:firefox`.
- TURN er platformens (`turn.gihc.online`): `config.js` rendres med
  `~/projects/infra/scripts/turn-config.sh --config-js`, og firewall/kvoter ejes
  også af infra-repoet.

## Key decisions

Arkitektoniske beslutninger er dokumenteret som ADR'er i `~/projects/adrs/`.
Beslutninger der stadig gælder for Loft:

| ADR | Beslutning |
|-----|------------|
| [0001](~/projects/adrs/0001-axum-over-actix.md) | Axum over Actix-web |
| [0002](~/projects/adrs/0002-sqlx-runtime-api.md) | SQLx runtime API over compile-time macros |
| [0003](~/projects/adrs/0003-rustls-over-native-tls.md) | rustls over native-tls |
| [0004](~/projects/adrs/0004-lib-bin-split.md) | lib + bin split til integration tests |
| [0005](~/projects/adrs/0005-authenticate-plain-async-fn.md) | Plain async fn frem for extractor |
| [0006](~/projects/adrs/0006-broadcast-channel-per-room.md) | Broadcast channel per loft |
| [0012](~/projects/adrs/0012-feature-flags-via-empty-env.md) | Tom env-variabel som feature flag |
| [0013](~/projects/adrs/0013-owasp-by-default.md) | OWASP-sikkerhed som del af definition of done |
| [0014](~/projects/adrs/0014-gdpr-principles.md) | GDPR-principper |
| [0015](~/projects/adrs/0015-gdpr-deletion-pattern.md) | Slettestrategi og post-deploy smoke test |
| [0024](~/projects/adrs/0024-cis-docker-benchmark.md) | CIS Docker Benchmark (non-root, read-only fs) |
| [0025](~/projects/adrs/0025-ipfs-apps-smoke-test.md) | Smoke test af deployet miljø |
| [0026](~/projects/adrs/0026-logging-and-secrets.md) | Logging og secrets |
| [0027](~/projects/adrs/0027-loft-link-rooms.md) | Loft: link-rum med gæsteadgang på k3s — afløser 0008–0010 og 0019 |
| [0028](~/projects/adrs/0028-loft-group-mesh.md) | Loft: mesh-topologi og per-deltager-signalering — afløser 0016 og 0018 |

### Projekt-specifikke detaljer

- **TURN ligger i platformen:** der kan kun køre én coturn pr. node (to
  instanser binder begge 3478 via `SO_REUSEPORT` og fordeler trafikken
  tilfældigt). Firewall og misbrugsbeskyttelse ejes af `~/projects/infra`
  (ADR 0003 der), og appen har ingen egen coturn.
- **`TURN_SECRET` er reelt offentlig:** den udleveres til browseren. Modvægten er
  coturns kvoter og `--denied-peer-ip`, ikke hemmeligholdelse. Følg-op: udsted
  kortlivede credentials fra API'et.
- **Én replica:** lofts, deltagere og WebSocket-kanaler er in-memory. `Recreate`
  bruges ved deploy, fordi PVC'en er ReadWriteOnce og rolling updates ville
  droppe WS-forbindelser — frontendens auto-reconnect dækker afbrydelsen.
- **WS-timeouts:** ingress-nginx har `proxy-read-timeout`/`proxy-send-timeout` =
  3600 på WebSocket-ruten.
- **Ingress-routing:** `/healthz` har sin egen rute (ellers svarer
  web-frontenden 404); `/v1` og `/` routes uden path-rewrites.
- **`TEST_API_URL` uden `/v1`** i e2e, og `BASE_URL` peger på frontenden.
- **Simply.com DNS API:** `scripts/create-dns-record.sh` kalder
  `https://api.simply.com/2/` med HTTP Basic Auth (konto + nøgle fra `pass`).
