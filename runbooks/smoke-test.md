# Smoke test — Loft

## Hvad tester den

`scripts/smoke-test.sh` kører gæste-loft-flowet ende-til-ende mod ét origin
(frontend + API + WebSocket deler host). Den bruges efter deploy (se
[loft-deploy.md](loft-deploy.md)) og kan køres manuelt mod ethvert miljø.

```bash
./scripts/smoke-test.sh https://loft.test.gihc.online --namespace loft-test
```

Er A-recorden lige oprettet og din resolver har NXDOMAIN-cachet navnet, kan
IP'en pinnes som med `curl --resolve`:

```bash
./scripts/smoke-test.sh https://loft.test.gihc.online --namespace loft-test \
  --resolve loft.test.gihc.online:443:65.109.233.92
```

| Trin | Kald | Forventet |
|------|------|-----------|
| 1 | `GET /healthz` | 200 `ok` |
| 2 | `POST /v1/lofts` | 201 med `id`, `url` og `owner_token` |
| 3 | `GET /v1/lofts/:id` | 200, navn matcher, `owner_token` lækkes ikke |
| 4 | `GET /v1/lofts/:ukendt-id` | 404 |
| 5 | WS `/v1/ws/:loft_id` | to gæster: roster, join-broadcast, media-state og signal med server-stemplet `from`, signal til ukendt modtager droppes, leave annonceres |
| 6 | `DELETE /v1/lofts/:id` uden/forkert ejer-nøgle | 403 |
| 7 | `DELETE /v1/lofts/:id` med ejer-nøgle | 204 |
| 8 | `GET` + `DELETE` efter lukning | 404 |
| 9 | `kubectl exec` i `loft-postgres` → `psql` | loftet findes ikke i `lofts`-tabellen |

Trin 5 kører [scripts/smoke-ws.py](../scripts/smoke-ws.py) (websockets-biblioteket)
og er det eneste sted ud over Playwright hvor WebSocket-protokollen afprøves.
Trin 9 kræver `--namespace` og dermed kubeconfig + SSH-tunnel; uden flaget
springes databasen over.

Testen opretter ét loft og lukker det igen med ejer-nøglen. `trap cleanup`
sletter loftet, hvis et tidligere trin fejler.

## Kør lokalt mod `cargo run`

```bash
cd chat && DATABASE_URL=postgres://postgres:postgres@localhost:5432 cargo run   # 0.0.0.0:8080
./scripts/smoke-test.sh http://localhost:8080
```

Bemærk: `POST /v1/lofts` er rate-limited (1/s, burst 10 pr. IP) — kør ikke
scriptet i stram løkke bag samme IP.

## Staging-certifikat

Er miljøet stadig på `letsencrypt-staging`, validerer Python-klienten ikke
certet. Brug da:

```bash
./scripts/smoke-test.sh https://loft.test.gihc.online --insecure
```

## Hvis et trin fejler

**Trin 1 (healthz) fejler med 404:** ingressen mangler `/healthz`-ruten — den
ligger på API'et, ikke på web-frontenden (se `k8s/test/ingress.yaml`).

**Trin 1 fejler med 502/503:** API-pod'en er ikke klar. Tjek
`kubectl -n loft-test get pods` og `kubectl -n loft-test logs deploy/loft-api`.
Ofte `loft-secrets` (forkert `database-url`) eller postgres der ikke er klar.

**Trin 2 fejler med 500:** API'et kan ikke skrive til databasen — tjek
migrationsfejl i API-loggen ved startup.

**Trin 5 fejler med "ingen besked inden for 10s":** WebSocket-håndtrykket lykkes
ikke. Tjek at loftet findes (trin 3), at ingressens `proxy-read-timeout` er
sat, og at `websockets`-versionen lokalt ikke er ændret.

**Trin 5 fejler med "uventet signal-besked":** serveren videresender signaler
til ukendte modtagere — det er en regressionsfejl i
`chat/src/routes/lofts.rs`, ikke et miljøproblem.

**Trin 9 fejler:** `kubectl exec` kunne ikke køre `psql` (forkert
namespace/pod-navn) eller loftet blev ikke slettet. Kør
`kubectl -n loft-test exec deploy/loft-postgres -- psql -U loft -d loftdb -c '\dt'`.

## Relateret

- Playwright-e2e mod det deployede miljø: `cd e2e && npm run test:test`
- TURN-relay (den del STUN-only-tests ikke dækker): `e2e/tests/turn.spec.ts`,
  se verifikationsafsnittet i [loft-deploy.md](loft-deploy.md)
- WebRTC-forbindelsesproblemer: [webrtc-debugging.md](webrtc-debugging.md)
  (skrevet i chat-tiden — STUN/TURN-afsnittet er stadig gyldigt)
