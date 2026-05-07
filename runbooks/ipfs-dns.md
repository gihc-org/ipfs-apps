# IPFS og DNS fejlfinding

## Arkitektur

```
Browser → chat.apps.gihc.online
            │
            └── _dnslink.chat.apps.gihc.online  TXT "dnslink=/ipfs/<CID>"
                    │
                    └── IPFS-gateway henter indholdet fra CID
```

Frontend serveres fra IPFS. DNS-linket opdateres automatisk af Ansible
efter hvert deploy via Simply.com REST API.

## Frontend viser gammel version

DNSLink-propagering kan tage op til TTL (120 sekunder). Vent lidt og
prøv igen. Tjek om DNS er opdateret:

```bash
dig TXT _dnslink.chat.apps.gihc.online +short
# Forventet: "dnslink=/ipfs/<nyeste-CID>"
```

Sammenlign med CID fra seneste deploy (vises i Ansible-output under
"Show IPFS CID").

## Manuel DNS-opdatering

Hvis Ansible-deployet fejlede på DNS-trinnet, opdateres DNS manuelt:

```bash
# Find den nye CID
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "cd /opt/chat && docker compose exec -T ipfs ipfs add -r /frontend/ -Q"

# Opdater via Ansible (kører kun DNS-trinnet)
ansible-playbook ansible/playbook.yml -i ansible/inventory.yml \
  --ask-vault-pass --tags dns
```

> Ansible-playbook'en har endnu ikke tags — ovenstående kræver at DNS-tasken
> tagges med `dns` i `playbook.yml`.

## Manuel IPFS-upload

```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "cd /opt/chat && docker compose exec -T ipfs ipfs add -r /frontend/ -Q"
```

## IPFS-daemon er ikke klar

Ansible venter automatisk op til 50 sekunder (10 forsøg × 5 s) på at
IPFS-daemonen svarer. Hvis den stadig ikke er klar:

```bash
ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "docker logs chat-ipfs-1 --tail 50"
```

## Simply.com API-fejl

Credentials hentes fra vault (`vault_simply_account`, `vault_simply_api_key`).
Test API-adgang manuelt:

```bash
curl -u "<kontonummer>:<api-nøgle>" \
  "https://api.simply.com/2/my/products/gihc.online/dns/records/"
```
