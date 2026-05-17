//! `GET /v1/presence` — returnerer UUIDs på brugere med aktive WS-forbindelser.

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};

use crate::{auth::authenticate, AppState};

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate(&state, &headers).await?;
    let online = state.online_users.read().await;
    let ids: Vec<String> = online.keys().map(|id| id.to_string()).collect();
    Ok(Json(serde_json::json!({ "online": ids })))
}
