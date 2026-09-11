# k8s/test — Loft test-miljø

Manifester til `loft.test.gihc.online` (namespace `loft-test`). Deployeres
manuelt efter [runbooks/loft-deploy.md](../../runbooks/loft-deploy.md) —
trinnene uden for repoet (DNS, secret, SSH-tunnel) kræver brugerens adgang.

```
namespace.yaml          loft-test
configmap.yaml          loft-config: config.js pr. miljø + coturn-realm/secret
deployment-postgres.yaml + pvc + service
deployment-api.yaml     1 replica (in-memory presence), read-only rootfs, uid 10001
deployment-web.yaml     nginx + config.js mountet fra ConfigMap
deployment-coturn.yaml  hostNetwork — TURN 3478 + UDP 49152–49200
ingress.yaml            /healthz + /v1 → api, / → web, WS-timeouts 3600
```

## Opret secret

```bash
kubectl create secret generic loft-secrets -n loft-test \
  --from-literal=postgres-password="$(pass loft/postgres-password)" \
  --from-literal=database-url="postgres://loft:$(pass loft/postgres-password)@postgres:5432/loftdb"
```

Juster `pass`-stien til den konvention der bruges i infra-projektet.

## Apply

```bash
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f .
```

## Noter

- `/healthz` ligger i API'et (ikke i frontenden) og har sin egen ingress-rute,
  så smoke-test og uptime-checks kan nå den udefra.
- `TURN_SECRET` i configmap.yaml er et test-placeholder og reelt offentlig
  (HMAC-credentials med 24 t TTL). Hold `config.js` og `turn-secret` i sync.
- GHCR-pakkerne `ghcr.io/gihc-org/loft` og `loft-web` skal gøres public første
  gang — ellers kræves `imagePullSecrets`.
- coturn kræver åbne firewall-porte (TCP+UDP 3478, UDP 49152–49200) i
  `infra/tofu/main.tf` før WebRTC virker på tværs af NAT. Reglerne er lagt ind
  — verificér med `tofu plan` i `~/projects/infra/tofu`.
- Skift issuer til `letsencrypt-prod`, når certifikatet er verificeret.
- Sikkerheds-headers: globale basis-headers sættes i `infra/tofu/platform.tf`;
  CSP/Permissions-Policy sættes i app-laget (nginx-conf/Axum), ikke som
  ingress-snippet.
