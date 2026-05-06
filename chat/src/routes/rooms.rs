//! `/rooms/*` — list rooms, create rooms, and fetch message history.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::authenticate,
    models::{MessageWithUser, Room},
    AppState,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({"detail": msg})))
}

#[derive(Deserialize)]
pub struct CreateRoom {
    pub name: String,
}

/// `GET /rooms` — returns public rooms (excludes DM rooms) ordered by creation
/// time. Requires auth.
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Room>>, ApiError> {
    authenticate(&state, &headers).await?;

    let rooms = sqlx::query_as::<_, Room>(
        "SELECT * FROM rooms WHERE is_dm = FALSE ORDER BY created_at"
    )
        .fetch_all(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(rooms))
}

/// `POST /rooms` — creates a new room. Returns 400 if the name already exists.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRoom>,
) -> Result<(StatusCode, Json<Room>), ApiError> {
    authenticate(&state, &headers).await?;

    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(err(StatusCode::BAD_REQUEST, "Rum-navn skal være 1–100 tegn"));
    }

    let existing = sqlx::query("SELECT id FROM rooms WHERE name = $1")
        .bind(name)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if existing.is_some() {
        return Err(err(StatusCode::BAD_REQUEST, "Room already exists"));
    }

    let room = sqlx::query_as::<_, Room>("INSERT INTO rooms (name) VALUES ($1) RETURNING *")
        .bind(name)
        .fetch_one(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create room"))?;

    Ok((StatusCode::CREATED, Json(room)))
}

/// `GET /rooms/:id/messages` — returns the 50 most recent messages with
/// their author's username. The LIMIT keeps response sizes bounded; a
/// cursor-based pagination endpoint would be the next step if history grows.
///
/// For DM rooms the authenticated user must be a member of the room.
pub async fn messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<MessageWithUser>>, ApiError> {
    let current_user = authenticate(&state, &headers).await?;

    // Check membership for DM rooms
    let room = sqlx::query_as::<_, Room>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "Rum ikke fundet"))?;

    if room.is_dm {
        let is_member = sqlx::query(
            "SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2",
        )
        .bind(room_id)
        .bind(current_user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .is_some();

        if !is_member {
            return Err(err(
                StatusCode::FORBIDDEN,
                "Du er ikke medlem af denne samtale",
            ));
        }
    }

    let messages = sqlx::query_as::<_, MessageWithUser>(
        r#"
        SELECT m.id, u.username AS "user", m.content, m.created_at
        FROM messages m
        JOIN users u ON u.id = m.user_id
        WHERE m.room_id = $1
        ORDER BY m.created_at
        LIMIT 50
        "#,
    )
    .bind(room_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(messages))
}
