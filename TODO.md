# TODO — Loft (tidligere ipfs-apps/chat)

Projektet fokuseres til **Loft**: WebRTC link-rum (lyd, video, skærmdeling)
med gæsteadgang, delt via link over Matrix/XMPP/mail. Samtidig migreres fra
Caddy + Docker Compose til k3s. Se [MIGRATION.md](MIGRATION.md) og ADR'erne i
`~/projects/adrs/` (0027 og 0028).

Afsluttet før refokuseringen (chat-rum, DM, filoverførsel, 1:1-skærmdeling og
lydopkald) ligger i git-historikken; Playwright- og WebRTC-mønstre derfra
genbruges.

## Start-prompt til ny session

Kopér blokken herunder som første besked til agenten:

> Fortsæt arbejdet på **Loft** (M0–M4 færdige, prod deployet). Alt er merget
> til `trunk` og pushet; CI bygger `ghcr.io/gihc-org/loft{,-web}` ved push.
> Start en **frisk session** frem for at genoptage en gammel: de tidligere
> sessioner er meget tunge (den forrige sendte ~286.000 tokens pr. request og
> ramte "idle timeout waiting for SSE"), og alt nødvendigt står i filerne
> herunder.
>
> Læs først `README.md`, `MIGRATION.md`, `TODO.md` og ADR 0027
> (`~/projects/adrs/0027-loft-link-rooms.md`) + 0028
> (`~/projects/adrs/0028-loft-group-mesh.md`).
>
> Tilstand:
> - Backend (`loft/`, omdøbt fra `chat/` i M5): Rust/Axum, krate/binær `loft`.
>   `POST /v1/lofts`, `GET /v1/lofts/:id`, `DELETE /v1/lofts/:id` med
>   `X-Owner-Token`, WS `/v1/ws/:loft_id`
>   (join/roster/signal/media-state/closed), `/healthz`, TTL-cleanup.
>   Migrationer kører automatisk ved start.
> - Frontend: `frontend/loft.html` + `frontend/rtc.js` (mesh, perfect
>   negotiation). `index.html` redirecter til loft.html.
> - Testmiljø: `k8s/test/` (namespace `loft-test`) — postgres + PVC, api, web
>   og ingress — kører på `loft.test.gihc.online` med CI-image `2816181…` og er
>   accepteret i hånden (smoke-test 19/19, e2e 9 passed / 2 skipped i både
>   Chromium og Firefox).
> - Prod: `loft.gihc.online` blev **deployet 2026-09-16** — namespace
>   `loft-prod`, pinnet CI-SHA `d082905…`, betroet `letsencrypt-prod`-cert,
>   smoke-test 19/19 og e2e grøn i Chromium og Firefox (inkl. TURN-relay).
> - TURN er flyttet til platformen: `turn.gihc.online` (namespace `coturn` i
>   `~/projects/infra`, ADR 0003), hærdet og med `--denied-peer-ip` + kvoter.
>   Appen har ingen egen coturn længere — `config.js` peger på den fælles
>   instans, og `TURN_SECRET` rendres fra `pass turn/static-auth-secret`
>   (verificeret i sync med prod-configmap'en 2026-09-16).
> - e2e (`npm run test:test` / `npm run test:test:firefox`) kører mod det
>   deployede miljø i **både Chromium og Firefox**; de to TURN-tests kræver
>   `TURN_URL` og er derfor de eneste der springes over som standard.
>
> M5-oprydningen er gennemført i repoet 2026-09-16 (`chat/` → `loft/`,
> compose-/ansible-/caddy-/IPFS-rester slettet, `AGENTS.md` og runbooks
> opdateret).
>
> ADR-drafts er accepteret og flyttet til `~/projects/adrs/` (0027 og 0028,
> opdateret så coturn ligger i platformen og `presence`-beskederne er væk).
>
> Næste opgave:
> 1. Talende brugere på Android: modpartens lyd går i telefonens højttaler
>    (lyttere er dækket af "join muted" og capture-frigivelse). Afbøderinger
>    står i afsnittet "Kendt begrænsning: talende brugere på Android".
> Adgang til k3s/pass/infra-repoet kræver brugerens godkendelse — spørg før
> trin uden for repoet. Til k3s-arbejde skal SSH-tunnelen være åben:
> `ssh -o ServerAliveInterval=20 -L 6443:localhost:6443 -N -f hetzner-k3s`.
>
> Kommandoer:
> - Unit: `cd loft && cargo test --lib`
> - Integration: `cd loft && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo test`
> - E2e (backend kørende): `cd e2e && TEST_API_URL=http://localhost:8081 npx playwright test`
> - E2e mod deployet miljø: `npm run test:test` (Chromium) og
>   `npm run test:test:firefox` (kræver `npx playwright install firefox` én gang)
> - TURN-relay mod et miljø: `cd e2e && TURN_URL="$(~/projects/infra/scripts/turn-config.sh --url)" TURN_SECRET="$(pass turn/static-auth-secret)" BASE_URL=https://loft.test.gihc.online npx playwright test tests/turn.spec.ts`
>
> Gotchas: `TEST_API_URL` uden `/v1`; `frontend/config.js` overskrives i k8s af
> ConfigMap; mikrofonen starter **ikke** ved join (se "Lyd-routing på
> telefoner"), så e2e tænder den eksplicit og en peer-forbindelse etableres
> først når nogen deler medier; headless Playwright spiller stadig testlyd —
> begge browsere er dæmpet i `playwright.config.ts`; `.guidelines` er et
> soft-link til `~/projects/guidelines`, og `scripts/check.sh` springer reviewet
> over ("fail open"), hvis `claude`-kaldet ikke kan komme på nettet.

## Branche og CI (afsluttet 2026-09-13)

- Grenen er merget til `trunk` og pushet; alle refs stod på `2816181`
  (`trunk`, `feat/loft-k3s-refocus`, `origin/trunk`), og efter
  dokumentationspushet 2026-09-16 står de på `d082905`. CI har bygget begge
  images: `ghcr.io/gihc-org/loft{,-web}` med tag
  `28161813015adf4f27fdcb57c774e3fedc67f550` (verificeret anonymt mod GHCR's
  tags-API, da `gh` ikke er installeret i agentmiljøet). `loft-web` er rullet i
  `loft-test` på det byggede image. `k8s/prod/` blev først pinnet til samme SHA
  og derefter (2026-09-16) rullet videre til `d0829054baf36b…`, som også ligger
  på `trunk`. Dokumentations-commits efter den rykker `trunk` videre uden at
  ændre koden, så pinnen skal først bumpes igen ved en egentlig kodeændring.
- Workflowet (`.github/workflows/build.yml`) udløses af push til `trunk` og af
  `workflow_dispatch`. Det har tre jobs: `api`, `web` og `e2e` — e2e-jobbet
  kører Playwright på Chromium mod en lokal postgres + backend, og
  **cargo-testene køres ikke i CI** (se det åbne punkt under M3).
- 2026-09-16: `98bef4a` (prod-pin), `2008e82` (URI-sikker postgres-adgangskode i
  runbooken) og `d082905` (statusopdatering af dokumentationen) er pushet til
  `trunk`.
- 2026-09-16 (senere): prod deployet og verificeret mod `d082905…`; pin bumpet
  fra `2816181…` til `d082905…` i `k8s/prod/deployment-{api,web}.yaml` og rullet
  med `kubectl apply` (Recreate, ~30 s afbrydelse).
- Historik (2026-09-12): indtil merge lå arbejdet på `feat/loft-k3s-refocus`
  foran `origin/trunk` (der stod på `f83722c`), og der blev ikke bygget images,
  fordi workflowet kun udløses af push til `trunk`.

## M0 — Fundament (dokumentation)

- [x] Beslutning: Loft link-rum (Plan B), gæsteadgang, statisk frontend
- [x] TODO.md og MIGRATION.md omskrevet til Loft + k3s
- [x] ADR-drafts 0027 (link-rum + k3s) og 0028 (mesh-topologi) — accepteret og
      flyttet til `~/projects/adrs/` 2026-09-16
- [x] `k8s/test/`-skelet, GitHub Actions-workflow, DNS-script,
      frontend-Dockerfile
- [x] Flyt ADR-drafts til `~/projects/adrs/` når de er accepteret — gjort
      2026-09-16

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
- [x] E2e kører mod `loft.test.gihc.online` før prod-promote — senest 9 passed
      / 2 skipped i både Chromium og Firefox 2026-09-13 (de to TURN-tests
      springes over uden `TURN_URL`). Suiten er `loft.spec.ts` (6),
      `audio-debug.spec.ts` (3) og `turn.spec.ts` (2) og dækker loft-flowet,
      skærmdeling, leave/rejoin, diagnostiksiden, TURN-relay mellem to klienter
      og et tjek af at den deployede sides egen `config.js` giver en
      relay-kandidat
- [ ] Kør `cargo test` (unit + integration) i CI — i dag har workflowet kun
      `api`-, `web`- og `e2e`-jobs, så cargo-testene er manuelle

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
- [x] Manifester for beta/prod i `k8s/prod/` — pinnet til CI-SHA; selve
      deployet følges op i afsnittet "Prod-deploy" nedenfor
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
      den delte instans (prod-deployet af `loft-prod` er i gang, se nedenfor)
- [x] Opdater README, MIGRATION og `runbooks/loft-deploy.md`, så de peger på den
      fælles instans og `scripts/check-turn.sh` i infra
- [x] Kør `e2e/tests/turn.spec.ts` mod testmiljøet igen — kørt grøn mod
      `turn.gihc.online` 2026-09-12 (`valgt pair: relay/relay` i både Chromium
      og Firefox) sammen med resten af suiten (6/6 i hver browser) og
      smoke-testen (18/18). Platformens egen `check-turn.sh` er også grøn.
- [x] Relay-stien bekræftet manuelt af brugeren (telefon på mobildata). Det gav
      fundet om lyd-routing — se afsnittet nedenfor.

## Prod-deploy — `loft.gihc.online` (i gang 2026-09-16)

Følger [runbooks/loft-deploy.md](runbooks/loft-deploy.md) under "Prod-deploy".
`k8s/prod/` er pinnet til `2816181…` og deler platformens TURN.

- [x] DNS: `bash scripts/create-dns-record.sh loft` → `A loft.gihc.online` →
      65.109.233.92 (verificeret udefra; ingress svarer 404 indtil appen er
      apply'et)
- [x] Secret: `pass insert loft/prod-postgres-password` (48 tegn, rent
      alfanumerisk → URI-sikker) og `kubectl -n loft-prod create secret
      loft-secrets` med `postgres-password` + `database-url` (verificeret ved at
      parse URL'en: bruger `loft`, host `postgres`, port 5432, database
      `loftdb`), sammen med namespace `loft-prod`
- [x] TURN-secret verificeret i sync med platformen (`pass
      turn/static-auth-secret` = `k8s/prod/configmap.yaml`, 32 tegn)
- [x] Apply `k8s/prod/configmap.yaml` + `k8s/prod/` og følg rollouts
      (`loft-postgres`, `loft-api`, `loft-web`) — alle tre "successfully rolled
      out" 2026-09-16
- [x] Cert: `letsencrypt-staging` → verificér → `letsencrypt-prod` — byttet via
      ingress-annotation; cert `loft-gihc-online-tls` er nu `letsencrypt-prod`,
      gyldigt til 2026-12-15, og `ssl_verify_result` er 0
- [x] Smoke-test (`scripts/smoke-test.sh https://loft.gihc.online
      --namespace loft-prod`) og e2e (`npm run test:beta`) — 19/19 checks;
      Chromium 9 passed / 2 skipped, Firefox 11/11 og TURN-relay 2/2
- [x] Push af de tre lokale commits (`98bef4a`, `2008e82`, `d082905`) — gjort
      2026-09-16; udløser et nyt CI-build, men deployet kan bruge de
      eksisterende images med `2816181…`, da koden er uændret
- [x] Pin `k8s/prod/` til det nyeste CI-SHA efter dokumentationspushet — rullet
      til `d082905…` 2026-09-16 (samme kildekode som `2816181…`)

## M5 — Oprydning (gennemført 2026-09-16 i repoet)

- [x] Slet `docker-compose*.yml`, `ansible/`, `caddy/`, `.woodpecker.yaml`
      og IPFS/DNSLink-rester — også `runbooks/deploy.md` og
      `runbooks/ipfs-dns.md` samt de sidste chat-sider i `frontend/`
      (`chat.html`, `rooms.html`, `forgot.html`, `reset.html`, `privacy.html`,
      `version-check.js`). `.env.example` og `OWASP-IMPROVEMENTS.md` blev
      beholdt (førstnævnte bruges til lokal `cargo run`, sidstnævnte er nu
      markeret historisk)
- [x] Slet døde DNS-records — 24 records slettet 2026-09-12 (18 A + 6
      `_dnslink`-TXT). Zonen har nu 12 records: kun de fem levende hosts
      (`loft.test`, `hyfer.test`, `capture.test`, `test`, `higgs`) plus
      MX/TXT/NS til mail. Liste med værdier (og CID'er) står i
      `referater/2026-09-12-00-15.md`, hvis en record skal genskabes.
- [x] Opdater `runbooks/`, `AGENTS.md` og ADR-index — `AGENTS.md` er skrevet om
      til Loft-tilstanden (chat-sektionerne væk), runbooks peger på k3s-flowet,
      og ADR-tabellen markerer hvilke ADR'er 0027/0028 afløser
- [x] Omdøb `chat/` til `loft/` — også kraten og binæren (`Cargo.toml`,
      `Dockerfile`, CI-workflow, `.gitignore`). Repo-navnet `ipfs-apps` står
      stadig tilbage (kræver ændring uden for repoet)
- [x] Flyt ADR-drafts 0027/0028 til `~/projects/adrs/` — gjort 2026-09-16:
      begge er markeret Accepted, coturn-afsnittet peger på platformen, og
      `presence`-beskederne er erstattet af roster/`join`/`leave` +
      `media-state`
- [x] Referat i `referater/` — `2026-09-16-17-37.md` (prod-deploy) og
      `2026-09-16-18-05.md` (M5-oprydning)

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

### Verificeret i desktop-browsere (2026-09-13, headless via Playwright)

| Browser | `HTMLMediaElement.setSinkId` | `AudioContext.setSinkId` | `selectAudioOutput` | `audiooutput` i `enumerateDevices()` |
|---------|------------------------------|--------------------------|---------------------|--------------------------------------|
| Chromium 147 | findes | findes | findes ikke | ja, uden tilladelse (fake-enheder) |
| Firefox 148 | findes | findes ikke | findes | kun **efter** mikrofon-tilladelse (`getUserMedia`) |

`audio.setSinkId(<deviceId>)` blev anvendt på en ægte remote WebRTC-lydtrack i
begge browsere (e2e-testen `lydudgang kan vælges … (setSinkId)`), så på desktop
virker sink-valg hele vejen fra `RTCPeerConnection` til udgangen. På Android er
det stadig uafklaret, og svaret afhænger af om `enumerateDevices()` overhovedet
viser `audiooutput`-enheder der.

### Android-afklaring (2026-09-13)

Slået op i MDN browser-compat-data og Bugzilla, efter telefonen meldte at
`HTMLMediaElement.setSinkId` ikke findes:

| API | Firefox desktop | Firefox for Android | Chrome for Android |
|-----|-----------------|---------------------|--------------------|
| `HTMLMediaElement.setSinkId` | 116+ | **nej** ([bug 1473346](https://bugzil.la/1473346)) | **nej** ([crbug 41276355](https://crbug.com/41276355)) |
| `mediaDevices.selectAudioOutput` | 116+ | **nej** | nej ([crbug 372214870](https://crbug.com/372214870)) |

Der findes altså **ingen** output-device-API på Android, hverken i Firefox eller
Chrome. `setSinkId`-vejen er dermed lukket på telefonen, og "Lyd ud"-vælgeren i
`loft.html` kan pr. konstruktion aldrig dukke op der (den er feature-detekteret).
Tilbage er adfærds-vejene: mikrofonen/capture (`MODE_IN_COMMUNICATION`) og
video-/speakerphone-tilstand — derfor er tone-testen i `audio-debug.html`
afgørende: den kører fjernlyd gennem WebRTC **uden** mikrofon og uden tilladelser.

Bekræftet i Firefox-kilden (gecko-dev, tip ≈ 156) 2026-09-13 — API'et er både
SecureContext- og pref-gated, og preffen er slået fra på Android:

```
- name: media.setsinkid.enabled
  type: bool
#if defined(MOZ_WIDGET_ANDROID)
  value: false # bug 1473346
#else
  value: true
#endif
```

(`dom/webidl/HTMLMediaElement.webidl`: `[SecureContext, Pref="media.setsinkid.enabled"]
readonly attribute DOMString sinkId;` + `setSinkId`.) Telefonens Firefox 155 er
altså omfattet — versionsnummeret ændrer ikke noget. `bug 1473346`
("Investigate supporting audio device enumeration in cubeb on android") står
stadig som NEW, så selv med preffen slået til manuelt ville der ikke være
`audiooutput`-enheder at vælge imellem.

Bemærk også: `[SecureContext]` betyder at hele API'et er skjult over http — den
første telefon-rapport ("setSinkId findes IKKE") kan derfor ikke bruges som
bevis i sig selv; kildekoden ovenfor er beviset.

### Telefon-måling 2026-09-13 (Android 15, Firefox 155)

`audio-debug.html` åbnet over LAN (http — ingen mikrofon, ingen tilladelser):

- Tone **uden** videospor → lyd i **earpluggene** ✅
- Tone **med** videospor → lyd i **earpluggene** ✅

Altså: fjernlyd gennem WebRTC alene ruter korrekt på denne telefon, og
video-/speakerphone-tilstand er udelukket som årsag. Tilbage er capture-delen:
Android holder audio-mode i kommunikationstilstand så længe et mikrofon-spor er
aktivt. Konsekvens i frontenden: "sluk mikrofon" friger nu sporet helt
(`track.stop()` + fjern fra peer-forbindelserne) i stedet for `enabled = false`
— det er både den sandsynlige fix og det mest ærlige "sluk" (browseren optager
intet mens knappen siger slukket). e2e dækker frigivelse og genoptagelse.

Næste skridt: verificér i det deployede miljø på telefonen — sluk mikrofonen
mens en anden deltager taler, og se om fjernlyden flytter til earpluggene.
Kræver at ændringen er deployet til `loft-test`.

### Telefon-måling 2: capture er synderen (bekræftet 2026-09-13)

Samme telefon, nu `audio-debug.html` over https (midlertidigt kopieret ind i
`loft-web`-pod'et):

1. Tone uden capture → **earplugs** (også med videospor).
2. Mens tonen kørte: mikrofon-capture aktiveret → **lyden forsvandt fra
   earpluggene**, og det samme gjorde en podcast i en anden app. Routen sidder
   altså fast i opkaldsprofil for hele telefonen — ikke kun for vores side.
3. **«Frigiv (stop spor)» → tonen og podcasten kom tilbage til earpluggene.**

Konklusion: det er capture-delen der flytter lyden, og routen kan genoprettes
ved at slippe sporet. Derfor er fixet "sluk = slip capture" rigtigt; det er
allerede i `loft.html` (og på testmiljøet via ad-hoc-kopien).

Følgekonsekvens: fordi mikrofonen auto-starter ved join, får en Android-lytter
fjernlyden ud af telefonens højttaler indtil mikrofonen slukkes. Derfor er "join
muted" (mikrofonen tændes først når man selv vil tale) det næste, naturlige
skridt — det er en produktbeslutning, ikke længere et teknisk spørgsmål.
For en bruger der *taler* er der ingen klientside-vej: så længe capture er
aktiv, vælger Android kommunikationsruten, og Firefox sætter ikke Bluetooth-
headsettet som communication device.

### Kendt begrænsning: talende brugere på Android (bekræftet 2026-09-13)

Med "join muted" på plads blev begge ben målt i Loft (laptop + telefon):

- **Telefonens mikrofon slukket** → laptoppens lyd kommer ud af earpluggene,
  blandet ind oveni podcasten (ingen overtagelse af ruten). ✅ Løsningen virker
  for lytteren — det var det oprindelige fund.
- **Telefonens mikrofon tændt** → lyden går til telefonens højttaler og giver
  rundhyl, altså ikke gennem earpluggene. ❌

Årsagen er den samme kommunikationsrute: så snart der er capture, vælger Android
opkaldsprofilen, og Firefox (Android) sætter ikke Bluetooth-headsettet som
communication device. Det kan frontenden ikke omgå — `setSinkId` findes ikke på
Android (se ovenfor).

Frontenden advarer derfor nu på Android når mikrofonen tændes (statuslinjen
fortæller at lyden kan gå til højttaleren og at sluk bringer den tilbage til
earpluggene). Mulige afbøderinger at afprøve:

- Chrome for Android som talekanal (Chrome har egen Bluetooth-håndtering) —
  virker lyden i earpluggene der, er det en Gecko-begrænsning og grundlag for en
  bugrapport.
- Ledningsbaseret headset på telefonen i stedet for Bluetooth.
- Lade telefonen være lytteren og tale fra en computer i samme loft.

### Diagnostikværktøj til telefonen

`frontend/audio-debug.html` (åbnes på telefonen, fx
`https://loft.test.gihc.online/audio-debug.html`) laver sin egen
`RTCPeerConnection`-loopback i siden, så fjernlyden går gennem præcis samme sti
som i Loft (remote track → `<audio>`-element):

1. **Miljø** — UA, secure context, `setSinkId`/`selectAudioOutput`-support.
2. **Enheder** — `enumerateDevices()` før/efter mikrofon-tilladelse.
3. **Tone** — 440 Hz gennem WebRTC, med valgfrit videospor (kamera/skærm-scenariet).
4. **Mikrofon** — lokal loopback med "sluk (mute)" og "frigiv (stop spor)".
5. **Output-enhed** — `setSinkId` på den kørende fjernlyd.
6. **Rapport** — JSON til copy/paste, så fundet kan deles uden devtools på telefonen.

Testplan på telefonen (samme earplugs hele vejen):

1. **Uafhængig kontroltest** (ikke vores kode). Google Meet er en død ende:
   Meet-web fejler i Firefox på Android med "Kan ikke færdiggøre forespørgslen,
   prøv igen" (fundet 2026-09-13) inden der overhovedet kommer et møde op.
   Brug i stedet WebRTC-projektets eget lydeksempel:
   `https://webrtc.github.io/samples/src/content/peerconnection/audio/` —
   `pc1`↔`pc2` i samme side, remote lyd i et `<audio>`-element, ingen konto og
   ingen signaleringsserver. Lander lyden i earpluggene der, er problemet vores
   frontend; går den i højttaleren, er det Firefox/Android, og `setSinkId` er den
   eneste klientside-vej. (Alternativ med to deltagere: Jitsi på telefonen +
   laptop som den anden part.)
2. Samme kontrol i **Chrome** på telefonen (hvis installeret) for at se om det er
   Firefox-specifikt.
3. **Ren telefon-test uden deploy:** spil musik i earpluggene (fx en podcast),
   åbn `https://loft.test.gihc.online` på telefonen og deltag i et loft —
   mikrofonen starter automatisk. Holder musikken op eller skifter profil, når
   capture starter? Det viser om Android skifter audio-mode (`MODE_IN_COMMUNICATION`,
   SCO/HFP) alene på grund af mikrofonen, uafhængigt af om der er andre deltagere.
4. `audio-debug.html`: kør tonen og skift output-enhed. Kommer der enheder i
   listen, og virker skiftet?
5. Mens tonen kører: sluk mikrofonen (mute) og derefter frigiv den helt. Skifter
   lyden tilbage til earpluggene, når capture slippes (SCO/HFP vs. A2DP)?
6. Gentag med "medtag videospor" slået til (video-sessioner bruger typisk
   speakerphone-tilstand).

- [x] Diagnostikværktøj: `frontend/audio-debug.html` + e2e i begge browsere
- [x] `setSinkId`-vælger i `loft.html` ("Lyd ud" i værktøjslinjen, gemt i
      localStorage `loft.sinkId`, skjult når browseren ikke eksponerer enheder)
- [ ] Afklar om det er Firefox-specifikt: Google Meet i **Firefox** på samme
      telefon med samme earplugs (punkt 1–2 ovenfor — kræver brugerens telefon;
      brug WebRTC-lydeksemplet, ikke Meet)
- [x] Afklaret 2026-09-13: **video er udelukket** (tone med videospor gik til
      earpluggene), og capture er synderen — et aktivt capture flytter hele
      telefonens medierute til opkaldsprofil (podcasten blev tavs), og
      frigivelse flytter den tilbage. Se telefon-målingerne ovenfor
- [x] Verificér på telefonen at frigivelse af capture flytter fjernlyden tilbage
      til earpluggene — bekræftet i `audio-debug.html` 2026-09-13 (podcasten kom
      tilbage). End-to-end med to deltagere (laptop + telefon) blev kørt
      2026-09-13 og bekræftede fundet — se "Kendt begrænsning: talende brugere
      på Android".
- [x] "Join muted" indført 2026-09-13: mikrofonen starter ikke ved join, så en
      Android-lytter beholder earpluggene; "🎙 Tænd mikrofon" er den primære
      handling, og capture frigives igen ved sluk. e2e opdateret i begge browsere
      (en peer-forbindelse etableres nu først når nogen deler medier)
- [x] Afprøv `setSinkId` på telefonen — afklaret 2026-09-13 uden telefon:
      API'et findes slet ikke på Android (MDN/Bugzilla), så denne vej er lukket
- [x] Mikrofonen frigives når den slukkes ("Sluk mikrofon" kalder `track.stop()`
      og fjerner sporet fra peer-forbindelserne) — implementeret 2026-09-13, se
      README og "Join muted" ovenfor

## GDPR (forenklet)

- [x] Ingen konti, emails eller beskedhistorik i MVP — minimal persondata
- [ ] Dokumentér opbevaring: lofts-tabel (navn + tidsstempler) med TTL;
      deltagernavne findes kun i hukommelsen under sessionen
