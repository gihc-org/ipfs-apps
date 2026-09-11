# Runbooks

Driftsopgaver og fejlfindingsprocedurer for ipfs-apps (nu Loft).

Modsat ADR'er (som dokumenterer *hvorfor*) dokumenterer runbooks *hvordan*.

> **Status (2026-09-11):** Loft deployer via k3s. `deploy.md` og `ipfs-dns.md`
> beskriver chat-tiden (Ansible/Caddy/IPFS) og er historiske indtil
> M5-oprydningen; brug [loft-deploy.md](loft-deploy.md) i stedet.

> **Infrastruktur-afhængighed:** platformen (ingress-nginx, cert-manager,
> local-path, firewall) ejes af `~/projects/infra`, ikke af dette projekt — se
> `infra/README.md` for opsætning og drift af platform-laget.

| Runbook | Indhold |
|---------|---------|
| [loft-deploy.md](loft-deploy.md) | Deploy af Loft-testmiljøet til k3s (DNS, secret, apply, cert, rollback) |
| [smoke-test.md](smoke-test.md) | Gæste-loft-flowet ende-til-ende, fejlfinding |
| [deploy.md](deploy.md) | *(historisk)* Ansible-deploy af chat-stakken |
| [ipfs-dns.md](ipfs-dns.md) | *(historisk)* IPFS-upload, DNSLink, Simply.com API |
| [webrtc-debugging.md](webrtc-debugging.md) | WebRTC — forbindelsesproblemer, STUN/TURN, lokal test |
