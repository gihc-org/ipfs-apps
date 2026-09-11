# k8s/prod — Loft produktionsmiljø

Spejling af [k8s/test/](../test/README.md) for `loft.gihc.online` i namespace
`loft-prod`. **Ikke deployet endnu** — manifesterne er forberedt og klar, og
tages i brug når testmiljøet har været accepteret i hånden. Trin-for-trin står i
[runbooks/loft-deploy.md](../../runbooks/loft-deploy.md) under "Prod-deploy".

Forskelle fra test:

| | test | prod |
|---|---|---|
| Host | `loft.test.gihc.online` | `loft.gihc.online` |
| Namespace | `loft-test` | `loft-prod` |
| TLS-secret | `loft-test-gihc-online-tls` | `loft-gihc-online-tls` |
| coturn realm | `loft.test.gihc.online` | `loft.gihc.online` |
| TURN-secret | test-placeholder | eget secret (sat i `configmap.yaml`) |
| Image-tag | `:latest` (Always) | pinnet SHA-tag (IfNotPresent) |

## Før deploy

1. Opret DNS-recorden: `bash scripts/create-dns-record.sh loft` (idempotent).
2. Opret `loft-secrets` i `loft-prod` med en **egen** postgres-adgangskode
   (`pass insert loft/prod-postgres-password`) — prod må ikke dele
   database-credentials med test.
3. Tjek at image-SHA'en i `deployment-api.yaml` og `deployment-web.yaml` er det
   tag CI har bygget på `trunk` (`curl -s
   https://ghcr.io/v2/gihc-org/loft/tags/list` med et anonymt token).
4. `configmap.yaml` har allerede realm `loft.gihc.online` og et rigtigt
   TURN-secret; rotér det kun hvis det er kompromitteret (husk begge felter i
   filen).

## Apply

Samme rækkefølge som i [runbooks/loft-deploy.md](../../runbooks/loft-deploy.md):
namespace → secret → configmap → resten, og derefter cert
(staging → verificér → prod-issuer) og smoke-test:

```bash
bash scripts/smoke-test.sh https://loft.gihc.online --namespace loft-prod
```
