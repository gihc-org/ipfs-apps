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

> Fortsæt arbejdet på **Loft**. Branchen `feat/loft-k3s-refocus` er merget til
> `trunk` (M0–M4 færdige): testmiljøet kører på `loft.test.gihc.online` med
> hærdet coturn, smoke-test og e2e (inkl. TURN-relay) grønt.
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
> - coturn kører hærdet (non-root, read-only rootfs, no_new_privs, kun
>   `NET_BIND_SERVICE` i bounding-settet) og med `--denied-peer-ip` + kvoter.
> - **coturn er på vej ud af dette repo**: den flyttes til platform-laget i
>   `~/projects/infra` som én delt instans på `turn.gihc.online` (se afsnittet
>   "Efter M4 — delt TURN flyttes til platformen" og infra-repoets TODO med
>   start-prompt). Indtil flytningen er gennemført, er coturn i `k8s/test`
>   den eneste instans, og prod-manifesterne må **ikke** deployes ved siden af.
>
> Næste opgaver, i denne rækkefølge:
> 1. Manuel accepttest af testmiljøet med rigtige enheder (headset; gerne én
>    enhed på mobildata så relay-stien bruges).
> 2. Når infra-sessionen har flyttet coturn: peg `config.js` i `k8s/test` og
>    `k8s/prod` på `turn:turn.gihc.online:3478?transport=udp`, fjern
>    coturn-manifesterne fra appen, og kør `e2e/tests/turn.spec.ts` igen.
> 3. Prod-deploy af `loft.gihc.online` efter `runbooks/loft-deploy.md` (DNS
>    via `scripts/create-dns-record.sh loft`, `pass insert
>    loft/prod-postgres-password`, cert staging → prod). TURN-portene står
>    allerede åbne i `platform-firewall`.
> Adgang til k3s/pass/infra-repoet kræver brugerens godkendelse — spørg før
> trin uden for repoet.
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

- [x] `loft.html` — opret/deltag via link, vælg navn, mic/cam-toggle,
      skærmdeling, forlad rum
- [x] `rtc.js` — mesh-modul med ét `RTCPeerConnection` pr. deltager og
      perfect negotiation (refaktor af chat.html's 1:1-logik)
- [x] Genbrug ICE/TURN-logik (`addTurnServer`) og auto-reconnect med backoff
- [x] Invite: copy-link + `navigator.share` (QR udskudt — kræver ekstern lib)
- [x] Luk loft (ejer-nøgle via `DELETE /v1/lofts/:id` + `closed`-broadcast)
- [ ] Link-preview: beslut backend-rendret `/h/:id` vs. generiske og-tags
- [x] Deltager-UI opdateres i realtid via roster/presence-beskeder
- [ ] Nye sider afløser index/rooms/chat.html (slet når e2e er grøn)

## M3 — Tests + CI

- [x] Playwright: to kontekster pr. test — connected + lyd, skærmdeling
      (fake stream), leave/rejoin og link fra frisk kontekst (4/4 grønne)
- [x] Cargo-tests: loft-registry og WS-signalering (14/14 grønne)
- [x] `.github/workflows/build.yml` bygger `loft` + `loft-web` (SHA-tags)
- [x] E2e-jobb i GitHub Actions (postgres + backend + Chromium)
- [x] GHCR-pakker public — pakker pushet af Actions med `GITHUB_TOKEN` arver
      repoets synlighed, så `loft`/`loft-web` kan hentes anonymt
- [x] E2e kører mod `loft.test.gihc.online` før prod-promote (5/5: fire
      loft-tests + TURN-relay, 2026-09-11)

## M4 — k3s deploy (test-miljø)

- [x] A-record for `loft.test.gihc.online` via `scripts/create-dns-record.sh`
- [x] Secret `loft-secrets` i `loft-test` (postgres-password, database-url) —
      password genereret og gemt i `pass` som `loft/postgres-password`
- [x] Apply `k8s/test/` (postgres, api, web, ingress, coturn) — kørende
- [x] Firewall i `infra/tofu/main.tf`: TCP+UDP 3478, UDP 49152–49200 — lagt
      ind 2026-08-01; verificér med `tofu plan` i `~/projects/infra/tofu`
- [x] `letsencrypt-staging` → verificér cert → `letsencrypt-prod` (betroet
      cert, `curl` uden `-k` giver 200)
- [x] Smoke-test omskrevet til `kubectl exec` + gæste-loft-flow
      (`scripts/smoke-test.sh` + `scripts/smoke-ws.py`, 18 checks)
- [x] Manifester for beta/prod i `k8s/prod/` (forberedt, ikke deployet)
- [x] Ingress-rute for `/healthz` (lå ellers hos web-frontenden → 404)
- [x] Runbook: `runbooks/loft-deploy.md` (DNS → secret → apply → cert → test)
- [x] coturn hærdet: non-root, read-only rootfs, no_new_privs, minimal
      capability-bounding-set (`add: ["NET_BIND_SERVICE"]`)
- [x] TURN-misbrugsbeskyttelse: `--denied-peer-ip` for private/loopback/
      link-local/CGNAT + kvoter (`max-bps`, `bps-capacity`, `user-quota`,
      `total-quota`, `stale-nonce`)
- [x] TURN-relay-test (`e2e/tests/turn.spec.ts`, relay-only ICE, kørt grønt mod
      loft.test.gihc.online)

## Efter M4 — delt TURN flyttes til platformen

Beslutning 2026-09-11: coturn hører ikke hjemme i app-repoet. Der kan kun køre
én coturn på noden (to instanser binder begge 3478 via `SO_REUSEPORT` og deler
trafikken tilfældigt), og flere WebRTC-apps skal kunne dele reliancen. Opgaven
ligger nu i `~/projects/infra/TODO.md` under "Platform — delt TURN (coturn)",
med start-prompt. Her i repoet er der kun tilbage at følge op:

- [ ] Peg `config.js` på `turn:turn.gihc.online:3478?transport=udp` i både
      `k8s/test/configmap.yaml` og `k8s/prod/configmap.yaml`, og sæt
      `TURN_SECRET` til det fælles secret fra `pass`
- [ ] Fjern `k8s/test/deployment-coturn.yaml` og
      `k8s/prod/deployment-coturn.yaml` samt `realm`/`external-ip`/`turn-secret`
      fra configmaps (de hører nu til platformen)
- [ ] `kubectl delete deploy/loft-coturn -n loft-test` (og tilsvarende i
      `loft-prod`, hvis det er deployet) så porten frigives til den delte instans
- [ ] Opdater README (arkitekturdiagrammet viser coturn under appen),
      MIGRATION og `runbooks/loft-deploy.md`, så de peger på den fælles
      instans og `scripts/check-turn.sh` i infra
- [ ] Kør `e2e/tests/turn.spec.ts` mod testmiljøet igen — den er den eneste
      test der fanger en forkert `TURN_URL` efter flytningen

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
- [ ] Udsted TURN-credentials i API'et (fx `GET /v1/ice`) med kort TTL, så
      `TURN_SECRET` ikke længere ligger i frontendens `config.js` — HMAC'en
      hører hjemme server-side
- [ ] Overvåg coturns allocations/båndbredde — flyttet til
      `~/projects/infra/TODO.md`, da instansen bliver delt
- [ ] CSP: `default-src 'self'`, kamera/mikrofon via `Permissions-Policy`,
      ingen tredjeparts-scripts (Turnstile udgår)
- [ ] Containerhærdning: non-root, read-only fs, `no-new-privileges`
- [ ] `trivy image` mod byggede images + `cargo audit` i CI
- [ ] OWASP ZAP passiv baseline mod `loft.test.gihc.online`

## GDPR (forenklet)

- [x] Ingen konti, emails eller beskedhistorik i MVP — minimal persondata
- [ ] Dokumentér opbevaring: lofts-tabel (navn + tidsstempler) med TTL;
      deltagernavne findes kun i hukommelsen under sessionen
