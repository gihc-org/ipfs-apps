# k8s/test — Loft test-miljø

Manifester til `loft.test.gihc.online` (namespace `loft-test`). Deployeres
manuelt efter [runbooks/loft-deploy.md](../../runbooks/loft-deploy.md) —
trinnene uden for repoet (DNS, secret, SSH-tunnel) kræver brugerens adgang.

```
namespace.yaml          loft-test
configmap.yaml          loft-config: config.js pr. miljø (TURN peger på platformen)
deployment-postgres.yaml + pvc + service
deployment-api.yaml     1 replica (in-memory presence), read-only rootfs, uid 10001
deployment-web.yaml     nginx + config.js mountet fra ConfigMap
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
- **TURN er platformens:** coturn kører ikke længere i dette namespace. Der kan
  kun være én coturn på noden (to instanser deler 3478 via `SO_REUSEPORT`), så
  den ligger i `infra` som namespace `coturn` med `turn.gihc.online` som navn —
  se `docs/adr/0003-delt-turn-platform.md` og `k8s/coturn/README.md` der.
  `config.js` her peger på den, og `TURN_SECRET` rendres fra platformens
  `pass turn/static-auth-secret` (`~/projects/infra/scripts/turn-config.sh
  --config-js`) — hold de to kopier i sync ved rotation.
- Hærdningen, kvoterne (`--user-quota=8` pr. credential ≈ 8 deltagere i et mesh,
  `--total-quota` = 40 i alt) og `--denied-peer-ip`-filtrene er flyttet med til
  platformen og tunes der, ikke her.
- GHCR-pakkerne `ghcr.io/gihc-org/loft` og `loft-web` blev public automatisk
  (pakker pushet af Actions med `GITHUB_TOKEN` arver repoets synlighed).
- Firewall-portene til TURN (TCP+UDP 3478, UDP 49152–49200) ejes af platformen
  og er allerede åbne i `platform-firewall`.
- Skift issuer til `letsencrypt-prod`, når certifikatet er verificeret.
- Sikkerheds-headers: globale basis-headers sættes i `infra/tofu/platform.tf`;
  CSP/Permissions-Policy sættes i app-laget (nginx-conf/Axum), ikke som
  ingress-snippet.
