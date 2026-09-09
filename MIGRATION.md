# Migration af chat til k3s + refokus til Loft — plan

Status: 2026-09-09 · branch `feat/loft-k3s-refocus`

Dette dokument afløser den tidligere k3s-plan (2026-08-01) og er skrevet sammen
med beslutningen om at fokusere projektet: chat-appen bliver **Loft** — WebRTC
link-rum (lyd, video og skærmdeling) med gæsteadgang, hvor man deler et link via
Matrix, XMPP eller mail. Modellen er "Plan B" fra den tidligere plan: rummet
lever videre efter at skaberen går, indtil det lukkes eller udløber, og URL'en
er adgangsnøglen.

Forudsætninger og platform-tilstand står i `../infra/MIGRATION.md` — vigtigst:

- Platformen (ingress-nginx, cert-manager, local-path-storage,
  secrets-encryption) er klar og bevist af hyfer og capture.
- **Der er ingen data at migrere.** Den gamle server blev slettet 2026-07-04
  før clusteret blev bygget. Migrationen er reelt en frisk deploy.
- Etableret mønster (hyfer, capture): app-repoet ejer sine egne manifester i
  `k8s/`, images bygges i GitHub Actions og pushes til GHCR som public
  packages, secrets hentes fra `pass` og oprettes imperativt med
  `kubectl create secret`, TLS via annotation `cert-manager.io/cluster-issuer`
  (staging → verificér → prod).

## Beslutninger (taget 2026-09-09)

1. **Produkt:** kun Loft. Chat-rum, DM, filoverførsel, konti og IPFS fjernes
   fra den aktive sti i M1–M5 (git-historik bevares). Navn, domæne og image
   hedder `loft`.
2. **Gæsteadgang (Plan B):** ingen konti/JWT/email i MVP. Loft-URL'en
   (tilfældig UUID) er adgangsnøgle; deltageren vælger selv vist navn. Fase 2
   kan tilføje konti/ejerskab uden at ændre signalprotokollen.
3. **Frontend:** statisk nginx-image pr. miljø, `config.js` mountet som
   ConfigMap. IPFS/DNSLink og ADR-0008 udgår; ADR-draft 0027 erstatter dem.
4. **Domæner:** nyt skema, ét origin pr. miljø — `loft.test.gihc.online`
   (test) og senere `loft.gihc.online` (prod). Frontend, REST (`/v1`) og
   WebSocket deler host, så CORS/CSP-par-koblingen forsvinder. `ALLOWED_ORIGIN`
   bevares kun til lokal udvikling.
5. **coturn:** beholdes — WebRTC uden TURN fejler bag NAT. Kører som
   `hostNetwork`-Deployment; firewall udvides i `infra/tofu/main.tf`:
   TCP+UDP 3478 og UDP 49152–49200.
6. **Postgres:** én lille instans pr. miljø-namespace — kun `lofts`-tabellen
   (id, navn, created_at, last_active). Deltagere, presence og signalering er
   in-memory → præcis 1 replica.
7. **Secrets:** `postgres-password` + `database-url` i `pass` → secret
   `loft-secrets`. `TURN_SECRET` er reelt offentlig (HMAC-baserede credentials
   med 24 t TTL) og ligger derfor i ConfigMap for test.
8. **CI:** GitHub Actions → `ghcr.io/gihc-org/loft` (+ `loft-web`), SHA-tags +
   latest, buildx med GHA-cache (Rust-build er langsom). `.woodpecker.yaml`
   udgår ved oprydning.

## Navngivning

| Rolle | Værdi |
|-------|-------|
| Produkt | Loft |
| Test-domæne | `loft.test.gihc.online` |
| Prod-domæne | `loft.gihc.online` |
| API-image | `ghcr.io/gihc-org/loft` |
| Web-image | `ghcr.io/gihc-org/loft-web` |
| Namespace (test) | `loft-test` |
| Manifester | `k8s/test/` (prod: `k8s/prod/` senere) |
| DNS-script | `scripts/create-dns-record.sh` |

## Trin-for-trin

0. **Dokumentation (denne gren):** TODO.md + MIGRATION.md omskrevet,
   ADR-drafts 0027/0028, `k8s/test`-skelet, GitHub Actions-workflow,
   DNS-script og frontend-Dockerfile. Intet deployet endnu.
1. **Backend (Loft-kerne):** migration `lofts`; `POST /v1/lofts`
   (rate-limited) og `GET /v1/lofts/:id`; WS `/v1/ws/:loft_id` med
   `join`/`leave`/`roster`/`presence`/`signal`; `/healthz`; TTL-oprydning.
   Fjern auth-, room-, DM- og file-stier fra routeren i takt med at WS'eren er
   omskrevet. Dockerfile: non-root + read-only fs (CIS-punkter i TODO).
2. **Frontend (Loft-UI):** ny `loft.html` (opret/deltag via link, navn,
   mic/cam, skærmdeling, forlad) og `rtc.js` med mesh + perfect negotiation
   (refaktor af chat.html's ene 1:1-`RTCPeerConnection`). Genbrug ICE/TURN-
   logik og auto-reconnect. Invite: copy-link, `navigator.share`, QR.
3. **Tests + CI:** Playwright med tre kontekster (connected, skærmdeling,
   leave/rejoin, link fra frisk kontekst); cargo-tests for loft-registry og
   WS-signalering; workflow bygger begge images; GHCR-pakker gøres public.
4. **Deploy test:** DNS A-record (scriptet), secret `loft-secrets`, apply
   `k8s/test`, firewall-regler i infra/tofu, `letsencrypt-staging` → verificér
   → prod-issuer, smoke-test omskrevet til `kubectl exec` + gæste-loft-flow.
5. **Beta/prod:** kopiér manifester til `k8s/prod/` (realm/domæner skiftes),
   deploy, skift DNS til `loft.gihc.online`.
6. **Oprydning:** slet `docker-compose*.yml`, `ansible/`, `caddy/`,
   `.woodpecker.yaml`, kubo/DNSLink-rester og døde DNS-records
   (`api.gihc.online`, `chat.apps.gihc.online`, `beta.*`, `test.*`). Opdater
   `runbooks/`, `AGENTS.md` og ADR-index i `~/projects/adrs/`; skriv referat.

## Tekniske noter / gotchas

- **WebSocket-timeouts:** ingress-nginx `proxy-read-timeout` /
  `proxy-send-timeout` = 3600 på `/v1/ws/...` — forbindelsen er langlivet.
  `proxy-body-size` udgår når filoverførsel fjernes.
- **Security headers:** globale basis-headers (nosniff, frame-deny,
  referrer-policy) sættes i `infra/tofu/platform.tf`. CSP og
  `Permissions-Policy` (kamera/mikrofon = self) sættes i app-laget — nginx-conf
  for web, Axum-layer for API — fordi ingress' `configuration-snippet` er slået
  fra som standard.
- **1 replica:** in-memory lofts/presence gør flere replicas meningsløse.
  `Recreate`-strategi bruges ved deploy (postgres-PVC + korte nedtider).
  Rolling updates dropper WS-forbindelser — frontend auto-reconnect dækker det.
- **Databasemigrationer** kører embedded ved app-start (`sqlx::migrate!`) —
  ingen separat Job nødvendig.
- **Ingress-routing:** backend nester selv på `/v1`; frontend på `/`. Ingen
  path-rewrites. Link-preview/OG er en åben M2-beslutning: backend-rendret
  `/h/:id`-landing (anbefalet) eller generiske og-tags i web-nginx.
- **GHCR-pakker er private som standard** — gør dem public manuelt første
  gang, ellers skal der `imagePullSecrets` til.
- **Rollback:** deploy med SHA-tag; `kubectl set image deployment/loft-api
  loft-api=ghcr.io/gihc-org/loft:<forrige-sha>`.
- **DNS-resolveren på serveren er allerede rettet** (infra, Ansible) — nye
  records slår hurtigt igennem for cert-manager's self-check.

## Efter migrationen

- Flyt accepterede ADR-drafts til `~/projects/adrs/` (nr. 0027 og 0028).
- Omdøb `chat/`-kataloget til `loft/` når backend-fokuseringen er gennemført,
  og genovervej repoets navn.
- Skriv referat i `referater/` efter hver væsentlig session, som sædvanligt.
