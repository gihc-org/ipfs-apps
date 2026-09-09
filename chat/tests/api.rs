//! Integrationstests for Loft-API'et.
//!
//! Kræver en kørende PostgreSQL. Sæt `DATABASE_URL` til en superuser-forbindelse
//! (uden databasenavn) før kørsel:
//!
//! ```
//! DATABASE_URL=postgres://postgres:password@localhost:5432 cargo test
//! ```
//!
//! `#[sqlx::test]` opretter en frisk database pr. test og dropper den bagefter.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tower::ServiceExt;

use chat::{build_app, AppState, Config};

type WsSink = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    WsMessage,
>;
type WsStream = futures::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

fn test_state(pool: PgPool) -> AppState {
    AppState {
        db: pool,
        config: Config {
            database_url: String::new(),
            allowed_origin: "*".into(),
            loft_ttl_hours: 168,
        },
        loft_channels: Arc::new(RwLock::new(HashMap::new())),
        loft_registry: Arc::new(RwLock::new(HashMap::new())),
    }
}

/// Starter en rigtig TCP-listener på en tilfældig port (til WS-tests).
async fn start_server(pool: PgPool) -> std::net::SocketAddr {
    let app = build_app(test_state(pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn api(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
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

async fn create_loft(app: &axum::Router, name: Option<&str>) -> String {
    let body = name.map(|n| json!({"name": n}));
    let (status, resp) = api(app, "POST", "/v1/lofts", body).await;
    assert_eq!(status, StatusCode::CREATED, "create loft failed: {resp}");
    resp["id"].as_str().unwrap().to_string()
}

async fn ws_connect(addr: std::net::SocketAddr, loft_id: &str) -> (WsSink, WsStream) {
    let url = format!("ws://{addr}/v1/ws/{loft_id}");
    let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.split()
}

async fn ws_join(addr: std::net::SocketAddr, loft_id: &str, name: &str) -> (WsSink, WsStream) {
    let (mut tx, rx) = ws_connect(addr, loft_id).await;
    tx.send(WsMessage::Text(
        json!({"type": "join", "name": name}).to_string(),
    ))
    .await
    .unwrap();
    (tx, rx)
}

async fn recv_value(rx: &mut WsStream) -> Value {
    let msg = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .expect("timeout waiting for WS message")
        .expect("WS stream closed")
        .expect("WS error");
    serde_json::from_str(msg.to_text().unwrap()).unwrap()
}

async fn recv_until(rx: &mut WsStream, mut pred: impl FnMut(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let msg = recv_value(rx).await;
            if pred(&msg) {
                return msg;
            }
        }
    })
    .await
    .expect("timeout waiting for matching WS message")
}

fn participant_id<'a>(msg: &'a Value, name: &str) -> &'a str {
    msg["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
}

// ── /healthz ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn healthz_returns_ok(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, _) = api(&app, "GET", "/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
}

// ── /v1/lofts ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn create_loft_returns_id_and_url(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, body) = api(
        &app,
        "POST",
        "/v1/lofts",
        Some(json!({"name": "Morgenmøde"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().unwrap();
    assert!(body["name"].as_str().unwrap().contains("Morgenmøde"));
    assert_eq!(body["url"], format!("/loft.html?id={id}"));
}

#[sqlx::test(migrations = "./migrations")]
async fn create_loft_defaults_name(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, body) = api(&app, "POST", "/v1/lofts", Some(json!({}))).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "Loft");
}

#[sqlx::test(migrations = "./migrations")]
async fn create_loft_rejects_too_long_name(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, body) = api(
        &app,
        "POST",
        "/v1/lofts",
        Some(json!({"name": "x".repeat(51)})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["detail"].as_str().unwrap().contains("1–50 tegn"));
}

#[sqlx::test(migrations = "./migrations")]
async fn get_loft_returns_metadata(pool: PgPool) {
    let app = build_app(test_state(pool));
    let id = create_loft(&app, Some("Planlægning")).await;
    let (status, body) = api(&app, "GET", &format!("/v1/lofts/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "Planlægning");
    assert!(body["created_at"].is_string());
    assert!(body["last_active"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn get_unknown_loft_returns_404(pool: PgPool) {
    let app = build_app(test_state(pool));
    let (status, body) = api(
        &app,
        "GET",
        "/v1/lofts/00000000-0000-0000-0000-000000000000",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["detail"], "Loftet findes ikke");
}

// ── WebSocket: join/roster ─────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn ws_unknown_loft_is_rejected(pool: PgPool) {
    let addr = start_server(pool).await;
    let url = format!("ws://{addr}/v1/ws/00000000-0000-0000-0000-000000000000");
    assert!(
        tokio_tungstenite::connect_async(&url).await.is_err(),
        "WS til ukendt loft skal afvises"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn join_receives_roster_and_broadcasts(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (_, mut alice_rx) = ws_join(addr, &loft_id, "alice").await;
    let alice_roster = recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    assert_eq!(alice_roster["participants"].as_array().unwrap().len(), 1);
    let alice_id = participant_id(&alice_roster, "alice").to_string();
    let alice_join = recv_until(&mut alice_rx, |m| {
        m["type"] == "join" && m["participant"]["name"] == "alice"
    })
    .await;
    assert_eq!(alice_join["participant"]["id"], alice_id);

    let (_, mut bob_rx) = ws_join(addr, &loft_id, "bob").await;
    let bob_roster = recv_until(&mut bob_rx, |m| m["type"] == "roster").await;
    let names: Vec<&str> = bob_roster["participants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"alice"));
    assert!(names.contains(&"bob"));

    let bob_join = recv_until(&mut alice_rx, |m| {
        m["type"] == "join" && m["participant"]["name"] == "bob"
    })
    .await;
    assert_eq!(
        bob_join["participant"]["id"],
        participant_id(&bob_roster, "bob")
    );
}

// ── WebSocket: signalering ─────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn signal_is_forwarded_with_server_stamped_from(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (mut alice_tx, mut alice_rx) = ws_join(addr, &loft_id, "alice").await;
    let alice_roster = recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    let alice_id = participant_id(&alice_roster, "alice").to_string();

    let (_, mut bob_rx) = ws_join(addr, &loft_id, "bob").await;
    let bob_roster = recv_until(&mut bob_rx, |m| m["type"] == "roster").await;
    let bob_id = participant_id(&bob_roster, "bob").to_string();
    recv_until(&mut alice_rx, |m| m["type"] == "join").await;

    // Klienten forsøger at forfalske `from` — serveren skal overskrive den.
    alice_tx
        .send(WsMessage::Text(
            json!({
                "type": "signal",
                "from": "spoofed",
                "to": bob_id,
                "signal": {"type": "offer", "sdp": "v=0"}
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let signal = recv_until(&mut bob_rx, |m| m["type"] == "signal").await;
    assert_eq!(signal["from"]["id"], alice_id);
    assert_eq!(signal["from"]["name"], "alice");
    assert_eq!(signal["to"], bob_id);
    assert_eq!(signal["signal"]["type"], "offer");
}

#[sqlx::test(migrations = "./migrations")]
async fn signal_to_unknown_participant_is_dropped(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (mut alice_tx, mut alice_rx) = ws_join(addr, &loft_id, "alice").await;
    recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    let (_, mut bob_rx) = ws_join(addr, &loft_id, "bob").await;
    recv_until(&mut bob_rx, |m| m["type"] == "roster").await;
    // Bobs egen join-broadcast ligger stadig i køen — dræn den, så
    // timeout-vinduet nedenfor kun kan indeholde et (utilsigtet) signal.
    recv_until(&mut bob_rx, |m| m["type"] == "join").await;

    alice_tx
        .send(WsMessage::Text(
            json!({
                "type": "signal",
                "to": "00000000-0000-0000-0000-000000000000",
                "signal": {"type": "offer"}
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_millis(300), bob_rx.next()).await;
    assert!(
        result.is_err(),
        "Bob skal ikke modtage et signal til ukendt deltager"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn signal_before_join_is_ignored(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (mut alice_tx, mut alice_rx) = ws_connect(addr, &loft_id).await;
    alice_tx
        .send(WsMessage::Text(
            json!({"type": "signal", "to": "x", "signal": {}}).to_string(),
        ))
        .await
        .unwrap();
    alice_tx
        .send(WsMessage::Text(
            json!({"type": "join", "name": "alice"}).to_string(),
        ))
        .await
        .unwrap();

    let roster = recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    assert_eq!(roster["participants"].as_array().unwrap().len(), 1);
}

// ── WebSocket: relay og leave ──────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn media_state_is_relayed_with_from(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (mut alice_tx, mut alice_rx) = ws_join(addr, &loft_id, "alice").await;
    let alice_roster = recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    let alice_id = participant_id(&alice_roster, "alice").to_string();
    let (_, mut bob_rx) = ws_join(addr, &loft_id, "bob").await;
    recv_until(&mut bob_rx, |m| m["type"] == "roster").await;

    alice_tx
        .send(WsMessage::Text(
            json!({"type": "media-state", "audio": false, "video": true}).to_string(),
        ))
        .await
        .unwrap();

    let media = recv_until(&mut bob_rx, |m| m["type"] == "media-state").await;
    assert_eq!(media["from"]["id"], alice_id);
    assert_eq!(media["audio"], false);
    assert_eq!(media["video"], true);
}

#[sqlx::test(migrations = "./migrations")]
async fn disconnect_broadcasts_leave(pool: PgPool) {
    let app = build_app(test_state(pool.clone()));
    let loft_id = create_loft(&app, None).await;
    let addr = start_server(pool).await;

    let (mut alice_tx, mut alice_rx) = ws_join(addr, &loft_id, "alice").await;
    let alice_roster = recv_until(&mut alice_rx, |m| m["type"] == "roster").await;
    let alice_id = participant_id(&alice_roster, "alice").to_string();
    let (_, mut bob_rx) = ws_join(addr, &loft_id, "bob").await;
    recv_until(&mut bob_rx, |m| m["type"] == "roster").await;

    alice_tx.close().await.unwrap();

    let leave = recv_until(&mut bob_rx, |m| m["type"] == "leave").await;
    assert_eq!(leave["participant"]["id"], alice_id);
    assert_eq!(leave["participant"]["name"], "alice");
}
