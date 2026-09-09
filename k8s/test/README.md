# k8s/test — Loft test-miljø

Skelet til `loft.test.gihc.online`. Manifesterne forudsætter M1-backend-koden
(non-root image, `/healthz`) og frontend-Dockerfilet i denne gren; de er ikke
deployet endnu.

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

- `TURN_SECRET` i configmap.yaml er et test-placeholder og reelt offentlig
  (HMAC-credentials med 24 t TTL). Hold `config.js` og `turn-secret` i sync.
- GHCR-pakkerne `ghcr.io/gihc-org/loft` og `loft-web` skal gøres public første
  gang — ellers kræves `imagePullSecrets`.
- coturn kræver åbne firewall-porte (TCP+UDP 3478, UDP 49152–49200) i
  `infra/tofu/main.tf` før WebRTC virker på tværs af NAT.
- Skift issuer til `letsencrypt-prod`, når certifikatet er verificeret.
- Sikkerheds-headers: globale basis-headers sættes i `infra/tofu/platform.tf`;
  CSP/Permissions-Policy sættes i app-laget (nginx-conf/Axum), ikke som
  ingress-snippet.
