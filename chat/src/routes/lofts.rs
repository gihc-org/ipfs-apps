//! `/v1/lofts` og `/v1/ws/:loft_id` — link-rum med gæsteadgang.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::{
    models::{Loft, Participant},
    state, AppState,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({"detail": msg})))
}

const MAX_NAME_LEN: usize = 50;

/// `POST /v1/lofts` — opretter et loft og returnerer `{ id, name, url }`.
///
/// `url` er relativ (`/loft.html?id=…`) og skal prefixes med frontend-origin
/// af klienten — Loft bruger ét origin pr. miljø.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateLoft>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let name = match body.name.as_deref() {
        None => "Loft".to_string(),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                "Loft".to_string()
            } else if trimmed.chars().count() > MAX_NAME_LEN
                || trimmed.chars().any(|c| c.is_control())
            {
                return Err(err(
                    StatusCode::BAD_REQUEST,
                    "Loft-navn skal være 1–50 tegn uden kontroltegn",
                ));
            } else {
                trimmed.to_string()
            }
        }
    };

    let loft = sqlx::query_as::<_, Loft>("INSERT INTO lofts (name) VALUES ($1) RETURNING *")
        .bind(&name)
        .fetch_one(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke oprette loft"))?;

    let url = format!("/loft.html?id={}", loft.id);
    tracing::info!(loft_id = %loft.id, name = %loft.name, "loft created");

    Ok((
        StatusCode::CREATED,
        Json(json!({"id": loft.id, "name": loft.name, "url": url})),
    ))
}

#[derive(Deserialize)]
pub struct CreateLoft {
    #[serde(default)]
    name: Option<String>,
}

/// `GET /v1/lofts/:id` — returnerer metadata for et loft (link-åbning).
pub async fn get(
    State(state): State<AppState>,
    Path(loft_id): Path<Uuid>,
) -> Result<Json<Loft>, ApiError> {
    let loft = sqlx::query_as::<_, Loft>("SELECT * FROM lofts WHERE id = $1")
        .bind(loft_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "Loftet findes ikke"))?;
    Ok(Json(loft))
}

/// `GET /v1/ws/:loft_id` — WebSocket-håndtryk for et loft.
///
/// Ingen JWT/auth — selve loft-id'et (tilfældig UUID) er adgangsnøglen.
/// Klienten sender `{"type":"join","name":"…"}` som første besked; serveren
/// tildeler deltager-id og svarer med `roster`.
pub async fn ws_handler(
    State(state): State<AppState>,
    Path(loft_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let _loft = sqlx::query_as::<_, Loft>("SELECT * FROM lofts WHERE id = $1")
        .bind(loft_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "Loftet findes ikke"))?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, loft_id)))
}

async fn handle_socket(socket: WebSocket, state: AppState, loft_id: Uuid) {
    let channel = state::get_or_create_channel(&state.loft_channels, loft_id).await;
    let mut rx = channel.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Én deltager pr. forbindelse. Ved reconnect får klienten en ny id —
    // roster er kilden til sandheden, og gamle deltagere fjernes via leave.
    let mut me: Option<Participant> = None;

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(data) = serde_json::from_str::<Value>(&text) else {
                            continue;
                        };
                        match data["type"].as_str() {
                            Some("join") => {
                                if me.is_some() {
                                    continue; // duplikerede join-beskeder ignoreres
                                }
                                let Some(raw_name) = data["name"].as_str() else {
                                    continue;
                                };
                                let Some(name) = valid_name(raw_name) else {
                                    continue;
                                };

                                let participant = Participant {
                                    id: Uuid::new_v4(),
                                    name,
                                };
                                state::add_participant(
                                    &state.loft_registry,
                                    loft_id,
                                    participant.clone(),
                                )
                                .await;
                                touch_loft(&state.db, loft_id);

                                let roster = state::roster(&state.loft_registry, loft_id).await;
                                let _ = sender
                                    .send(Message::Text(
                                        json!({"type": "roster", "participants": roster})
                                            .to_string(),
                                    ))
                                    .await;
                                let _ = channel.send(
                                    json!({"type": "join", "participant": participant})
                                        .to_string(),
                                );
                                me = Some(participant);
                            }
                            Some("signal") => {
                                let Some(participant) = me.clone() else {
                                    continue; // skal joine først
                                };
                                let Some(to) = data["to"]
                                    .as_str()
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                else {
                                    continue;
                                };
                                if !state::participant_exists(
                                    &state.loft_registry,
                                    loft_id,
                                    to,
                                )
                                .await
                                {
                                    continue; // ukendt modtager — droppes
                                }
                                let mut fwd = data;
                                fwd["from"] =
                                    json!({"id": participant.id, "name": participant.name});
                                let _ = channel.send(fwd.to_string());
                            }
                            _ if me.is_some() => {
                                // Øvrige beskedtyper (fx media-state) relayes med
                                // server-stemplet afsender.
                                let participant = me.clone().unwrap();
                                let mut fwd = data;
                                fwd["from"] =
                                    json!({"id": participant.id, "name": participant.name});
                                let _ = channel.send(fwd.to_string());
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if sender.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }

    // Forbindelsen lukket — fjern deltageren og annoncér leave.
    if let Some(participant) = me {
        if state::remove_participant(&state.loft_registry, loft_id, participant.id)
            .await
            .is_some()
        {
            touch_loft(&state.db, loft_id);
            let _ = channel.send(json!({"type": "leave", "participant": participant}).to_string());
        }
    }
}

fn valid_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let len = name.chars().count();
    if len == 0 || len > MAX_NAME_LEN || name.chars().any(|c| c.is_control()) {
        None
    } else {
        Some(name.to_string())
    }
}

/// Opdaterer `last_active` ild-og-glem: fejl må ikke koble en deltager af.
fn touch_loft(db: &PgPool, loft_id: Uuid) {
    let db = db.clone();
    tokio::spawn(async move {
        if let Err(e) = sqlx::query("UPDATE lofts SET last_active = NOW() WHERE id = $1")
            .bind(loft_id)
            .execute(&db)
            .await
        {
            tracing::warn!(loft_id = %loft_id, error = %e, "failed to touch loft");
        }
    });
}
