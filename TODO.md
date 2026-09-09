# TODO — Loft (tidligere ipfs-apps/chat)

Projektet fokuseres til **Loft**: WebRTC link-rum (lyd, video, skærmdeling)
med gæsteadgang, delt via link over Matrix/XMPP/mail. Samtidig migreres fra
Caddy + Docker Compose til k3s. Se [MIGRATION.md](MIGRATION.md) og ADR-drafts i
`adr-drafts/`.

Afsluttet før refokuseringen (chat-rum, DM, filoverførsel, 1:1-skærmdeling og
lydopkald) ligger i git-historikken; Playwright- og WebRTC-mønstre derfra
genbruges.

## M0 — Fundament (dokumentation)

- [x] Beslutning: Loft link-rum (Plan B), gæsteadgang, statisk frontend
- [x] TODO.md og MIGRATION.md omskrevet til Loft + k3s
- [x] ADR-drafts 0027 (link-rum + k3s) og 0028 (mesh-topologi)
- [x] `k8s/test/`-skelet, GitHub Actions-workflow, DNS-script,
      frontend-Dockerfile
- [ ] Flyt ADR-drafts til `~/projects/adrs/` når de er accepteret

## M1 — Backend: Loft-kerne

- [ ] Migration: `huddles`-tabel (id, navn, created_at, last_active)
- [ ] `POST /v1/huddles` — opret link-rum, returnér `{ id, url }`
- [ ] `GET /v1/huddles/:id` — findes rummet? (til link-åbning)
- [ ] WS `/v1/huddles/:id` — gæste-join med navn; `join`/`leave`/`roster`/
      `presence`/`signal`; serveren sætter `from` på signaler
- [ ] `/healthz`-endpoint + k8s-probes
- [ ] TTL-oprydning af inaktive huddles (baggrundsjob eller lazy)
- [ ] Rate limiting på huddle-oprettelse (tower_governor, XFF-baseret)
- [ ] Fjern auth-, room-, DM-, file- og uploads-stier fra routeren i takt med
      at WS'eren er omskrevet
- [ ] Dockerfile: non-root-bruger (uid 10001), `no-new-privileges`,
      read-only rootfs (uploads-sti udgår)

## M2 — Frontend: Loft-UI

- [ ] `huddle.html` — opret/deltag via link, vælg navn, mic/cam-toggle,
      skærmdeling, forlad rum
- [ ] `rtc.js` — mesh-modul med ét `RTCPeerConnection` pr. deltager og
      perfect negotiation (refaktor af chat.html's 1:1-logik)
- [ ] Genbrug ICE/TURN-logik (`addTurnServer`) og auto-reconnect med backoff
- [ ] Invite: copy-link, `navigator.share`, QR-kode
- [ ] Link-preview: beslut backend-rendret `/h/:id` vs. generiske og-tags
- [ ] Deltager-UI opdateres i realtid via roster/presence-beskeder
- [ ] Nye sider afløser index/rooms/chat.html (slet når e2e er grøn)

## M3 — Tests + CI

- [ ] Playwright: tre kontekster i samme rum — connected, skærmdeling,
      leave/rejoin, link åbnet fra frisk kontekst
- [ ] Cargo-tests: huddle-registry og WS-signalering
- [ ] `.github/workflows/build.yml` bygger `loft` + `loft-web` (SHA-tags)
- [ ] GHCR-pakker gøres public
- [ ] E2e kører mod `loft.test.gihc.online` før prod-promote

## M4 — k3s deploy (test-miljø)

- [ ] A-record for `loft.test.gihc.online` via `scripts/create-dns-record.sh`
- [ ] Secret `loft-secrets` i `loft-test` (postgres-password, database-url)
- [ ] Apply `k8s/test/` (postgres, api, web, ingress, coturn)
- [ ] Firewall i `infra/tofu/main.tf`: TCP+UDP 3478, UDP 49152–49200
- [ ] `letsencrypt-staging` → verificér cert → `letsencrypt-prod`
- [ ] Smoke-test omskrevet til `kubectl exec` + gæste-huddle-flow
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

- [ ] Rate limiting på huddle-oprettelse og WS-håndtryk
- [ ] Capability-link er tilfældig UUID; verificér at ingress-logs ikke
      logger query/fragment med hvis tokens indføres
- [ ] Link-expiry/revocation (TTL på huddles)
- [ ] CSP: `default-src 'self'`, kamera/mikrofon via `Permissions-Policy`,
      ingen tredjeparts-scripts (Turnstile udgår)
- [ ] Containerhærdning: non-root, read-only fs, `no-new-privileges`
- [ ] `trivy image` mod byggede images + `cargo audit` i CI
- [ ] OWASP ZAP passiv baseline mod `loft.test.gihc.online`

## GDPR (forenklet)

- [x] Ingen konti, emails eller beskedhistorik i MVP — minimal persondata
- [ ] Dokumentér opbevaring: huddles-tabel (navn + tidsstempler) med TTL;
      deltagernavne findes kun i hukommelsen under sessionen
