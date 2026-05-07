# Deploy til produktion

## Forudsætninger

- Ansible installeret lokalt
- SSH-nøgle til VPS: `~/.ssh/id_ed25519.hetzner`
- Vault-password (opbevares uden for repoet)

```bash
ansible-galaxy collection install -r ansible/requirements.yml  # kun første gang
```

## Kør deploy

**Ved release** (bruges normalt):
```bash
cd /home/kristian/projects/ipfs-apps
ansible-playbook ansible/deploy.yml -i ansible/inventory.yml --ask-vault-pass
```

**Første gang på en ny server** (infrastruktur + deploy):
```bash
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml --ask-vault-pass
```

`deploy.yml` gør i rækkefølge:

1. Synkroniserer projektet til VPS (`/opt/chat/`) via rsync
2. Renderer `Caddyfile` og `.env` fra Ansible-templates + vault
3. Genstarter Caddy så ny config indlæses
4. Bygger og starter Docker Compose-services (`docker-compose.yml` + `docker-compose.prod.yml`)
5. Venter på at IPFS-daemonen er klar
6. Uploader `frontend/` til IPFS og henter CID
7. Opdaterer `_dnslink.chat.apps.gihc.online` hos Simply.com via REST API
8. Kører smoke test mod `https://api.gihc.online` (se [smoke-test.md](smoke-test.md))

## Hvis deploy fejler

**Trin 3 — Caddy genstarter ikke:**
```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "docker logs chat-caddy-1 --tail 50"
```

**Trin 4 — Docker build fejler:**
```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "cd /opt/chat && docker compose -f docker-compose.yml -f docker-compose.prod.yml logs chat --tail 100"
```

**Trin 8 — Smoke test fejler:**
Se [smoke-test.md](smoke-test.md).

## Manuel rollback

Der er ingen automatisk rollback. Forrige fungerende version hentes fra git:

```bash
# Find forrige commit
git log --oneline -5

# Skub forrige version til VPS
git checkout <commit>
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml --ask-vault-pass
git checkout trunk
```
