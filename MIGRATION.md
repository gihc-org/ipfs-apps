# Migration af ipfs-apps til k3s — plan

Udarbejdet 2026-07-25. Forudsætninger og overordnet tilstand står i
`../infra/MIGRATION.md` — det vigtigste herfra:

- Platformen (ingress-nginx, cert-manager, local-path-storage,
  secrets-encryption) er klar og bevist af hyfer og capture.
- **Der er ingen data at migrere.** Den gamle server blev slettet 2026-07-04
  før clusteret blev bygget; postgres-data, uploads og IPFS-repoet er væk.
  Migrationen er reelt en frisk deploy — ingen pg_dump/restore, ingen
  volume-kopi.
- Etableret mønster (hyfer, capture): app-repoet ejer sine egne manifester i
  `k8s/`, images bygges i GitHub Actions og pushes til GHCR som public
  packages, secrets hentes fra `pass` og oprettes imperativt med
  `kubectl create secret`, TLS via annotation `cert-manager.io/cluster-issuer`
  (staging → verificér → prod).

## Åbne beslutninger

Tag disse først — de styrer resten af planen.

### 1. Frontend: IPFS eller statisk image?

Den største beslutning. I dag serveres frontend af kubo-gatewayen med
Host-header-baseret DNSLink-resolution, og deploy-flowet kører `ipfs add -r`
og opdaterer `_dnslink.*` TXT-records hos Simply.com.

- **(a) Behold IPFS:** kubo som pod + PVC, gateway bag ingress (ingress-nginx
  videresender Host-headeren som standard, så den del er enkel). Kræver at
  `ipfs add` + DNSLink-TXT-opdatering automatiseres i CI pr. miljø. Bevarer
  ADR-0008 og projektets IPFS-identitet, men er markant mere komplekst.
- **(b) Statisk image:** frontend pakkes i et lille nginx-image pr. miljø med
  `config.js` bagt ind (eller mountet som ConfigMap). Simpelt, matcher
  hyfer/capture-mønsteret, men opgiver IPFS/DNSLink — kræver at ADR-0008
  erstattes af en ny ADR.

Anbefaling: (b), medmindre IPFS-hostingen er et formål i sig selv.

### 2. coturn / WebRTC: beholdes, og hvor?

coturn kan ikke ligge bag ingress-nginx (rene TCP/UDP-porte: 3478 +
49152–49200/udp). Muligheder:

- **(a) `hostNetwork: true`-pod i k3s** + firewall-regler i `infra/tofu/main.tf`
  (TCP+UDP 3478, UDP 49152–49200). Det eneste reelt blokerende infra-arbejde.
- **(b) Droppes foreløbigt**, hvis WebRTC/video ikke bruges i praksis — så er
  der ingen infra-blokering overhovedet.

Bemærk: `TURN_SECRET` indsættes i dag i frontend-`config.js` og er reelt
offentlig (HMAC-baserede credentials genereres klient-side) — ingen
hemmelighedsproblem at løse her.

### 3. Domæneskema

- **(a) Genbrug gamle domæner** (`api.gihc.online` + `chat.apps.gihc.online`,
  `beta.*`, `test.*`): peger allerede på serverens IP → nul DNS-arbejde, ingen
  URL-ændring.
- **(b) Nyt skema** som capture/hyfer (`chat.test.gihc.online` osv.):
  konsistent på tværs af apps, men kræver nye DNS-records + oprydning af gamle.

Uanset valg: frontend og API er et **par** — CSP `connect-src` og
`ALLOWED_ORIGIN` skal pege på hinanden pr. miljø.

### 4. JWT_SECRET: delt eller per miljø?

I dag delt på tværs af test/beta/prod. Da der alligevel startes forfra (ingen
data, ingen gyldige tokens at bevare), anbefales **én secret per miljø**.

### 5. postgres-layout

I dag én instans med tre databaser (`chatdb`, `chatdb_test`, `chatdb_beta`).

- **(a) Én postgres per miljø-namespace** (pod + PVC i hvert namespace):
  selvstændige miljøer, idiomatic k8s, lettere at begrunde om i manifesterne.
  Tre postgres-pods koster lidt ekstra RAM (serveren har 4 GB).
- **(b) Én delt postgres** med tre databaser (som i dag): skal ligge i et
  delt namespace, og de to ekstra databaser skal oprettes (ConfigMap med
  init-SQL i `/docker-entrypoint-initdb.d/`, eller imperativt én gang).

Anbefaling: (a) — simplere manifester, ingen cross-miljø-kobling.

### 6. CI: GitHub Actions eller Woodpecker?

Repoet har `.woodpecker.yaml`, men Woodpecker kører ikke endnu (det er et
ønske, se `../infra/TODO.md`). hyfer og capture bruger GitHub Actions → GHCR.
Anbefaling: GitHub Actions-workflow nu (kopier captures `.github/workflows/
build.yml`-mønster); Woodpecker-deploy kan altid lægges ovenpå senere.

## Forudsætninger i infra (før deploy)

1. **Firewall** — kun hvis coturn beholdes: TCP+UDP 3478 og UDP 49152–49200
   tilføjes `hcloud_firewall.platform` i `infra/tofu/main.tf` + `tofu apply`.
2. **Security headers** — i dag sat af Caddy. Aftalt linje (jf.
   `../infra/MIGRATION.md`): globale basis-headers (`nosniff`, frame-deny,
   referrer-policy) via ingress-nginx' ConfigMap i `infra/tofu/platform.tf`;
   CSP sættes per app/miljø som ingress-annotationer, fordi den peger på
   API-domænet.
3. **Secrets fra `pass`** — oprettes imperativt pr. namespace, intet i git:
   ```
   kubectl create secret generic chat-secrets -n <namespace> \
     --from-literal=postgres-password=$(pass ...) \
     --from-literal=jwt-secret=$(pass ...) \
     --from-literal=turnstile-secret=$(pass ...) \
     --from-literal=resend-api-key=$(pass ...)
   ```
   Husk feature-flag-konventionen (ADR-0012): tom `TURNSTILE_SECRET` slår
   CAPTCHA fra, tom `RESEND_API_KEY` auto-verificerer — test/beta kører med
   tomme værdier.

## Trin-for-trin

1. Tag beslutningerne 1–6 ovenfor.
2. **GitHub Actions-workflow**: byg `chat`-imaget → `ghcr.io/gihc-org/chat`
   (public package, ellers kræves `imagePullSecrets`). Brug buildx med GHA
   cache — Rust-build er langsomt. Tag med git SHA, ikke kun `:latest`
   (rollback-mulighed). Evt. også frontend-image hvis beslutning 1 = (b).
3. **`k8s/`-manifester**, ét sæt per miljø (start med test):
   - `namespace.yaml` — fx `chat-test`.
   - `postgres.yaml` — Deployment (1 replica, `Recreate`) + PVC + ClusterIP-
     service + secret-reference. Kun hvis beslutning 5 = (a); ellers ét delt
     sæt + init-SQL.
   - `deployment.yaml` — chat, 1 replica (`Recreate`: PVC + in-memory WS/
     presence-state gør flere replicas meningsløse). env fra secret + ConfigMap
     (`DATABASE_URL`, `ALLOWED_ORIGIN`, `BASE_URL`, `FRONTEND_URL` m.fl.).
     Tilføj probes (TCP på 8080 er nok; app'en har ingen `/healthz` endnu) og
     resource requests/limits — se også CIS-punkterne i `TODO.md` (non-root,
     read-only fs), som kan tages med her.
   - `service.yaml` — ClusterIP 80 → 8080.
   - `ingress.yaml` — API + frontend (to hosts eller to filer), med
     annotationerne fra "Tekniske noter" nedenfor. Start med
     `letsencrypt-staging`, verificér, skift til `letsencrypt-prod`.
4. **DNS** — kun hvis nyt skema: opret A-records via Simply.com API'et
   (capture har et genbrugeligt idempotent script,
   `../capture/scripts/create-dns-record.sh`).
5. **Verificér test** — curl + login-flow + WebSocket + upload. Omskriv
   `scripts/smoke-test.sh` fra `docker compose exec postgres psql` til
   `kubectl exec`.
6. **Beta og prod** — gentag per miljø.
7. **Oprydning** (når prod kører stabilt): slet `docker-compose*.yml`,
   `ansible/`, `caddy/`, opdatér `runbooks/` og `AGENTS.md` til k8s-flowet,
   slet døde DNS-records hos Simply.com hvis skemaet skiftede.

## Tekniske noter / gotchas

- **Ingress-annotations for chat-API:**
  - `nginx.ingress.kubernetes.io/proxy-body-size: "50m"` (uploads op til
    50 MB — default er 1m).
  - `nginx.ingress.kubernetes.io/proxy-read-timeout` /
    `proxy-send-timeout` sat højt (fx 3600) — WebSocket på `/v1/ws/:room_id`
    er langlevet.
  - Rate limiting keyed på `X-Forwarded-For` virker uændret — ingress-nginx
    sætter headeren korrekt som standard.
- **Databasemigrationer** kører embedded ved app-start
  (`sqlx::migrate!`) — ingen separat Job nødvendig.
- **Rolling updates dropper WS-forbindelser** — acceptabelt, frontend har
  auto-reconnect med backoff. `Recreate`-strategien giver kort nedetid ved
  deploy, som i dag.
- **Backend nester selv routeren på `/v1`** — ingress routes hele hosten, ingen
  path-rewrites.
- **kubo (hvis IPFS beholdes):** PVC til `/data/ipfs`, gateway 8080 bag
  ingress med Host-header passthrough (standard). Swarm-porte er ikke
  eksponeret i dag og skal heller ikke være det.
- **GHCR-pakker er private som standard** — gør dem public manuelt første
  gang (som for hyfer/capture), ellers skal der `imagePullSecrets` til.
- **DNS-resolveren på serveren er allerede rettet** (infra, Ansible) — nye
  records slår igennem hurtigt for cert-manager's self-check.

## Efter migrationen

- Genovervej `.woodpecker.yaml` — enten omlæg til det nye flow eller slet,
  når GitHub Actions bærer bygget.
- Skriv ADR der afløser ADR-0019 (platform-Caddy) — og hvis IPFS droppes,
  en der afløser ADR-0008.
- Referat i `referater/` efter hver væsentlig session, som sædvanligt.
