# TODO — Loft (tidligere ipfs-apps/chat)

Projektet fokuseres til **Loft**: WebRTC link-rum (lyd, video, skærmdeling)
med gæsteadgang, delt via link over Matrix/XMPP/mail. Samtidig migreres fra
Caddy + Docker Compose til k3s. Se [MIGRATION.md](MIGRATION.md) og ADR-drafts i
`adr-drafts/`.

Afsluttet før refokuseringen (chat-rum, DM, filoverførsel, 1:1-skærmdeling og
lydopkald) ligger i git-historikken; Playwright- og WebRTC-mønstre derfra
genbruges.

## Start-prompt til ny session

Kopér blokken herunder som første besked til agenten:

> Fortsæt arbejdet på **Loft** i branchen `feat/loft-k3s-refocus`
> (M0–M3 færdige, 2026-09-09).
>
> Læs først `README.md`, `MIGRATION.md`, `TODO.md` og ADR-drafts
> `0027-loft-link-rooms.md` + `0028-loft-group-mesh.md`. AGENTS.md's
> chat-sektioner er historiske indtil M5.
>
> Tilstand:
> - Backend (`chat/`, omdøbes til `loft/` i M5): Rust/Axum.
>   `POST /v1/lofts`, `GET /v1/lofts/:id`, `DELETE /v1/lofts/:id` med
>   `X-Owner-Token`, WS `/v1/ws/:loft_id`
>   (join/roster/signal/media-state/closed), `/healthz`, TTL-cleanup.
>   Migrationer kører automatisk ved start.
> - Frontend: `frontend/loft.html` + `frontend/rtc.js` (mesh, perfect
>   negotiation). `index.html` redirecter til loft.html.
> - k8s: `k8s/test/` (namespace `loft-test`); GitHub Actions bygger
>   `ghcr.io/gihc-org/loft`(+web) og kører e2e.
>
> Næste opgave (M4): deploy af test-miljø — A-record via
> `scripts/create-dns-record.sh`, secret `loft-secrets`, firewall i
> `~/projects/infra/tofu` (3478 + 49152–49200), apply `k8s/test/`,
> cert staging → prod. Dele af M4 kræver adgang til k3s/pass/infra-repoet —
> spørg brugeren før trin uden for repoet.
>
> Kommandoer:
> - Unit: `cd chat && cargo test --lib`
> - Integration: `cd chat && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo test`
> - E2e (backend kørende): `cd e2e && TEST_API_URL=http://localhost:8081 npx playwright test`
>
> Gotchas: `TEST_API_URL` uden `/v1`; `frontend/config.js` overskrives i k8s af
> ConfigMap; e2e bruger sessionStorage-nøglen `loft.noAutoMic`.

## M0 — Fundament (dokumentation)

- [x] Beslutning: Loft link-rum (Plan B), gæsteadgang, statisk frontend
- [x] TODO.md og MIGRATION.md omskrevet til Loft + k3s
- [x] ADR-drafts 0027 (link-rum + k3s) og 0028 (mesh-topologi)
- [x] `k8s/test/`-skelet, GitHub Actions-workflow, DNS-script,
      frontend-Dockerfile
- [ ] Flyt ADR-drafts til `~/projects/adrs/` når de er accepteret

## M1 — Backend: Loft-kerne

- [x] Migration: `lofts`-tabel (id, navn, created_at, last_active)
- [x] `POST /v1/lofts` — opret link-rum, returnér `{ id, url }`
- [x] `GET /v1/lofts/:id` — findes rummet? (til link-åbning)
- [x] WS `/v1/ws/:loft_id` — gæste-join med navn; `join`/`leave`/`roster`/
      `presence`/`signal`; serveren sætter `from` på signaler
- [x] `/healthz`-endpoint + k8s-probes
- [x] TTL-oprydning af inaktive lofts (baggrundsjob, LOFT_TTL_HOURS)
- [x] Rate limiting på loft-oprettelse (tower_governor, XFF-baseret)
- [x] Fjern auth-, room-, DM-, file- og uploads-stier fra routeren i takt med
      at WS'eren er omskrevet
- [x] Integrationstests kørt mod lokal PostgreSQL (13/13 grønne)
- [x] Dockerfile: non-root-bruger (uid 10001); read-only rootfs i k8s-manifest

## M2 — Frontend: Loft-UI

- [ ] `loft.html` — opret/deltag via link, vælg navn, mic/cam-toggle,
      skærmdeling, forlad rum
- [ ] `rtc.js` — mesh-modul med ét `RTCPeerConnection` pr. deltager og
      perfect negotiation (refaktor af chat.html's 1:1-logik)
- [ ] Genbrug ICE/TURN-logik (`addTurnServer`) og auto-reconnect med backoff
- [x] Invite: copy-link + `navigator.share` (QR udskudt — kræver ekstern lib)
- [x] Luk loft (ejer-nøgle via `DELETE /v1/lofts/:id` + `closed`-broadcast)
- [ ] Link-preview: beslut backend-rendret `/h/:id` vs. generiske og-tags
- [ ] Deltager-UI opdateres i realtid via roster/presence-beskeder
- [ ] Nye sider afløser index/rooms/chat.html (slet når e2e er grøn)

## M3 — Tests + CI

- [x] Playwright: to kontekster pr. test — connected + lyd, skærmdeling
      (fake stream), leave/rejoin og link fra frisk kontekst (4/4 grønne)
- [x] Cargo-tests: loft-registry og WS-signalering (14/14 grønne)
- [x] `.github/workflows/build.yml` bygger `loft` + `loft-web` (SHA-tags)
- [x] E2e-jobb i GitHub Actions (postgres + backend + Chromium)
- [ ] GHCR-pakker gøres public
- [ ] E2e kører mod `loft.test.gihc.online` før prod-promote

## M4 — k3s deploy (test-miljø)

- [ ] A-record for `loft.test.gihc.online` via `scripts/create-dns-record.sh`
- [ ] Secret `loft-secrets` i `loft-test` (postgres-password, database-url)
- [ ] Apply `k8s/test/` (postgres, api, web, ingress, coturn)
- [ ] Firewall i `infra/tofu/main.tf`: TCP+UDP 3478, UDP 49152–49200
- [ ] `letsencrypt-staging` → verificér cert → `letsencrypt-prod`
- [ ] Smoke-test omskrevet til `kubectl exec` + gæste-loft-flow
- [ ] Manifester for beta/prod i `k8s/prod/`

## M5 — Oprydning

- [ ] Slet `docker-compose*.yml`, `ansible/`, `caddy/`, `.woodpecker.yaml`
      og IPFS/DNSLink-rester
- [ ] Slet døde DNS-records (`api.gihc.online`, `chat.apps.gihc.online`,
      `beta.*`, `test.*`)
- [ ] Opdater `runbooks/`, `AGENTS.md` og ADR-index
- [ ] Omdøb `chat/` til `loft/` og genovervej repoets navn
- [ ] Referat i `referater/`

## Sikkerhed (videreført fra chat)

- [ ] Rate limiting på loft-oprettelse og WS-håndtryk
- [ ] Capability-link er tilfældig UUID; verificér at ingress-logs ikke
      logger query/fragment med hvis tokens indføres
- [ ] Link-expiry/revocation (TTL på lofts)
- [ ] CSP: `default-src 'self'`, kamera/mikrofon via `Permissions-Policy`,
      ingen tredjeparts-scripts (Turnstile udgår)
- [ ] Containerhærdning: non-root, read-only fs, `no-new-privileges`
- [ ] `trivy image` mod byggede images + `cargo audit` i CI
- [ ] OWASP ZAP passiv baseline mod `loft.test.gihc.online`

## GDPR (forenklet)

- [x] Ingen konti, emails eller beskedhistorik i MVP — minimal persondata
- [ ] Dokumentér opbevaring: lofts-tabel (navn + tidsstempler) med TTL;
      deltagernavne findes kun i hukommelsen under sessionen
