# Runbooks

Driftsopgaver og fejlfindingsprocedurer for ipfs-apps (nu Loft).

Modsat ADR'er (som dokumenterer *hvorfor*) dokumenterer runbooks *hvordan*.

> **Status (2026-09-16):** Loft deployer via k3s, og M5-oprydningen er
> gennemført: `deploy.md` (Ansible) og `ipfs-dns.md` (IPFS/DNSLink) er slettet
> sammen med selve stakken. Historikken står i
> [MIGRATION.md](../MIGRATION.md). Brug [loft-deploy.md](loft-deploy.md)
> til deploy.

> **Infrastruktur-afhængighed:** platformen (ingress-nginx, cert-manager,
> local-path, firewall) ejes af `~/projects/infra`, ikke af dette projekt — se
> `infra/README.md` for opsætning og drift af platform-laget.

| Runbook | Indhold |
|---------|---------|
| [loft-deploy.md](loft-deploy.md) | Deploy af Loft til k3s — test og prod (DNS, secret, apply, cert, rollback) |
| [smoke-test.md](smoke-test.md) | Gæste-loft-flowet ende-til-ende, fejlfinding |
| [webrtc-debugging.md](webrtc-debugging.md) | WebRTC — forbindelsesproblemer, STUN/TURN, lokal test |
