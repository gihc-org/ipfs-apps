# TODO

## Sikkerhed — OWASP-scanning

Se `OWASP-IMPROVEMENTS.md` for kendte fund og anbefalinger til kodeændringer.
Nedenfor er værktøjer til løbende validering.

### 1. HTTP security headers — verificer efter næste deploy

Kør mod live URL efter deploy for at bekræfte at Caddyfile-headerne virker:

- https://securityheaders.com/?q=https://api.gihc.online
- https://observatory.mozilla.org/analyze/api.gihc.online

Forventet resultat: mindst A baseret på de headers vi tilføjede i commit c09b52d.

### 2. OWASP ZAP — dynamisk scanning mod kørende app

Passiv baseline-scan (ingen destruktive tests, velegnet til CI):

```bash
docker run -t owasp/zap2docker-stable zap-baseline.py \
  -t https://api.gihc.online
```

Fuld scan (mere aggressiv — kør kun mod testmiljø):

```bash
docker run -t owasp/zap2docker-stable zap-full-scan.py \
  -t https://api.gihc.online
```

Overvej at tilføje baseline-scan som task i `ansible/playbook.yml` efter deploy.

### 3. cargo audit — dependency-scanning

```bash
cd chat && cargo audit
```

Tilføj som task i `ansible/playbook.yml` inden `docker compose build` — se
anbefaling i `OWASP-IMPROVEMENTS.md`.

---

## Sikkerhed — tilbageværende kodeændringer

Se `OWASP-IMPROVEMENTS.md` for detaljer og kodeeksempler.

- [ ] Rate limiting på `/auth/token` og `/auth/register` (`tower_governor`)
- [ ] JWT-token revokering ved logout
- [ ] Verifikationstoken udløber efter 48 timer
- [ ] Account lockout efter gentagne fejlede loginforsøg
- [ ] Sikkerhedslogning med `tracing::warn!` på auth-hændelser
- [ ] JWT i `httpOnly`-cookie frem for `localStorage`
- [ ] `email`-felt NOT NULL i database
