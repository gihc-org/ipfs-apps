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

> Fortsæt arbejdet på **Loft** (M0–M4 færdige). Testmiljøet kører på
> `loft.test.gihc.online`. Arbejdet ligger i commits på
> `feat/loft-k3s-refocus`, som **ikke** er merget til `trunk` eller pushet
> endnu — se afsnittet "Branche og CI" nedenfor.
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
> - k8s: `k8s/test/` (namespace `loft-test`) — postgres + PVC, api, web og
>   ingress. Prod-manifesterne i `k8s/prod/` er forberedt (pinnet SHA), ikke
>   deployet.
> - TURN er flyttet til platformen: `turn.gihc.online` (namespace `coturn` i
>   `~/projects/infra`, ADR 0003), hærdet og med `--denied-peer-ip` + kvoter.
>   Appen har ingen egen coturn længere — `config.js` peger på den fælles
>   instans, og `TURN_SECRET` rendres fra `pass turn/static-auth-secret`.
> - e2e kører mod det deployede miljø i **både Chromium og Firefox**
>   (`npm run test:test` / `npm run test:test:firefox`, 6/6 hver 2026-09-12).
>
> Næste opgaver, i denne rækkefølge:
> 1. Åbent fund: lyd-routing på telefoner — WebRTC-lyden kom ud af telefonens
>    højttaler i stedet for Bluetooth-earpluggene (podcast går fint i
>    earpluggene). Start med at køre Google Meet i Firefox på samme telefon:
>    se afsnittet "Lyd-routing på telefoner".
> 2. Tag stilling til grenen: push `feat/loft-k3s-refocus` og merge til `trunk`
>    (CI bygger kun på push til `trunk`, og Actions læser workflows fra default
>    branch), eller fortsæt på grenen.
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
> - E2e mod deployet miljø: `npm run test:test` (Chromium) og
>   `npm run test:test:firefox` (kræver `npx playwright install firefox` én gang)
>
> Gotchas: `TEST_API_URL` uden `/v1`; `frontend/config.js` overskrives i k8s af
> ConfigMap; e2e bruger sessionStorage-nøglen `loft.noAutoMic`; headless
> Playwright spiller stadig testlyd — begge browsere er dæmpet i
> `playwright.config.ts`.

## Branche og CI (status 2026-09-12)

- Alt arbejdet ligger i commits på `feat/loft-k3s-refocus`, flere commits foran
  `origin/trunk` (der stadig står på `f83722c`). Intet er pushet.
- `.github/workflows/build.yml` bygger kun ved push til `trunk`, og GitHub
  Actions læser workflows fra default branch. Før grenen er pushet/merget,
  bygges der altså ingen nye images — testmiljøet kører på `:latest` fra det
  sidste build, mens `k8s/prod/` er pinnet til `f83722c`.
- Derfor: beslut om grenen (punkt 2 i start-prompten) før et prod-deploy.

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
- [x] E2e kører mod `loft.test.gihc.online` før prod-promote — 6/6 i både
      Chromium og Firefox 2026-09-12 (fire loft-tests, TURN-relay mellem to
      klienter og et tjek af at den deployede sides egen `config.js` giver en
      relay-kandidat)

## M4 — k3s deploy (test-miljø)

- [x] A-record for `loft.test.gihc.online` via `scripts/create-dns-record.sh`
- [x] Secret `loft-secrets` i `loft-test` (postgres-password, database-url) —
      password genereret og gemt i `pass` som `loft/postgres-password`
- [x] Apply `k8s/test/` (postgres, api, web, ingress) — kørende. coturn blev
      først deployet her og er siden flyttet til platformen (se nedenfor)
- [x] Firewall i `infra/tofu/main.tf`: TCP+UDP 3478, UDP 49152–49200 — lagt
      ind 2026-08-01 (ejes nu af platformen sammen med coturn; TURN-portene er
      ikke app-specifikke længere)
- [x] `letsencrypt-staging` → verificér cert → `letsencrypt-prod` (betroet
      cert, `curl` uden `-k` giver 200)
- [x] Smoke-test omskrevet til `kubectl exec` + gæste-loft-flow
      (`scripts/smoke-test.sh` + `scripts/smoke-ws.py`, 18 checks)
- [x] Manifester for beta/prod i `k8s/prod/` (forberedt, ikke deployet)
- [x] Ingress-rute for `/healthz` (lå ellers hos web-frontenden → 404)
- [x] Runbook: `runbooks/loft-deploy.md` (DNS → secret → apply → cert → test)
- [x] coturn hærdet: non-root, read-only rootfs, no_new_privs, minimal
      capability-bounding-set (`add: ["NET_BIND_SERVICE"]`) — hærdningen fulgte
      med til platformen 2026-09-12
- [x] TURN-misbrugsbeskyttelse: `--denied-peer-ip` for private/loopback/
      link-local/CGNAT + kvoter (`max-bps`, `bps-capacity`, `user-quota`,
      `total-quota`, `stale-nonce`) — ligeledes flyttet til platformen
- [x] TURN-relay-test (`e2e/tests/turn.spec.ts`, relay-only ICE, kørt grønt mod
      loft.test.gihc.online)

## Efter M4 — delt TURN flyttet til platformen (afsluttet 2026-09-12)

Beslutning 2026-09-11: coturn hører ikke hjemme i app-repoet. Der kan kun køre
én coturn på noden (to instanser binder begge 3478 via `SO_REUSEPORT` og deler
trafikken tilfældigt), og flere WebRTC-apps skal kunne dele reliancen. Flytningen
er gennemført — platform-siden står i `~/projects/infra`
(`docs/adr/0003-delt-turn-platform.md`, `k8s/coturn/`, `scripts/check-turn.sh`).

- [x] Peg `config.js` på `turn:turn.gihc.online:3478?transport=udp` i både
      `k8s/test/configmap.yaml` og `k8s/prod/configmap.yaml`, med `TURN_SECRET`
      fra `pass turn/static-auth-secret`
- [x] Fjern `k8s/test/deployment-coturn.yaml` og
      `k8s/prod/deployment-coturn.yaml` samt `realm`/`external-ip`/`turn-secret`
      fra configmaps (de hører nu til platformen)
- [x] `kubectl delete deploy/loft-coturn -n loft-test` — porten er frigivet til
      den delte instans (`loft-prod` er ikke deployet)
- [x] Opdater README, MIGRATION og `runbooks/loft-deploy.md`, så de peger på den
      fælles instans og `scripts/check-turn.sh` i infra
- [x] Kør `e2e/tests/turn.spec.ts` mod testmiljøet igen — kørt grøn mod
      `turn.gihc.online` 2026-09-12 (`valgt pair: relay/relay` i både Chromium
      og Firefox) sammen med resten af suiten (6/6 i hver browser) og
      smoke-testen (18/18). Platformens egen `check-turn.sh` er også grøn.
- [x] Relay-stien bekræftet manuelt af brugeren (telefon på mobildata). Det gav
      fundet om lyd-routing — se afsnittet nedenfor.

## M5 — Oprydning

- [ ] Slet `docker-compose*.yml`, `ansible/`, `caddy/`, `.woodpecker.yaml`
      og IPFS/DNSLink-rester
- [x] Slet døde DNS-records — 24 records slettet 2026-09-12 (18 A + 6
      `_dnslink`-TXT). Zonen har nu 12 records: kun de fem levende hosts
      (`loft.test`, `hyfer.test`, `capture.test`, `test`, `higgs`) plus
      MX/TXT/NS til mail. Liste med værdier (og CID'er) står i
      `referater/2026-09-12-00-15.md`, hvis en record skal genskabes.
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
- [x] Overvåg coturns allocations/båndbredde — flyttet til platformen, som nu
      eksponerer Prometheus-metrics (`turn_total_allocations`,
      `turn_unauthenticated_401_requests`, `turn_auth_credential_failures` …)
      på `coturn-metrics.coturn.svc.cluster.local:9641`. Der mangler kun en
      scraper i clusteret (infra-opgave)
- [ ] CSP: `default-src 'self'`, kamera/mikrofon via `Permissions-Policy`,
      ingen tredjeparts-scripts (Turnstile udgår)
- [ ] Containerhærdning: non-root, read-only fs, `no-new-privileges`
- [ ] `trivy image` mod byggede images + `cargo audit` i CI
- [ ] OWASP ZAP passiv baseline mod `loft.test.gihc.online`

## Lyd-routing på telefoner (fundet 2026-09-12)

Manuel accepttest på Android + Bluetooth-earplugs: podcast og anden medieafspilning
kører i earpluggene (A2DP), men Lofts WebRTC-lyd kom ud af telefonens højttaler.
Headsettet har både "telefonopkald" (HFP) og "medielyd" (A2DP) slået til, så det
er ikke et A2DP-only headset — det er browserens/Android's valg af
kommunikationsenhed for WebRTC.

- [ ] Afklar om det er Firefox-specifikt: kør Google Meet i **Firefox** på samme
      telefon med samme earplugs (Meet er ren WebRTC). Landet lyden i earpluggene
      der, er der noget i vores frontend; gør den ikke, er det Firefox/Android.
- [ ] Tjek om det forsvinder uden kamera/skærmdeling (Chrome/Firefox bruger
      speakerphone-tilstand i video-sessioner) og om earpluggene skifter til
      SCO/opkaldsprofil, når mikrofonen tændes.
- [ ] Afprøv om `HTMLMediaElement.setSinkId()` kan tvinge fjern-lyden over på
      earpluggene. Desktop-Firefox 148 har API'et (verificeret 2026-09-12),
      `AudioContext.setSinkId` findes kun i Chromium — og på Firefox til Android
      er det uafklaret, så det skal testes på telefonen. Kræver at
      `enumerateDevices()` overhovedet viser `audiooutput`-enheder på Android.
- [ ] Hvis `setSinkId` ikke er en vej: undersøg om Firefox skifter til SCO når
      mikrofonen er aktiv (pauser earpluggene deres musik, når man joiner?).

## GDPR (forenklet)

- [x] Ingen konti, emails eller beskedhistorik i MVP — minimal persondata
- [ ] Dokumentér opbevaring: lofts-tabel (navn + tidsstempler) med TTL;
      deltagernavne findes kun i hukommelsen under sessionen
