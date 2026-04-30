use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{HeaderMap, StatusCode};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{models::User, AppState};

pub type ApiError = (StatusCode, axum::Json<serde_json::Value>);

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

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

pub fn decode_token(token: &str, secret: &str) -> Option<Uuid> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .and_then(|data| Uuid::parse_str(&data.claims.sub).ok())
}

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

pub fn err(msg: &str) -> ApiError {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({"detail": msg})),
    )
}
