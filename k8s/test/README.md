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
deployment-coturn.yaml  hostNetwork — TURN 3478 + UDP 49152–49200, hærdet
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
- **coturn kører hærdet:** ikke-root (`runAsUser: 65534`), `readOnlyRootFilesystem`,
  `allowPrivilegeEscalation: false` (no_new_privs) og
  `capabilities: {drop: ["ALL"], add: ["NET_BIND_SERVICE"]}`.
  `NET_BIND_SERVICE` skal stå i bounding-settet for at `execve` af turnserver
  lykkes (binæren bærer file-capabilities); med no_new_privs bliver den ikke
  givet videre til processen. Se `deployment-coturn.yaml` for detaljer —
  `drop: ["ALL"]` uden `add` giver en crash-loop.
- **Sabotagebeskyttelse:** `TURN_SECRET` ligger i frontendens `config.js` og er
  dermed offentlig — alle kan udstede HMAC-credentials (24 t TTL). Derfor:
  `--denied-peer-ip` for RFC1918/loopback/link-local/CGNAT (så TURN ikke kan
  bruges som springbræt ind i nodens netværk) og kvoter
  (`--max-bps`, `--bps-capacity`, `--user-quota`, `--total-quota`,
  `--stale-nonce`). Langsigtet løsning: udsted credentials i API'et med kort
  TTL, så hemmeligheden ikke ligger i frontend-koden.
- **Kvoterne skal tunes efter mesh-størrelsen:** hver peer-forbindelse i
  meshen bruger én allocation, så `--user-quota=8` (pr. credential) rækker til
  ca. 8 deltagere. Vokser loftene, er `--user-quota` og `--total-quota` de
  første tal der skal hæves; relay-portintervallet (49152–49200 = 49 porte) er
  den næste grænse.
- `-n` er udeladt i coturns args: imagets entrypoint kører `eval "echo $i"` pr.
  argument, og `echo -n` bliver til en tom streng, så coturn logger
  "Unknown argument:" ved hver start. Imaget har ingen `turnserver.conf`.
- GHCR-pakkerne `ghcr.io/gihc-org/loft` og `loft-web` blev public automatisk
  (pakker pushet af Actions med `GITHUB_TOKEN` arver repoets synlighed).
- coturn kræver åbne firewall-porte (TCP+UDP 3478, UDP 49152–49200) i
  `infra/tofu/main.tf` før WebRTC virker på tværs af NAT. Reglerne er lagt ind
  (og var åbne fra 2026-07-04) — verificér med `tofu plan` i
  `~/projects/infra/tofu`.
- Skift issuer til `letsencrypt-prod`, når certifikatet er verificeret.
- Sikkerheds-headers: globale basis-headers sættes i `infra/tofu/platform.tf`;
  CSP/Permissions-Policy sættes i app-laget (nginx-conf/Axum), ikke som
  ingress-snippet.
