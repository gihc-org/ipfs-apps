# OWASP Security Review — ipfs-apps/chat

Gennemgang pr. 2026-05-05 baseret på OWASP Top 10 (2021) og ASVS.

---

## Rettet

| # | Fund | OWASP | Fil |
|---|------|-------|-----|
| 1 | **Stored XSS** — `${r.name}` indsat i `innerHTML` uden escaping | A03 Injection | `frontend/rooms.html` |
| 2 | **Stored XSS** — `${user}` (username) indsat i `innerHTML` uden escaping | A03 Injection | `frontend/chat.html` |
| 3 | **Ingen security headers** — CSP, X-Frame-Options, X-Content-Type-Options manglede | A05 Misconfiguration | `ansible/templates/Caddyfile.j2` |
| 4 | **Ingen input-validering** — password (min 8), username (2–50), email (max 254) manglede | A04 Insecure Design | `chat/src/routes/auth.rs` |
| 5 | **Email enumeration** — "Email allerede i brug" afslørede om en email var registreret | A07 Auth Failures | `chat/src/routes/auth.rs` |
| 6 | **Rum-navn uden validering** — ingen trimning eller max-længde i backend | A04 Insecure Design | `chat/src/routes/rooms.rs` |
| 7 | **Ubegrænset besked-payload** — WebSocket accepterede vilkårligt store beskeder | A04 Insecure Design | `chat/src/routes/chat.rs` |

---

## Tilbageværende — prioriteret

### Høj

#### Rate limiting på auth-endpoints (A07)
Ingen begrænsning på login- og registreringsforsøg muliggør brute force og credential stuffing.

**Løsning:** Tilføj `tower_governor`-crate som middleware på `/auth/token` og `/auth/register`, f.eks. 5 req/min pr. IP.

```toml
# Cargo.toml
tower_governor = "0.4"
```

```rust
// lib.rs — tilføj til build_app()
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

let governor_conf = GovernorConfigBuilder::default()
    .per_second(5)
    .burst_size(10)
    .finish()
    .unwrap();

Router::new()
    // ...routes...
    .layer(GovernorLayer { config: Arc::new(governor_conf) })
```

---

#### JWT-token revokering ved logout (A07)
Logout sletter kun `localStorage` i browseren — JWT'en er stadig gyldig server-side i op til 24 timer. En stjålet token kan misbruges indtil udløb.

**Løsning (kortsigtet):** Sæt `JWT_EXPIRE_HOURS` til 1 (eller lavere) via env var — minimerer eksponeringsvinduet uden kodeændringer.

**Løsning (langsigtet):** Indfør en server-side token-blocklist ved logout:

```sql
-- migration
CREATE TABLE revoked_tokens (
    jti UUID PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX ON revoked_tokens (expires_at);
```

```rust
// Tilføj jti-claim til JWT, check blocklist i authenticate()
// Kør periodisk cleanup af udløbne rækker
```

---

#### Verifikationstoken udløber aldrig (A07)
Emaillinket er gyldigt for evigt (eksplicit dokumenteret i emailteksten). Et lækket verifikationslink kan bruges på et vilkårligt tidspunkt.

**Løsning:** Tilføj udløbstidspunkt i migration og check det ved verify-endpoint.

```sql
-- migration
ALTER TABLE users ADD COLUMN verification_token_expires_at TIMESTAMPTZ;
```

```rust
// Ved oprettelse
let expires_at = Utc::now() + Duration::hours(48);

// I verify-handler — tilføj til WHERE-clause
"UPDATE users SET email_verified = TRUE, verification_token = NULL,
 verification_token_expires_at = NULL
 WHERE verification_token = $1
 AND verification_token_expires_at > NOW()"
```

Opdater emailteksten: *"Linket udløber om 48 timer."*

---

### Mellemhøj

#### JWT i localStorage er sårbart over for XSS (A02)
`localStorage` er tilgængeligt fra JavaScript — et XSS-angreb kan eksfiltrere token. De kendte XSS-huller er nu lukket, men `localStorage` er et vedvarende svagt punkt.

**Løsning:** Brug `httpOnly`-session-cookie i stedet for Bearer token. Kræver:
- Backend sætter `Set-Cookie: session=<token>; HttpOnly; Secure; SameSite=Strict`
- CORS konfigureres med `allow_credentials(true)` og specifik origin (ikke `*`)
- Frontend fjerner `Authorization`-header og stoler på at browseren sender cookie
- WebSocket-auth skal gentænkes (cookie sendes automatisk med WS-upgrade på samme origin)

---

#### Ingen account lockout efter fejlede loginforsøg (A07)
Ingen beskyttelse mod online password guessing ud over rate limiting.

**Løsning:** Tæl fejlede forsøg pr. bruger i DB og lås midlertidigt:

```sql
-- migration
ALTER TABLE users ADD COLUMN failed_login_attempts INT NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN locked_until TIMESTAMPTZ;
```

```rust
// I login-handler
if user.locked_until.map(|t| t > Utc::now()).unwrap_or(false) {
    return Err(err(StatusCode::TOO_MANY_REQUESTS, "Konto midlertidigt låst"));
}
// Ved fejlet login: inkrementer tæller, lås ved >= 10
// Ved succesfuldt login: nulstil tæller
```

---

#### Ingen sikkerhedslogning (A09)
Fejlede loginforsøg, JWT-valideringsfejl og registreringer logges ikke. Det er umuligt at opdage angreb i efterfølgende loganalyse.

**Løsning:** Tilføj `tracing::warn!` på sikkerhedsrelevante hændelser:

```rust
// I login-handler ved fejlet password
tracing::warn!(username = %body.username, "Failed login attempt");

// I authenticate() ved ugyldig token
tracing::warn!("Invalid JWT presented");

// I register-handler ved succesfuld oprettelse
tracing::info!(username = %body.username, "New user registered");
```

---

### Lav

#### Ingen automatisk dependency-scanning (A06)
Kendte sårbarheder i afhængigheder opdages ikke automatisk.

**Løsning:** Kør `cargo audit` som del af deploy-pipeline:

```yaml
# ansible/playbook.yml — tilføj task inden docker compose build
- name: Audit Rust dependencies
  command: cargo audit
  args:
    chdir: "{{ project_dir }}/chat"
  failed_when: false  # advar men blokér ikke deploy
```

Eller installer `cargo-audit` lokalt og kør før commit:
```bash
cargo install cargo-audit
cargo audit
```

---

#### `email`-felt er nullable trods obligatorisk ved registration (data-integritet)
`users.email` er `TEXT` (nullable) i databasen, men registration kræver en email. Fremtidige kodestier kan fejlagtigt oprette brugere uden email.

**Løsning:** Tilføj `NOT NULL` constraint i en migration, efter at eksisterende rækker er opdateret:

```sql
-- migration
UPDATE users SET email = 'unknown@placeholder' WHERE email IS NULL;
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
```

Opdater modellen:
```rust
pub email: String,  // ikke længere Option<String>
```
