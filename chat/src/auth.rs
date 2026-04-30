//! Authentication primitives: password hashing, JWT creation/validation,
//! and the per-request `authenticate` helper used by all protected handlers.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{HeaderMap, StatusCode};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{models::User, AppState};

/// Axum handler error type: HTTP status + JSON `{"detail": "..."}` body.
pub type ApiError = (StatusCode, axum::Json<serde_json::Value>);

/// JWT payload. `sub` holds the user's UUID as a string; `exp` is a Unix
/// timestamp. Both fields are validated automatically by `jsonwebtoken`.
#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

/// Hashes a plaintext password with Argon2id and a random salt.
///
/// Argon2id is intentionally slow and memory-hard — the latency during login
/// is a feature, not a bug. The output is a self-contained PHC string that
/// includes the algorithm parameters and salt.
pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

/// Returns `true` if `password` matches the stored Argon2 `hash`.
///
/// Returns `false` for a malformed hash rather than panicking, so a corrupted
/// row in the database degrades gracefully to "invalid credentials".
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Creates a signed HS256 JWT containing the user's UUID.
///
/// The token expires after `expire_hours` hours. The secret is the raw bytes
/// of `JWT_SECRET` from the environment.
pub fn create_token(user_id: Uuid, secret: &str, expire_hours: i64) -> String {
    let exp = (chrono::Utc::now() + chrono::Duration::hours(expire_hours)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

/// Validates a JWT's signature and expiry, returning the user UUID on success.
///
/// `jsonwebtoken` checks both the HMAC signature and the `exp` claim, so an
/// expired token returns `None` without further logic.
pub fn decode_token(token: &str, secret: &str) -> Option<Uuid> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .and_then(|data| Uuid::parse_str(&data.claims.sub).ok())
}

/// Extracts and validates the `Authorization: Bearer <token>` header, then
/// fetches the corresponding user from the database.
///
/// Implemented as a plain `async fn` rather than `FromRequestParts` because
/// Rust 1.88 tightened lifetime rules for async fns in traits, which broke
/// the extractor approach in ways that are difficult to work around cleanly.
pub async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| err("Missing token"))?;

    let user_id = decode_token(token, &state.config.jwt_secret)
        .ok_or_else(|| err("Invalid token"))?;

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err("Database error"))?
        .ok_or_else(|| err("User not found"))
}

/// Convenience constructor for a 401 `ApiError`.
pub fn err(msg: &str) -> ApiError {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({"detail": msg})),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("correct-password");
        assert!(verify_password("correct-password", &hash));
        assert!(!verify_password("wrong-password", &hash));
    }

    #[test]
    fn verify_rejects_malformed_hash() {
        assert!(!verify_password("any-password", "not-a-valid-argon2-hash"));
    }

    #[test]
    fn token_roundtrip() {
        let id = Uuid::new_v4();
        let token = create_token(id, "test-secret", 24);
        assert_eq!(decode_token(&token, "test-secret"), Some(id));
    }

    #[test]
    fn token_wrong_secret_rejected() {
        let id = Uuid::new_v4();
        let token = create_token(id, "secret-a", 24);
        assert!(decode_token(&token, "secret-b").is_none());
    }

    #[test]
    fn token_garbage_rejected() {
        assert!(decode_token("not.a.jwt", "secret").is_none());
    }

    #[test]
    fn expired_token_rejected() {
        let id = Uuid::new_v4();
        // Build a token whose exp is already in the past
        let exp = (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize;
        let claims = Claims {
            sub: id.to_string(),
            exp,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(decode_token(&token, "secret").is_none());
    }
}
