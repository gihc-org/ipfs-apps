//! `/auth/*` — register, login, and identity endpoints.

use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{authenticate, create_token, hash_password, verify_password},
    models::User,
    AppState,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({"detail": msg})))
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
}

/// `POST /auth/register` — creates a new user and returns their id/username.
///
/// Returns 400 if the username is already taken. The password is hashed with
/// Argon2id before storage; the plaintext is never persisted.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let existing = sqlx::query("SELECT id FROM users WHERE username = $1")
        .bind(&body.username)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if existing.is_some() {
        return Err(err(StatusCode::BAD_REQUEST, "Username already taken"));
    }

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING *",
    )
    .bind(&body.username)
    .bind(hash_password(&body.password))
    .fetch_one(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create user"))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"id": user.id, "username": user.username})),
    ))
}

/// `POST /auth/token` — validates credentials and returns a JWT bearer token.
///
/// Returns a generic 401 for both unknown usernames and wrong passwords
/// (no information leakage about which users exist).
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
        .bind(&body.username)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let token = create_token(user.id, &state.config.jwt_secret, state.config.jwt_expire_hours);
    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".into(),
    }))
}

/// `GET /auth/me` — returns the authenticated user's id and username.
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    Ok(Json(serde_json::json!({"id": user.id, "username": user.username})))
}
