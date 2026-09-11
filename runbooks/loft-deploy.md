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
export KUBECONFIG=~/projects/infra/kubeconfig.yml
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

## 2. Firewall (TURN)

Reglerne ligger allerede i `~/projects/infra/tofu/main.tf`: **TCP+UDP 3478** og
**UDP 49152–49200**. De er applied på `platform-firewall` (åbne mod
`0.0.0.0/0` siden 2026-07-04), så TURN er offentligt tilgængeligt i samme
øjeblik coturn-pod'en kører — der er ingen ekstra "åbn"-kommando. Bekræft
reglerne med:

```bash
cd ~/projects/infra/tofu && direnv allow && tofu plan   # forventet: no changes
```

Uden de porte virker mesh-lyd/video stadig på samme netværk (STUN), men fejler
bag NAT — det er først verificerbart med e2e fra to forskellige netværk.

Misbrugsbeskyttelsen sidder i coturns args: `--denied-peer-ip` for
RFC1918/loopback/link-local/CGNAT og kvoter (`--max-bps`, `--bps-capacity`,
`--user-quota`, `--total-quota`). Baggrunden er at `TURN_SECRET` er offentlig
(den ligger i `config.js`), så alle kan udstede credentials. Kvoterne skal
tunes efter mesh-størrelsen: hver peer-forbindelse bruger én allocation, så
`--user-quota=8` rækker til ca. 8 deltagere — hæv den (og `--total-quota`) hvis
loftene vokser.

## 3. Secret `loft-secrets`

```bash
export KUBECONFIG=~/projects/infra/kubeconfig.yml
kubectl apply -f k8s/test/namespace.yaml

# Engang: gem postgres-adgangskoden i pass (samme mønster som capture).
PG_PASS="$(openssl rand -base64 24)"
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

`loft-postgres` og `loft-coturn` bruger `Recreate` (PVC henholdsvis hostNetwork),
og API'et kører med én replica fordi presence/lofts-kanaler er in-memory —
rolling updates ville droppe WebSocket-forbindelser.

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

# TURN-relay (relay-only ICE, to browser-kontekster gennem coturn)
cd e2e && TURN_URL='turn:loft.test.gihc.online:3478?transport=udp' \
  TURN_SECRET="$(kubectl -n loft-test get cm loft-config -o jsonpath='{.data.turn-secret}')" \
  BASE_URL=https://loft.test.gihc.online npx playwright test tests/turn.spec.ts
```

`scripts/smoke-ws.py` forbinder to WebSocket-gæster og tjekker roster,
`join`/`leave`, `media-state` og at signaler til ukendte deltagere droppes.

TURN-testen er den eneste der beviser at relayeren virker (STUN-only-tests
forbinder direkte og rører aldrig coturn). Begge peers tvinges til
`iceTransportPolicy: 'relay'`, og testen fejler hvis kandidaterne ikke er af
typen `relay`, eller hvis data ikke kommer begge veje. Den skal køres fra en
maskine med DNS- og UDP-adgang — i CI springes den over.

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
- **TURN_SECRET i ConfigMap'en** er reelt offentlig (HMAC-credentials med
  24 t TTL) og skal holdes i sync mellem `config.js` og `turn-secret` i
  [k8s/test/configmap.yaml](../k8s/test/configmap.yaml).
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
