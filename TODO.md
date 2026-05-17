# TODO

## Playwright e2e tests

- [x] Opsæt Playwright i `e2e/` med fake mikrofon/kamera (Chromium-flags)
- [x] Auth-test: registrering og login-flow
- [x] Call-test: ring op → accepter → læg på (to browser-contexts)
- [ ] Screen share-test: del skærm → modtager ser video → stop
- [ ] Integrer i CI-pipeline (kræver Chromium i CI-image)

### Integration i deploy-flow

- [x] Opret test-miljø på VPS (`test.chat.apps.gihc.online` / `test.api.gihc.online`) — `chat-test` service, Caddy-blokke, DNS, IPFS/DNSLink via Ansible
- [x] Opret `.woodpecker.yaml` — pipeline: `deploy-test` → e2e mod test-miljøet → `deploy-promote` (beta + prod)
- [x] Playwright-image `mcr.microsoft.com/playwright:v1.44.0-jammy` bruges som e2e CI-image
- [x] E2e-trinnet gater deploy til beta/prod via `depends_on`
- [ ] Konfigurér secrets i Woodpecker UI: `vault_password` og `ssh_private_key`

---

## CI/CD — Woodpecker CI på Raspberry Pi

- [ ] Installer Woodpecker server + agent på Raspberry Pi via Docker Compose
- [ ] Forbind til GitHub som forge (OAuth app)
- [ ] Tilføj `VAULT_PASSWORD` som krypteret pipeline-secret i Woodpecker UI
- [ ] Opret `.woodpecker.yaml` i repo med Ansible deploy-step
- [ ] Konfigurer cron-job i Woodpecker UI (fx hvert 10. minut) som pull-mekanisme
- [ ] Valgfrit: tilføj webhook fra GitHub til Pi for øjeblikkelig deploy ved push
- [ ] Kør `e2e/` Playwright-tests i pipeline før deploy (kræver Chromium i CI-image)
- [ ] Opsæt staging-/testmiljø (separat VPS eller Docker Compose lokalt på Pi) som forudsætning for fuld OWASP ZAP-scan og Playwright e2e i CI — uden testmiljø kan aggressiv scanning og browser-tests ikke køre mod prod

### Allerede automatiseret

- [x] Post-deploy smoke test (`scripts/smoke-test.sh`) — kører automatisk sidst i Ansible-playbook'en og tester login, GET /auth/me, GET /rooms og DELETE /auth/me mod prod

---

## Sikkerhed — OWASP-scanning

Se `OWASP-IMPROVEMENTS.md` for kendte fund og anbefalinger til kodeændringer.
Nedenfor er værktøjer til løbende validering.

### 1. HTTP security headers — verificer efter næste deploy

Kør mod live URL efter deploy for at bekræfte at Caddyfile-headerne virker:

- https://securityheaders.com/?q=https://api.gihc.online
- https://observatory.mozilla.org/analyze/api.gihc.online

Forventet resultat: mindst A baseret på de headers vi tilføjede i commit c09b52d.

### 2. OWASP ZAP — dynamisk scanning mod kørende app

- [x] Passiv baseline-scan integreret i `ansible/playbook.yml` — kører automatisk efter smoke test ved hvert deploy (`ghcr.io/zaproxy/zaproxy:stable`, fejler ved FAIL-level alerts)
- [ ] Fuld scan (aggressiv) — kræver testmiljø, se punkt nedenfor

Fuld scan køres manuelt mod testmiljø når det er opsat:

```bash
docker run --rm ghcr.io/zaproxy/zaproxy:stable \
  zap-full-scan.py -t https://<testmiljø-url>
```

### 3. cargo audit — dependency-scanning

```bash
cd chat && cargo audit
```

Tilføj som task i `ansible/playbook.yml` inden `docker compose build` — se
anbefaling i `OWASP-IMPROVEMENTS.md`.

---

## Sikkerhed — tilbageværende kodeændringer

Se `OWASP-IMPROVEMENTS.md` for detaljer og kodeeksempler.

- [ ] Rate limiting på `/auth/token`, `/auth/register`, og `/dms` POST (`tower_governor`)
- [ ] JWT-token revokering ved logout
- [ ] Verifikationstoken udløber efter 48 timer
- [ ] Account lockout efter gentagne fejlede loginforsøg
- [ ] Sikkerhedslogning med `tracing::warn!` på auth-hændelser (✅ delvist implementeret for DM-adgang)
- [ ] JWT i `httpOnly`-cookie frem for `localStorage`
- [ ] `email`-felt NOT NULL i database

---

## GDPR-compliance

Projektet gemmer persondata (email, brugernavn) på EU-borgere. Se ADR-0014.

- [x] `DELETE /auth/me` — ret til sletning (sletter bruger + DM-rum; beskeder og medlemskaber via CASCADE)
- [x] `GET /auth/me` — returnerer alle gemte felter: id, username, email, email_verified, created_at
- [x] Tilføj privacy policy-side til frontend (`frontend/privacy.html`) med oplysning om hvad der gemmes og hvorfor; link tilføjet til registreringsformularen
- [x] Dokumentér dataopbevaring — politik: konti og beskeder gemmes uden tidsbegrænsning indtil brugeren sletter sin konto (DELETE /auth/me); ved sletning fjernes beskeder og rum-medlemskaber via ON DELETE CASCADE; ingen automatisk sletning af inaktive konti
- [x] Bekræft at PostgreSQL-data ikke replikeres til tredjelande — VPS kører hos Hetzner i Helsinki, Finland (EU)
- [x] Verifikationstoken er persondata — slettes ved verify (`verification_token = NULL`) og ved sletning af konto (hele brugerrækken slettes via CASCADE); eksponeres aldrig i API-svar (`#[serde(skip)]`)

---

## WebSocket auto-reconnect med exponential backoff

- [x] Exponential backoff ved reconnect: 1s → 2s → 4s → ... → 30s max; reset ved vellykket forbindelse
- [x] Fjern `hangUp()` og `stopScreenShare()` fra `ws.onclose` — WebRTC håndterer sit eget lifecycle via `onconnectionstatechange`
- [x] Stop reconnect ved sidenavigation (`beforeunload`)
- [x] Vis ventetid i statuslinjen ("forsøger igen om Xs…")

---

## Online-indikator under direkte beskeder

Vis en grøn prik ud for DM-kontakter der aktuelt er forbundet til serveren.

- [x] **Backend**: tilføj global `online_users: Arc<RwLock<HashMap<Uuid, u32>>>` i `AppState` — opdateres når en WS-forbindelse åbnes/lukkes (ref-counted for multiple tabs)
- [x] **Backend**: ny WS-beskedtype `presence` — broadcastes i rummet når en brugers online-status skifter
- [x] **Backend**: `GET /v1/presence` — returnerer liste af online bruger-UUIDs (kræver auth)
- [x] **Frontend** (`rooms.html`): hent `/v1/presence` ved sideload og vis grøn prik ud for online DM-kontakter; opdateres hvert 30. sekund
- [ ] **Frontend**: opdater prikken i realtid via `presence`-besked i WS (kræver WS-forbindelse på rooms.html)

---

## WebRTC skærmdeling (ADR-0016)

### Fase 1 — 1:1 skærmdeling i DM-rum

- [x] **Backend** (`src/routes/chat.rs`): viderebringe `signal`-beskeder i WS-broadcast med `from` sat til autentificeret brugers UUID; ikke gemme i database
- [x] **Frontend** (`frontend/chat.html`): "Del skærm"-knap i DM-rum, `getDisplayMedia()`, `RTCPeerConnection` med STUN, stop-knap
- [x] **Frontend**: modtager-side viser indgående stream i `<video>`-element; håndter offer/answer/ICE-candidate flow
- [x] **Integration test**: signal-beskeder videresendes korrekt og gemmes ikke
- [x] **TURN-server** (`coturn`) i `docker-compose.prod.yml` — løser NAT-traversal på tværs af netværk; HMAC-SHA1 tidsbegrænsede credentials via `--use-auth-secret`
- [x] **Rejoin** — sharer sender automatisk nyt offer når peer vender tilbage til DM-rum
- [ ] **Frontend**: disable "Del skærm"-knappen når peeren ikke er online (track tilstedeværelse via WS `join`/`leave`-beskeder eller heartbeat)

### Beslutninger der mangler afklaring

- [ ] **Modtager-notifikation**: skal der vises en synlig notifikation ("X deler sin skærm") når skærmdeling starter, så modtageren ikke overser videoen?
- [ ] **TURN-fejlhåndtering**: skal brugeren have en besked hvis TURN-serveren er nede og forbindelsen fejler, frem for stille at falde tilbage til STUN (som kan fejle bag NAT)?
- [ ] **TURN-credential TTL**: credentials udløber efter 24 timer — skal siden automatisk forny dem (re-kalde `addTurnServer()` og genstarte `RTCPeerConnection`), eller er en reload-besked til brugeren nok?

### Fase 2 — Gruppe-huddles (fremtidig)

- [ ] Ny ADR for gruppe-topologi (mesh vs. SFU)

---

## WebRTC opkald i DM-rum (fremtidig)

1:1 lyd- og videoopkald via WebRTC i DM-rum. Genbruger eksisterende signal-infrastruktur og TURN-server.

- [ ] ADR for opkald (beslut: ringesignal-flow, afvis/accepter UI, delt RTCPeerConnection med skærmdeling)
- [ ] **Ringesignal**: ny signal-type `call-invite`/`call-accept`/`call-reject` — modtageren ser en indgående opkaldsboks med accepter/afvis
- [ ] **Backend**: ingen ændringer forventet — signal-forwarding håndterer de nye typer uændret
- [ ] **Frontend**: "Ring op"-knap i DM-rum (kun synlig når peer er online), `getUserMedia({ audio: true })`, delt `RTCPeerConnection` med skærmdeling via renegotiering
- [ ] **Frontend**: indgående opkald vises som overlay med ringetone (Web Audio API eller `<audio>`) og accepter/afvis-knapper
- [ ] **Frontend**: aktiv opkald-UI — dæmp mikrofon, læg på; kan kombineres med aktiv skærmdeling
- [ ] **Fase 2**: kamera-video — `getUserMedia({ audio: true, video: true })`, `<video>`-element til modtager

---

## CIS Docker Benchmark

Hærdning af Docker-opsætning. Se ADR-0014.

- [ ] Kør backend-container som non-root bruger — tilføj til `chat/Dockerfile`:
  ```dockerfile
  RUN useradd -m appuser
  USER appuser
  ```
- [ ] Tilføj resource limits i `docker-compose.yml`:
  ```yaml
  deploy:
    resources:
      limits:
        memory: 256m
        cpus: "0.5"
  ```
- [ ] Sæt `read_only: true` på containere der ikke skriver til filsystem (chat)
- [ ] Sæt `no-new-privileges: true` på alle services:
  ```yaml
  security_opt:
    - no-new-privileges:true
  ```
- [ ] Kør `docker scout cves` eller `trivy image` mod bygget image efter deploy
