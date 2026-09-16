# k8s/prod — Loft produktionsmiljø

Spejling af [k8s/test/](../test/README.md) for `loft.gihc.online` i namespace
`loft-prod`. **Deployet 2026-09-16** og pinnet til CI-SHA'en `d082905…`; cert
fra `letsencrypt-prod`, smoke-test 19/19 og e2e grøn i Chromium og Firefox
(inkl. TURN-relay). Trin-for-trin står i
[runbooks/loft-deploy.md](../../runbooks/loft-deploy.md) under "Prod-deploy".

Forskelle fra test:

| | test | prod |
|---|---|---|
| Host | `loft.test.gihc.online` | `loft.gihc.online` |
| Namespace | `loft-test` | `loft-prod` |
| TLS-secret | `loft-test-gihc-online-tls` | `loft-gihc-online-tls` |
| Image-tag | `:latest` (Always) | pinnet SHA-tag (IfNotPresent) |

TURN er fælles for begge miljøer: begge `config.js` peger på
`turn:turn.gihc.online:3478?transport=udp` med platformens secret
(`pass turn/static-auth-secret`, ADR 0003 i `~/projects/infra`). Der er ingen
egen coturn i prod-manifesterne — én instans pr. node er hele pointen.

## Før deploy

1. DNS-recorden — udført 2026-09-16: `bash scripts/create-dns-record.sh loft`
   (idempotent; `A loft.gihc.online` → 65.109.233.92).
2. `loft-secrets` i `loft-prod` med en **egen** postgres-adgangskode — udført
   2026-09-16 (`pass insert loft/prod-postgres-password`; prod må ikke dele
   database-credentials med test).
3. Tjek at image-SHA'en i `deployment-api.yaml` og `deployment-web.yaml` er det
   tag CI har bygget på `trunk` (`curl -s
   https://ghcr.io/v2/gihc-org/loft/tags/list` med et anonymt token).
4. `configmap.yaml` peger allerede på platformens TURN. Hvis platformens
   secret er roteret, skal `TURN_SECRET` opdateres her:
   `~/projects/infra/scripts/turn-config.sh --config-js`. (Verificeret i sync
   2026-09-16.)

## Status

| Trin | Resultat 2026-09-16 |
|---|---|
| Rollouts | `loft-postgres`, `loft-api` og `loft-web` "successfully rolled out" |
| Cert | `loft-gihc-online-tls`, issuer `letsencrypt-prod`, gyldigt til 2026-12-15 |
| HTTPS | `/healthz` → 200 med `ssl_verify_result` 0 |
| Smoke-test | `scripts/smoke-test.sh https://loft.gihc.online --namespace loft-prod` → 19/19 |
| e2e | Chromium 9 passed / 2 skipped; Firefox 11/11; TURN-relay 2/2 (`relay/relay`) |

Prod-pinnen blev efter dokumentationspushet flyttet fra `2816181…` til
`d082905…` (samme kildekode) med `kubectl apply` — så det kørende image matcher
`trunk`. Rollback er `kubectl -n loft-prod set image`.

## Apply

Samme rækkefølge som i [runbooks/loft-deploy.md](../../runbooks/loft-deploy.md):
namespace → secret → configmap → resten, og derefter cert
(staging → verificér → prod-issuer) og smoke-test:

```bash
bash scripts/smoke-test.sh https://loft.gihc.online --namespace loft-prod
```
