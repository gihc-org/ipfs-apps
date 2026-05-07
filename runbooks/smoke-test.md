# Smoke test

## Hvad tester den

`scripts/smoke-test.sh` kører automatisk sidst i Ansible-playbook'en og
verificerer fem ting mod `https://api.gihc.online`:

| Trin | Endpoint | Forventet |
|------|----------|-----------|
| 1 | `POST /auth/token` | 200, JWT returneret |
| 2 | `GET /auth/me` | 200, GDPR-felter til stede (id, username, email, email_verified, created_at) |
| 3 | `GET /rooms` | 200 |
| 4 | `DELETE /auth/me` | 204 |
| 5 | `GET /auth/me` efter sletning | 401 |

Testen opretter en midlertidig bruger direkte i databasen (omgår CAPTCHA),
kører sine checks og sletter brugeren igen via `DELETE /auth/me`. `trap`
sikrer oprydning selv ved fejl.

## Kør manuelt

```bash
PG_PASS=$(grep POSTGRES_PASSWORD /home/kristian/projects/ipfs-apps/.env | cut -d= -f2)

ssh -i ~/.ssh/id_ed25519.hetzner root@65.109.233.92 \
  "bash /opt/chat/scripts/smoke-test.sh \
    https://api.gihc.online chatuser '$PG_PASS' chatdb /opt/chat"
```

## Hvis et trin fejler

**Trin 1 fejler (401 ved login):**
- API'et er oppe men brugeren kunne ikke logge ind
- Tjek at argon2id-hashet i scriptet stadig matcher applikationens parametre (se nedenfor)
- Tjek `docker logs chat-chat-1 --tail 50` for fejl

**Trin 1 fejler (502/503):**
- Backend er ikke oppe — tjek `docker ps` på serveren
- Kør `docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d` manuelt

**Trin 2 fejler (GDPR-felter mangler):**
- `GET /auth/me` returnerer ikke alle felter — sandsynligvis en migration der ikke er kørt
- Tjek `docker logs chat-chat-1` for migration-fejl ved startup

**Trin 5 fejler (stadig 200 efter sletning):**
- `DELETE /auth/me` returnerede 204 men brugeren er ikke slettet
- Sandsynligvis et databaseproblem — tjek PostgreSQL-logs

## Regenerer argon2id-hash

Smoke-testen bruger et forudberegnet hash af adgangskoden `SmokeTest99!`
med `Argon2::default()` (m=19456, t=2, p=1). Hvis parametrene i
`chat/src/auth.rs` ændres, skal hashet regenereres:

```bash
cd /home/kristian/projects/ipfs-apps/chat
mkdir -p examples
cat > examples/gen_hash.rs << 'EOF'
fn main() {
    println!("{}", chat::auth::hash_password("SmokeTest99!"));
}
EOF
cargo run --example gen_hash
rm examples/gen_hash.rs && rmdir examples
```

Opdater `TEST_HASH` i `scripts/smoke-test.sh` med det nye hash.
