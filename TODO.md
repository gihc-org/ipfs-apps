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

- [ ] Rate limiting på `/auth/token`, `/auth/register`, og `/dms` POST (`tower_governor`)
- [ ] JWT-token revokering ved logout
- [ ] Verifikationstoken udløber efter 48 timer
- [ ] Account lockout efter gentagne fejlede loginforsøg
- [ ] Sikkerhedslogning med `tracing::warn!` på auth-hændelser (✅ delvist implementeret for DM-adgang)
- [ ] JWT i `httpOnly`-cookie frem for `localStorage`
- [ ] `email`-felt NOT NULL i database

---

## GDPR-compliance

Projektet gemmer persondata (email, brugernavn) på EU-borgere. Se ADR-0014.

- [ ] `DELETE /auth/me` — ret til sletning (slet bruger + tilknyttede beskeder)
- [ ] `GET /auth/me` — udvid til at returnere alle gemte felter (ret til indsigt)
- [ ] Tilføj privacy policy-side til frontend med oplysning om hvad der gemmes og hvorfor
- [ ] Dokumentér dataopbevaring: hvor længe gemmes beskeder og konti?
- [ ] Bekræft at PostgreSQL-data ikke replikeres til tredjelande (check VPS-lokation)
- [ ] Verifikationstoken er persondata — sørg for at det slettes ved verify og ved sletning af konto

---

## CIS Docker Benchmark

Hærdning af Docker-opsætning. Se ADR-0014.

- [ ] Kør backend-container som non-root bruger — tilføj til `chat/Dockerfile`:
  ```dockerfile
  RUN useradd -m appuser
  USER appuser
  ```
- [ ] Tilføj resource limits i `docker-compose.yml`:
  ```yaml
  deploy:
    resources:
      limits:
        memory: 256m
        cpus: "0.5"
  ```
- [ ] Sæt `read_only: true` på containere der ikke skriver til filsystem (chat)
- [ ] Sæt `no-new-privileges: true` på alle services:
  ```yaml
  security_opt:
    - no-new-privileges:true
  ```
- [ ] Kør `docker scout cves` eller `trivy image` mod bygget image efter deploy
