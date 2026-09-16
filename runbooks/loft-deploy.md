# Deploy af Loft-testmiljøet (M4)

Runbook for `loft.test.gihc.online` på k3s-clusteret. Manifesterne ligger i
[k8s/test/](../k8s/test/README.md) og deployes manuelt — app-repoet ejer dem,
platformen (ingress-nginx, cert-manager, local-path, firewall) ejes af
`~/projects/infra`.

Trin 1–3 kræver adgang uden for dette repo (VPS, `pass`, `infra/tofu`). Spørg
brugeren før de køres.

## 0. Forudsætninger

```bash
# kubeconfig peger på 127.0.0.1:6443 — API'et er ikke eksponeret offentligt,
# så SSH-tunnelen skal være åben (samme mønster som hyfer/capture).
ssh -L 6443:localhost:6443 -N -f root@65.109.233.92
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/gihc.yml}"
kubectl get nodes
```

Images skal være bygget af `.github/workflows/build.yml` (push til `trunk`) og
GHCR-pakkerne gjort public første gang — ellers kræver pod'erne
`imagePullSecrets`:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' \
  https://ghcr.io/v2/gihc-org/loft/tags/list          # 401 = privat pakke
curl -sS -o /dev/null -w '%{http_code}\n' \
  https://ghcr.io/v2/gihc-org/loft-web/tags/list
```

Deploy med et SHA-tag i stedet for `latest` giver rollback-mulighed (se
afsnittet Rollback).

## 1. DNS

```bash
bash scripts/create-dns-record.sh                  # idempotent, bruger pass simply/*
dig +short loft.test.gihc.online                   # → 65.109.233.92
```

Scriptet opretter `A loft.test` hos Simply.com. TTL er 3600, og serverens
resolver er allerede rettet, så cert-manager's self-check slår hurtigt igennem.

## 2. TURN (platformens delte tjeneste)

Appen har **ingen egen coturn** længere. TURN er en delt platform-tjeneste i
`~/projects/infra` (namespace `coturn`, ADR 0003): der kan kun køre én coturn på
noden, fordi to instanser binder begge 3478 via `SO_REUSEPORT` og derefter
fordeler trafikken tilfældigt.

Det betyder for deployet her:

- `config.js` peger på `turn:turn.gihc.online:3478?transport=udp`.
- `TURN_SECRET` skal være identisk med platformens
  `pass turn/static-auth-secret`. Render linjerne med

  ```bash
  ~/projects/infra/scripts/turn-config.sh --config-js
  ```

- Firewall-portene (TCP+UDP 3478, UDP 49152–49200) ejes af platformen og står
  allerede åbne i `platform-firewall` — der er ingen "åbn"-kommando her.
- Misbrugsbeskyttelsen (kvoter og `--denied-peer-ip`) ligger i platformens
  `k8s/coturn/`-manifest. `TURN_SECRET` er reelt offentlig (den udleveres til
  browseren), så kvoterne er modvægten — de skal hæves i infra, hvis loftene
  vokser (hver peer-forbindelse i meshen bruger én allocation).
- Platformen verificeres app-uafhængigt med `~/projects/infra/scripts/check-turn.sh`.

## 3. Secret `loft-secrets`

```bash
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/gihc.yml}"
kubectl apply -f k8s/test/namespace.yaml

# Engang: gem postgres-adgangskoden i pass (samme mønster som capture).
# Brug et URI-sikkert alfabet — koden indlejres i DATABASE_URL nedenfor
# (se "Kendte faldgruber").
PG_PASS="$(openssl rand -hex 24)"
printf '%s' "$PG_PASS" | pass insert -m loft/postgres-password

# Idempotent oprettelse (kubectl create secret er ikke idempotent alene).
kubectl -n loft-test create secret generic loft-secrets \
  --from-literal=postgres-password="$PG_PASS" \
  --from-literal="database-url=postgres://loft:${PG_PASS}@postgres:5432/loftdb" \
  --dry-run=client -o yaml | kubectl apply -f -
```

Ved senere deploys hentes koden fra `pass` i stedet:

```bash
PG_PASS="$(pass loft/postgres-password)"
```

## 4. Apply manifesterne

```bash
kubectl apply -f k8s/test/configmap.yaml
kubectl apply -f k8s/test/
kubectl -n loft-test rollout status deploy/loft-postgres --timeout=180s
kubectl -n loft-test rollout status deploy/loft-api --timeout=180s
kubectl -n loft-test rollout status deploy/loft-web --timeout=180s
kubectl -n loft-test get pods,svc,ingress
```

`loft-postgres` bruger `Recreate` (PVC), og API'et kører med én replica fordi
presence/lofts-kanaler er in-memory — rolling updates ville droppe
WebSocket-forbindelser. TURN-pod'en er flyttet til platformen (se trin 2).

Migrationer kører automatisk i API'et ved start (`sqlx::migrate!`), så der er
ingen separat Job.

## 5. Cert: staging → verificér → prod

Manifesterne starter med `cert-manager.io/cluster-issuer: letsencrypt-staging`
for ikke at ramme Let's Encrypts produktions-rate-limits.

```bash
kubectl -n loft-test wait --for=condition=Ready \
  certificate/loft-test-gihc-online-tls --timeout=300s

# Staging-certet er ikke betroet af browseren → -k
curl -sk -o /dev/null -w '%{http_code}\n' https://loft.test.gihc.online/healthz

# Skift til prod-issueren (samme secret-navn → cert-manager re-issuer).
kubectl -n loft-test annotate ingress loft --overwrite \
  cert-manager.io/cluster-issuer=letsencrypt-prod
kubectl -n loft-test wait --for=condition=Ready \
  certificate/loft-test-gihc-online-tls --timeout=300s

# Verificér at kæden er betroet (ssl_verify_result 0)
curl -sS -o /dev/null -w '%{http_code} %{ssl_verify_result}\n' \
  https://loft.test.gihc.online/healthz
```

Hvis certifikatet ikke bliver re-issued efter annotationsskiftet:

```bash
kubectl -n loft-test delete certificate loft-test-gihc-online-tls   # genskabes af ingress-shim
```

## 6. Verifikation

```bash
# Gæste-loft-flowet (REST + WebSocket + luk med ejer-nøgle + DB-række væk)
bash scripts/smoke-test.sh https://loft.test.gihc.online --namespace loft-test

# Playwright mod det deployede miljø (rigtige MediaStreams, falske enheder)
cd e2e && npm run test:test

# Samme suite i Firefox (kræver `npx playwright install firefox` én gang)
cd e2e && npm run test:test:firefox

# TURN-relay (relay-only ICE, to browser-kontekster gennem platformens coturn)
cd e2e && TURN_URL="$(~/projects/infra/scripts/turn-config.sh --url)" \
  TURN_SECRET="$(pass turn/static-auth-secret)" \
  BASE_URL=https://loft.test.gihc.online npx playwright test tests/turn.spec.ts
```

`scripts/smoke-ws.py` forbinder to WebSocket-gæster og tjekker roster,
`join`/`leave`, `media-state` og at signaler til ukendte deltagere droppes.

TURN-testen er den eneste der beviser at relayeren virker (STUN-only-tests
forbinder direkte og rører aldrig coturn). Begge peers tvinges til
`iceTransportPolicy: 'relay'`, og testen fejler hvis kandidaterne ikke er af
typen `relay`, eller hvis data ikke kommer begge veje. Den skal køres fra en
maskine med DNS- og UDP-adgang — i CI springes den over.

`tests/turn.spec.ts` har desuden en test der bruger den **deployede sides egen**
`config.js` (via `loft.html`s `buildIceServers()`) og kræver en relay-kandidat.
De øvrige loft-tests stubber `config.js` med tom TURN, så uden den ville en
forkert `TURN_URL` i `configmap.yaml` først vise sig for en rigtig bruger.

## 7. Rollback

```bash
kubectl -n loft-test set image deploy/loft-api \
  loft-api=ghcr.io/gihc-org/loft:<forrige-sha>
kubectl -n loft-test rollout status deploy/loft-api
```

`Recreate`-strategien betyder et kort afbrudt forløb; frontendens
auto-reconnect dækker WebSocket-nedbruddet.

## 8. Nedtagning / oprydning

```bash
kubectl delete namespace loft-test      # sletter PVC, cert og ingress med
```

DNS-recorden og `pass`-nøglen skal ryddes manuelt (henholdsvis Simply.com og
`pass rm loft/postgres-password`). Det hører til M5-oprydningen sammen med de
gamle chat-records.

## Kendte faldgruber

- **`GET /healthz` ligger på API'et**, ikke på web-frontenden — ingressen har
  en eksplicit `/healthz`-rute (ellers svarer nginx-frontenden 404).
- **TURN_SECRET i ConfigMap'en** er reelt offentlig (HMAC-credentials) og er
  rendret fra platformens `pass turn/static-auth-secret` — hold den i sync med
  [k8s/test/configmap.yaml](../k8s/test/configmap.yaml) via
  `~/projects/infra/scripts/turn-config.sh --config-js`, hvis den roteres.
- **`frontend/config.js` overskrives** i k8s af ConfigMap'en — ændringer i
  repo-filen påvirker kun lokal udvikling.
- **`TEST_API_URL` uden `/v1`** i e2e; `BASE_URL` peger på frontenden.
- **DNS-cache efter ny A-record:** din lokale resolver kan have NXDOMAIN-cachet
  navnet (systemd-resolved gør det i SOA-minimum-TTL). Tøm med
  `sudo resolvectl flush-caches`, eller brug `--resolve` — smoke-testen
  understøtter `--resolve host:port:ip`, og `smoke-ws.py` har
  `--connect-ip`/`--sni` til samme formål.
- **Browserens ICE-stak bruger ikke `--host-resolver-rules`** til TURN/STUN:
  kan din maskine ikke slå værtsnavnet op, så kør relay-testen med TURN på IP
  (`turn:65.109.233.92:3478?transport=udp`).
- **Ingress-logs**: loft-id'et er adgangsnøglen, så del aldrig fulde URL'er
  med `id=`-parameteren i fejlrapporter.

- **Postgres-adgangskoden skal være URI-sikker.** Den bruges både som
  `POSTGRES_PASSWORD` og indlejret i
  `database-url=postgres://loft:<kode>@postgres:5432/loftdb`, så tegnene
  `/ ? # % @ :` og mellemrum skal percent-encodes — eller undgås helt.
  `openssl rand -base64 24` rammer `/` med ca. 1/3 sandsynlighed og brækker
  dermed URL'en, så brug `openssl rand -hex 24` (kun `0-9a-f`). Vil du have
  symboler med (fx fra `pwgen -y`), skal koden percent-encodes før den lægges
  i `database-url`. Postgres selv har ingen begrænsning på tegnene; grænsen
  ligger i URI'en, shell-quotingen og `kubectl --from-literal`.

## Prod-deploy (`loft.gihc.online`)

Samme kæde som test (trin 0–8), men i namespace `loft-prod` og med egne
secrets. Manifesterne ligger klar i [k8s/prod/](../k8s/prod/README.md) og
deployes **først efter** at testmiljøet er accepteret i hånden.

Status: **gennemført 2026-09-16**. Rækkefølgen nedenfor er den der blev kørt:
configmap → resten af `k8s/prod/` → rollouts → cert (staging → verificér →
`letsencrypt-prod`) → smoke-test → e2e. Resultaterne står i
[k8s/prod/README.md](../k8s/prod/README.md#status).

Forskelle fra test:

```bash
# 1. DNS — samme script, andet record-navn
bash scripts/create-dns-record.sh loft          # → loft.gihc.online

# 2. Eget postgres-password (må ikke deles med test)
PROD_PG_PASS="$(openssl rand -hex 24)"
printf '%s' "$PROD_PG_PASS" | pass insert -m loft/prod-postgres-password
kubectl apply -f k8s/prod/namespace.yaml
kubectl -n loft-prod create secret generic loft-secrets \
  --from-literal=postgres-password="$PROD_PG_PASS" \
  --from-literal="database-url=postgres://loft:${PROD_PG_PASS}@postgres:5432/loftdb" \
  --dry-run=client -o yaml | kubectl apply -f -

# 3. Apply (configmap'en har prod-realm og et rigtigt TURN-secret)
kubectl apply -f k8s/prod/configmap.yaml
kubectl apply -f k8s/prod/

# 4. Cert: staging → verificér → prod-issuer (som trin 5, men i loft-prod)
kubectl -n loft-prod annotate ingress loft --overwrite \
  cert-manager.io/cluster-issuer=letsencrypt-prod
```

**Images er pinnet på SHA-tag** i `k8s/prod/deployment-api.yaml` og
`deployment-web.yaml` (ikke `:latest`), så prod ikke skifter under fødderne og
rollback er `kubectl set image` til det forrige tag. Ved promote: erstat SHA'en
i begge filer med det tag CI har bygget på `trunk`, og `kubectl apply -f
k8s/prod/` (eller `kubectl -n loft-prod set image`).

Verifikation:

```bash
bash scripts/smoke-test.sh https://loft.gihc.online --namespace loft-prod
cd e2e && npm run test:beta      # sætter BASE_URL/TEST_API_URL til prod
# TURN-relay mod prod (samme test som test — TURN er platformens, ikke appens):
cd e2e && TURN_URL="$(~/projects/infra/scripts/turn-config.sh --url)" \
  TURN_SECRET="$(pass turn/static-auth-secret)" \
  BASE_URL=https://loft.gihc.online npx playwright test tests/turn.spec.ts
```

**TURN-secretet** er platformens (`pass turn/static-auth-secret`) og står kun i
`config.js` i `k8s/prod/configmap.yaml`. Roterer du det i `infra`, hentes de nye
linjer og web-frontenden genstartes — coturn kører i platformen, ikke her:

```bash
~/projects/infra/scripts/turn-config.sh --config-js
kubectl apply -f k8s/prod/configmap.yaml
kubectl -n loft-prod rollout restart deploy/loft-web
```

Rydder man testmiljøet senere, er det `kubectl delete namespace loft-test` plus
DNS-recorden og `pass rm loft/postgres-password` — se trin 8.
