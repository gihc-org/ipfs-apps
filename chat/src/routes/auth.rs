//! `/auth/*` — register, login, identity, and email verification endpoints.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Redirect,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{authenticate, create_token, hash_password, verify_password},
    captcha, email,
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
    pub email: String,
    /// Turnstile challenge response token from the frontend widget.
    pub turnstile_token: String,
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

#[derive(Deserialize)]
pub struct VerifyQuery {
    token: Uuid,
}

/// `POST /auth/register` — validates the CAPTCHA, creates a user, and sends a
/// verification email. The account cannot be used to log in until the email
/// link is clicked.
///
/// If `TURNSTILE_SECRET` or `RESEND_API_KEY` are empty (local dev), those
/// steps are skipped and the account is auto-verified.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    // 1. Validate CAPTCHA
    if !captcha::verify(&state.http, &state.config.turnstile_secret, &body.turnstile_token).await {
        return Err(err(StatusCode::BAD_REQUEST, "CAPTCHA-validering fejlede"));
    }

    // 2. Check username uniqueness
    if sqlx::query("SELECT id FROM users WHERE username = $1")
        .bind(&body.username)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .is_some()
    {
        return Err(err(StatusCode::BAD_REQUEST, "Brugernavnet er allerede taget"));
    }

    // 3. Check email uniqueness
    if sqlx::query("SELECT id FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .is_some()
    {
        return Err(err(StatusCode::BAD_REQUEST, "Email-adressen er allerede i brug"));
    }

    // 4. In local dev (no API key), auto-verify so the account is immediately usable
    let auto_verify = state.config.resend_api_key.is_empty();
    let verification_token = if auto_verify { None } else { Some(Uuid::new_v4()) };

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password_hash, email, email_verified, verification_token)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(&body.username)
    .bind(hash_password(&body.password))
    .bind(&body.email)
    .bind(auto_verify)
    .bind(verification_token)
    .fetch_one(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke oprette bruger"))?;

    // 5. Send verification email (skipped in local dev)
    if let Some(token) = verification_token {
        let verify_url = format!("{}/auth/verify?token={}", state.config.base_url, token);
        email::send_verification(
            &state.http,
            &state.config.resend_api_key,
            &state.config.resend_from,
            &body.email,
            &verify_url,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to send verification email: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke sende verifikationsmail")
        })?;
    }

    let msg = if auto_verify {
        "Konto oprettet — du kan nu logge ind."
    } else {
        "Konto oprettet — tjek din email og klik på bekræftelseslinket."
    };

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"id": user.id, "username": user.username, "message": msg})),
    ))
}

/// `POST /auth/token` — validates credentials and returns a JWT bearer token.
///
/// Returns 403 if the email has not been verified yet, 401 for wrong credentials.
/// Both unknown usernames and wrong passwords return the same 401 message to
/// avoid leaking information about which users exist.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
        .bind(&body.username)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Forkert brugernavn eller adgangskode"))?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err(err(StatusCode::UNAUTHORIZED, "Forkert brugernavn eller adgangskode"));
    }

    if !user.email_verified {
        return Err(err(StatusCode::FORBIDDEN, "Bekræft din email før du logger ind"));
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

/// `GET /auth/verify?token=<uuid>` — marks the user's email as verified and
/// redirects to the frontend.
///
/// The link is sent by email after registration. After verification the user
/// can log in normally.
pub async fn verify(
    State(state): State<AppState>,
    Query(params): Query<VerifyQuery>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    let updated = sqlx::query(
        "UPDATE users SET email_verified = TRUE, verification_token = NULL
         WHERE verification_token = $1",
    )
    .bind(params.token)
    .execute(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if updated.rows_affected() == 0 {
        return Err(err(StatusCode::BAD_REQUEST, "Ugyldigt eller allerede brugt verifikationslink"));
    }

    let redirect_url = format!("{}?verified=true", state.config.frontend_url);
    Ok(Redirect::to(&redirect_url))
}
