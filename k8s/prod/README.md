# k8s/prod — Loft produktionsmiljø

Spejling af [k8s/test/](../test/README.md) for `loft.gihc.online` i namespace
`loft-prod`. **Ikke deployet endnu** — filerne er forberedt i M4 og tages i brug
i M5, når testmiljøet har været grønt i en periode.

Forskelle fra test:

| | test | prod |
|---|---|---|
| Host | `loft.test.gihc.online` | `loft.gihc.online` |
| Namespace | `loft-test` | `loft-prod` |
| TLS-secret | `loft-test-gihc-online-tls` | `loft-gihc-online-tls` |
| coturn realm | `loft.test.gihc.online` | `loft.gihc.online` |
| TURN-secret | test-placeholder | **skal sættes** (`change-me-prod-turn-secret`) |

## Før deploy

1. Erstat TURN-placeholderne i `configmap.yaml` (`config.js` **og** `turn-secret`
   skal være identiske) med et rigtigt secret fra `pass`, fx
   `pass insert loft/prod-turn-secret`.
2. Pint image-tags: erstat `:latest` i `deployment-api.yaml` og
   `deployment-web.yaml` med det SHA-tag der er verificeret i test.
3. Opret DNS-recorden for `loft.gihc.online` (samme mønster som
   `scripts/create-dns-record.sh`, men med `RECORD_NAME="loft"`).
4. Opret `loft-secrets` i `loft-prod` med en **egen** postgres-adgangskode —
   prod må ikke dele database-credentials med test.

## Apply

Samme rækkefølge som i [runbooks/loft-deploy.md](../../runbooks/loft-deploy.md):
namespace → secret → configmap → resten, og derefter cert
(staging → verificér → prod-issuer) og smoke-test:

```bash
bash scripts/smoke-test.sh https://loft.gihc.online --namespace loft-prod
```
