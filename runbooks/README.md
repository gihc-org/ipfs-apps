# Runbooks

Driftsopgaver og fejlfindingsprocedurer for ipfs-apps/chat.

Modsat ADR'er (som dokumenterer *hvorfor*) dokumenterer runbooks *hvordan*.

> **Status (2026-09-09):** runbooks beskriver chat-tiden
> (Caddy/Ansible/IPFS/DM) og er forældede indtil M5-oprydningen. Loft deployer
> via k3s — se [MIGRATION.md](../MIGRATION.md) og `k8s/test/`.

> **Infrastruktur-afhængighed:** ipfs-apps bruger en delt Caddy fra `infra`-projektet
> (`~/projectes/infra`). Caddy ejes ikke af dette projekt — se `infra/README.md`
> for opsætning og drift af platform-laget.

| Runbook | Indhold |
|---------|---------|
| [deploy.md](deploy.md) | Deploy til produktion via Ansible, rollback |
| [smoke-test.md](smoke-test.md) | Hvad smoke testen dækker, fejlfinding, regenerer argon2id-hash |
| [ipfs-dns.md](ipfs-dns.md) | IPFS-upload, DNSLink-opdatering, Simply.com API |
| [webrtc-debugging.md](webrtc-debugging.md) | WebRTC skærmdeling — forbindelsesproblemer, STUN/TURN, lokal test |
