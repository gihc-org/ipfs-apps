# Deploy til produktion

## Forudsætninger

- Ansible installeret lokalt
- SSH-nøgle til VPS: `~/.ssh/id_ed25519.hetzner`
- Vault-password (opbevares uden for repoet)
- `infra`-projektet er sat op på VPS'en (se nedenfor)

```bash
ansible-galaxy collection install -r ansible/requirements.yml  # kun første gang
```

## Afhængighed: platform-lag

ipfs-apps bruger **ikke** sin egen Caddy. En delt Caddy kører i `infra`-projektet
(`/opt/platform/` på VPS) og importerer `conf.d/*.caddy`. Dette deploy skriver
`/opt/platform/caddy/conf.d/chat.caddy` og genindlæser Caddy dér.

Sæt `infra` op én gang:
```bash
cd ~/projectes/infra
ansible-playbook ansible/infra.yml -i ansible/inventory.yml
```

## Kør deploy

**Normalt deploy:**
```bash
ansible-playbook ansible/deploy.yml -i ansible/inventory.yml --ask-vault-pass
```

**Første gang på ny server** (infrastruktur + deploy):
```bash
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml --ask-vault-pass
```

`deploy.yml` gør i rækkefølge:

1. Synkroniserer projektet til VPS (`/opt/chat/`) via rsync
2. Renderer `chat.caddy` fra template og skriver til `/opt/platform/caddy/conf.d/`
3. Genindlæser platform Caddy (`caddy reload` — ingen nedetid)
4. Renderer `.env` fra vault
5. Bygger og starter Docker Compose-services
6. Venter på at IPFS-daemonen er klar
7. Uploader `frontend/` til IPFS og henter CID
8. Opdaterer `_dnslink.chat.apps.gihc.online` hos Simply.com
9. Kører smoke test mod `https://api.gihc.online`

## Hvis deploy fejler

**Caddy genindlæser ikke:**
```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "docker logs platform-caddy-1 --tail 50"
```

Tjek at `conf.d/chat.caddy` er syntaktisk korrekt:
```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "docker compose -f /opt/platform/docker-compose.yml exec caddy caddy validate --config /etc/caddy/Caddyfile"
```

**Docker build fejler:**
```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "cd /opt/chat && docker compose -f docker-compose.yml -f docker-compose.prod.yml logs chat --tail 100"
```

**Smoke test fejler:**
Se [smoke-test.md](smoke-test.md).

## Manuel rollback

```bash
git log --oneline -5
git checkout <commit>
ansible-playbook ansible/deploy.yml -i ansible/inventory.yml --ask-vault-pass
git checkout trunk
```
