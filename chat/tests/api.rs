//! Integration tests for the HTTP API.
//!
//! These tests require a running PostgreSQL server. Set `DATABASE_URL` to a
//! superuser connection string (without a database name) before running:
//!
//! ```
//! DATABASE_URL=postgres://postgres:password@localhost:5432 cargo test
//! ```
//!
//! `#[sqlx::test]` creates a fresh database for each test and drops it after,
//! so tests can run in parallel without interfering with each other.

use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower::ServiceExt;

use chat::{AppState, Config, RoomMap, build_app};

fn test_state(db: PgPool) -> AppState {
    AppState {
        db,
        config: Config {
            database_url: String::new(),
            jwt_secret: "test-secret-key".into(),
            jwt_expire_hours: 24,
            allowed_origin: "*".into(),
        },
        rooms: Arc::new(RwLock::new(HashMap::new())) as RoomMap,
    }
}

/// Generic request helper. Clones the router for each call (required by oneshot).
async fn api(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(tok) = token {
        builder = builder.header("authorization", format!("Bearer {tok}"));
    }
    let req_body = if let Some(j) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(j.to_string())
    } else {
        Body::empty()
    };

    let resp = app
        .clone()
        .oneshot(builder.body(req_body).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// Registers a user and returns their JWT.
async fn register_and_login(app: &axum::Router, username: &str, password: &str) -> String {
    api(
        app,
        "POST",
        "/auth/register",
        None,
        Some(json!({"username": username, "password": password})),
    )
    .await;
    let (_, body) = api(
        app,
        "POST",
        "/auth/token",
        None,
        Some(json!({"username": username, "password": password})),
    )
    .await;
    body["access_token"].as_str().unwrap().to_string()
}

// ── /auth/register ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn register_success(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, body) = api(
        &app,
        "POST",
        "/auth/register",
        None,
        Some(json!({"username": "alice", "password": "password123"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["username"], "alice");
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn register_duplicate_username(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "alice", "password": "pw"}))).await;
    let (status, body) = api(
        &app,
        "POST",
        "/auth/register",
        None,
        Some(json!({"username": "alice", "password": "different"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["detail"], "Username already taken");
}

// ── /auth/token ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn login_success(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "bob", "password": "secret"}))).await;
    let (status, body) = api(
        &app,
        "POST",
        "/auth/token",
        None,
        Some(json!({"username": "bob", "password": "secret"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["access_token"].is_string());
    assert_eq!(body["token_type"], "bearer");
}

#[sqlx::test(migrations = "./migrations")]
async fn login_wrong_password(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "carol", "password": "correct"}))).await;
    let (status, _) = api(
        &app,
        "POST",
        "/auth/token",
        None,
        Some(json!({"username": "carol", "password": "wrong"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_unknown_user(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(
        &app,
        "POST",
        "/auth/token",
        None,
        Some(json!({"username": "nobody", "password": "pw"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── /auth/me ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn me_returns_current_user(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "dave", "pw").await;
    let (status, body) = api(&app, "GET", "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "dave");
}

#[sqlx::test(migrations = "./migrations")]
async fn me_without_token(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn me_with_invalid_token(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/auth/me", Some("not.a.jwt"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── /rooms ───────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn list_rooms_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/rooms", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_rooms_empty(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "eve", "pw").await;
    let (status, body) = api(&app, "GET", "/rooms", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[sqlx::test(migrations = "./migrations")]
async fn create_room_success(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "frank", "pw").await;
    let (status, body) = api(
        &app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({"name": "general"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "general");
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn create_room_duplicate(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "grace", "pw").await;
    api(&app, "POST", "/rooms", Some(&token), Some(json!({"name": "lobby"}))).await;
    let (status, body) = api(
        &app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({"name": "lobby"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["detail"], "Room already exists");
}

#[sqlx::test(migrations = "./migrations")]
async fn create_room_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "POST", "/rooms", None, Some(json!({"name": "secret"}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_rooms_shows_created_rooms(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "henry", "pw").await;
    api(&app, "POST", "/rooms", Some(&token), Some(json!({"name": "alpha"}))).await;
    api(&app, "POST", "/rooms", Some(&token), Some(json!({"name": "beta"}))).await;
    let (status, body) = api(&app, "GET", "/rooms", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let rooms = body.as_array().unwrap();
    assert_eq!(rooms.len(), 2);
    let names: Vec<&str> = rooms.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
}

// ── /rooms/:id/messages ──────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn messages_empty_for_new_room(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "iris", "pw").await;
    let (_, room) = api(
        &app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({"name": "empty-room"})),
    )
    .await;
    let room_id = room["id"].as_str().unwrap();
    let (status, body) = api(&app, "GET", &format!("/rooms/{room_id}/messages"), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[sqlx::test(migrations = "./migrations")]
async fn messages_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app, "jack", "pw").await;
    let (_, room) = api(
        &app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({"name": "private"})),
    )
    .await;
    let room_id = room["id"].as_str().unwrap();
    let (status, _) = api(&app, "GET", &format!("/rooms/{room_id}/messages"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
