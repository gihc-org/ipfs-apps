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
            turnstile_secret: String::new(),
            resend_api_key: String::new(),
            resend_from: "noreply@test.example".into(),
            base_url: "http://localhost:8001".into(),
            frontend_url: "http://localhost:8001".into(),
        },
        rooms: Arc::new(RwLock::new(HashMap::new())) as RoomMap,
        http: reqwest::Client::new(),
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
    let email = format!("{username}@test.example");
    api(
        app,
        "POST",
        "/auth/register",
        None,
        Some(json!({"username": username, "password": password, "email": email, "turnstile_token": "test"})),
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
        Some(json!({"username": "alice", "password": "password123", "email": "alice@test.example", "turnstile_token": "test"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["username"], "alice");
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn register_duplicate_username(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "alice", "password": "password123", "email": "alice@test.example", "turnstile_token": "test"}))).await;
    let (status, body) = api(
        &app,
        "POST",
        "/auth/register",
        None,
        Some(json!({"username": "alice", "password": "different1", "email": "alice2@test.example", "turnstile_token": "test"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["detail"], "Brugernavnet er allerede taget");
}

// ── /auth/token ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn login_success(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "bob", "password": "password123", "email": "bob@test.example", "turnstile_token": "test"}))).await;
    let (status, body) = api(
        &app,
        "POST",
        "/auth/token",
        None,
        Some(json!({"username": "bob", "password": "password123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["access_token"].is_string());
    assert_eq!(body["token_type"], "bearer");
}

#[sqlx::test(migrations = "./migrations")]
async fn login_wrong_password(pool: PgPool) {
    let app = build_app(test_state(pool));
    api(&app, "POST", "/auth/register", None, Some(json!({"username": "carol", "password": "password123", "email": "carol@test.example", "turnstile_token": "test"}))).await;
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
    let token = register_and_login(&app,"dave", "password").await;
    let (status, body) = api(&app, "GET", "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "dave");
    assert!(body["email"].is_string());
    assert!(body["email_verified"].is_boolean());
    assert!(body["created_at"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_me_removes_account(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"todelete", "password").await;
    let (status, _) = api(&app, "DELETE", "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Token is now invalid — account is gone
    let (status, _) = api(&app, "GET", "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_me_removes_dm_rooms(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": user_b["id"]}))).await;

    api(&app, "DELETE", "/auth/me", Some(&token_a), None).await;

    // Bob's DM list should now be empty
    let (status, body) = api(&app, "GET", "/dms", Some(&token_b), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_me_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "DELETE", "/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
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
    let token = register_and_login(&app,"eve", "password").await;
    let (status, body) = api(&app, "GET", "/rooms", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[sqlx::test(migrations = "./migrations")]
async fn create_room_success(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"frank", "password").await;
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
    let token = register_and_login(&app,"grace", "password").await;
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
    let token = register_and_login(&app,"henry", "password").await;
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
    let token = register_and_login(&app,"iris", "password").await;
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
    let token = register_and_login(&app,"jack", "password").await;
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

// ── /users ─────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn list_users_excludes_self(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"alice", "password").await;
    register_and_login(&app,"bob", "password").await;
    let (status, body) = api(&app, "GET", "/users", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let users = body.as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["username"], "bob");
}

#[sqlx::test(migrations = "./migrations")]
async fn search_users_finds_by_substring(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"alice", "password").await;
    register_and_login(&app,"bobby", "password").await;
    register_and_login(&app,"carol", "password").await;
    let (status, body) = api(&app, "GET", "/users?search=bob", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let users = body.as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["username"], "bobby");
}

#[sqlx::test(migrations = "./migrations")]
async fn list_users_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── /dms ───────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn create_dm_returns_room(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    let bob_id = user_b["id"].as_str().unwrap();

    let (status, body) = api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": bob_id}))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["room_id"].is_string());
    assert_eq!(body["other_user"], "bob");
}

#[sqlx::test(migrations = "./migrations")]
async fn dm_is_idempotent(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    let (_, user_a) = api(&app, "GET", "/auth/me", Some(&token_a), None).await;
    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    let alice_id = user_a["id"].as_str().unwrap();
    let bob_id = user_b["id"].as_str().unwrap();

    let (_, dm1) = api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": bob_id}))).await;
    // Bob creates DM with Alice — same room
    let (_, dm2) = api(&app, "POST", "/dms", Some(&token_b),
        Some(json!({"user_id": alice_id}))).await;
    assert_eq!(dm1["room_id"], dm2["room_id"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_dm_self(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"alice", "password").await;
    let (_, me) = api(&app, "GET", "/auth/me", Some(&token), None).await;
    let (status, _) = api(&app, "POST", "/dms", Some(&token),
        Some(json!({"user_id": me["id"]}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_dm_nonexistent_user(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token = register_and_login(&app,"alice", "password").await;
    let (status, _) = api(&app, "POST", "/dms", Some(&token),
        Some(json!({"user_id": "00000000-0000-0000-0000-000000000000"}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_dms_shows_conversations(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;

    api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": user_b["id"]}))).await;

    let (status, body) = api(&app, "GET", "/dms", Some(&token_a), None).await;
    assert_eq!(status, StatusCode::OK);
    let dms = body.as_array().unwrap();
    assert_eq!(dms.len(), 1);
    assert_eq!(dms[0]["other_username"], "bob");
    assert!(dms[0]["room_id"].is_string());
    assert!(dms[0]["other_user_id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn list_dms_requires_auth(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/dms", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── DM membership enforcement ──────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn rooms_list_excludes_dm_rooms(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;

    api(&app, "POST", "/rooms", Some(&token_a),
        Some(json!({"name": "public"}))).await;

    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": user_b["id"]}))).await;

    let (status, body) = api(&app, "GET", "/rooms", Some(&token_a), None).await;
    assert_eq!(status, StatusCode::OK);
    let rooms = body.as_array().unwrap();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0]["name"], "public");
}

#[sqlx::test(migrations = "./migrations")]
async fn non_member_cannot_read_dm_messages(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    let token_c = register_and_login(&app,"carol", "password").await;

    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    let (_, dm) = api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": user_b["id"]}))).await;
    let room_id = dm["room_id"].as_str().unwrap();

    let (status, _) = api(&app, "GET",
        &format!("/rooms/{room_id}/messages"), Some(&token_c), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn member_can_read_dm_messages(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;

    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    let (_, dm) = api(&app, "POST", "/dms", Some(&token_a),
        Some(json!({"user_id": user_b["id"]}))).await;
    let room_id = dm["room_id"].as_str().unwrap();

    // Bob, the other member, can read messages
    let (status, body) = api(&app, "GET",
        &format!("/rooms/{room_id}/messages"), Some(&token_b), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_dm_creation_yields_same_room(pool: PgPool) {
    let app = build_app(test_state(pool));
    let token_a = register_and_login(&app,"alice", "password").await;
    let token_b = register_and_login(&app,"bob", "password").await;
    
    let (_, user_a) = api(&app, "GET", "/auth/me", Some(&token_a), None).await;
    let (_, user_b) = api(&app, "GET", "/auth/me", Some(&token_b), None).await;
    let alice_id = user_a["id"].as_str().unwrap();
    let bob_id = user_b["id"].as_str().unwrap();

    // Simulate concurrent DM creation: both users create DM with each other simultaneously
    let app_clone = app.clone();
    let token_a_clone = token_a.clone();
    let token_b_clone = token_b.clone();
    let bob_id_clone = bob_id.to_string();
    let alice_id_clone = alice_id.to_string();

    let (dm1, dm2) = tokio::join!(
        async {
            api(&app, "POST", "/dms", Some(&token_a_clone),
                Some(json!({"user_id": bob_id_clone}))).await
        },
        async {
            api(&app_clone, "POST", "/dms", Some(&token_b_clone),
                Some(json!({"user_id": alice_id_clone}))).await
        }
    );

    // Both requests should succeed and return the same room_id
    let (status1, body1) = dm1;
    let (status2, body2) = dm2;
    assert_eq!(status1, StatusCode::OK);
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(body1["room_id"], body2["room_id"]);
}
