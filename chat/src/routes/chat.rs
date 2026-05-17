//! `/ws/:room_id` — WebSocket handler for real-time chat.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::decode_token, models::{Room, User}, ws::get_or_create_sender, AppState};

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

/// `GET /ws/:room_id?token=<jwt>` — upgrades to WebSocket after validating the
/// token.
///
/// The token is passed as a query parameter instead of an `Authorization`
/// header because the browser's WebSocket API does not allow setting custom
/// headers on the upgrade request. The token is validated before the upgrade
/// so unauthenticated requests are rejected with a proper HTTP 401.
pub async fn handler(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let user_id = decode_token(&query.token, &state.config.jwt_secret)
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Invalid token"}))))?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": "Database error"}))))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "User not found"}))))?;

    // Check membership for DM rooms
    let room = sqlx::query_as::<_, Room>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": "Database error"}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "Room not found"}))))?;

    if room.is_dm {
        let is_member = sqlx::query(
            "SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2",
        )
        .bind(room_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": "Database error"}))))?
        .is_some();

        if !is_member {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"detail": "Not a member of this DM"})),
            ));
        }
    }

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, room_id, user)))
}

async fn handle_socket(socket: WebSocket, state: AppState, room_id: Uuid, user: User) {
    let room_key = room_id.to_string();
    let tx = get_or_create_sender(&state.rooms, &room_key).await;
    let mut rx = tx.subscribe();

    let (mut sender, mut receiver) = socket.split();

    // Increment connection count; broadcast presence if this is the first connection
    {
        let mut online = state.online_users.write().await;
        let count = online.entry(user.id).or_insert(0);
        *count += 1;
        if *count == 1 {
            let _ = tx.send(serde_json::json!({
                "type": "presence",
                "user_id": user.id.to_string(),
                "online": true
            }).to_string());
        }
    }

    broadcast(&tx, "join", &user.username, None, None);

    // Forward incoming broadcast messages to this WebSocket client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let username = user.username.clone();
    let db = state.db.clone();
    let tx2 = tx.clone();
    // Read messages from this client, persist them, and broadcast to the room
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };

            // WebRTC signaling — forward with server-stamped `from`, never persisted
            if data["type"] == "signal" {
                if let Some(target) = data["target"].as_str() {
                    if Uuid::parse_str(target).is_ok() {
                        let mut fwd = data.clone();
                        fwd["from"] = serde_json::Value::String(user.id.to_string());
                        let _ = tx2.send(fwd.to_string());
                    }
                }
                continue;
            }

            let Some(content) = data["content"].as_str().map(str::trim).filter(|s| !s.is_empty() && s.len() <= 4000) else {
                continue;
            };

            let Ok(msg) = sqlx::query_as::<_, crate::models::Message>(
                "INSERT INTO messages (room_id, user_id, content) VALUES ($1, $2, $3) RETURNING *",
            )
            .bind(room_id)
            .bind(user.id)
            .bind(content)
            .fetch_one(&db)
            .await else {
                continue;
            };

            broadcast(&tx2, "message", &username, Some(content), Some(msg.created_at));
        }
    });

    // When either task exits (client disconnected or send error), abort the other
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Decrement connection count; broadcast offline if no connections remain
    {
        let mut online = state.online_users.write().await;
        if let Some(count) = online.get_mut(&user.id) {
            *count -= 1;
            if *count == 0 {
                online.remove(&user.id);
                let _ = tx.send(serde_json::json!({
                    "type": "presence",
                    "user_id": user.id.to_string(),
                    "online": false
                }).to_string());
            }
        }
    }

    broadcast(&tx, "leave", &user.username, None, None);
}

/// Sends a JSON event to all subscribers in the room.
///
/// `tx.send` returns `Err` when there are no active receivers — this is
/// expected (e.g. the last client just left) and can be safely discarded.
fn broadcast(
    tx: &tokio::sync::broadcast::Sender<String>,
    kind: &str,
    user: &str,
    content: Option<&str>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
) {
    let mut msg = serde_json::json!({"type": kind, "user": user});
    if let Some(c) = content {
        msg["content"] = serde_json::Value::String(c.to_string());
    }
    if let Some(ts) = created_at {
        msg["created_at"] = serde_json::Value::String(ts.to_rfc3339());
    }
    let _ = tx.send(msg.to_string());
}
