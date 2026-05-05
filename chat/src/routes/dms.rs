//! `/users/*` and `/dms/*` — user listing, DM creation, and DM listing.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::authenticate,
    models::{DmConversation, Room, User},
    AppState,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({"detail": msg})))
}

#[derive(Deserialize)]
pub struct UserListQuery {
    search: Option<String>,
}

/// `GET /users` — list all users except the authenticated user.
/// `GET /users?search=term` — search by username (case-insensitive ILIKE).
/// Results are capped at 50 (or 20 for search) to keep responses bounded.
pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UserListQuery>,
) -> Result<Json<Vec<User>>, ApiError> {
    let current_user = authenticate(&state, &headers).await?;

    let users = if let Some(ref search) = params.search {
        let trimmed = search.trim();
        if trimmed.is_empty() {
            sqlx::query_as::<_, User>(
                "SELECT * FROM users WHERE id != $1 ORDER BY username LIMIT 50",
            )
            .bind(current_user.id)
            .fetch_all(&state.db)
            .await
        } else {
            // ILIKE pattern construction: The % wildcards are safe here because:
            // 1. The user input (trimmed) is bound as a parameter ($1), not concatenated into SQL
            // 2. format!() only constructs the pattern string, which is then passed to bind()
            // 3. SQLx escapes the parameter value, preventing SQL injection
            // Example: user input "bob" becomes pattern "%bob%" bound to $1
            let pattern = format!("%{}%", trimmed);
            sqlx::query_as::<_, User>(
                "SELECT * FROM users WHERE username ILIKE $1 AND id != $2 ORDER BY username LIMIT 20",
            )
            .bind(pattern)
            .bind(current_user.id)
            .fetch_all(&state.db)
            .await
        }
    } else {
        sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id != $1 ORDER BY username LIMIT 50",
        )
        .bind(current_user.id)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct CreateDm {
    pub user_id: Uuid,
}

/// `POST /dms` — create or return an existing DM room with another user.
///
/// Idempotent: the room name is deterministic (`dm:<low>:<high>` where UUIDs
/// are sorted), so both `A → B` and `B → A` produce the same room. The
/// UNIQUE constraint on `rooms.name` provides atomicity against concurrent
/// creation.
pub async fn create_or_get_dm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateDm>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let current_user = authenticate(&state, &headers).await?;

    if current_user.id == body.user_id {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Du kan ikke sende en direkte besked til dig selv",
        ));
    }

    let other_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(body.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "Brugeren blev ikke fundet"))?;

    // Deterministic room name: sort UUIDs so order is always stable
    let (id1, id2) = if current_user.id.to_string() < other_user.id.to_string() {
        (current_user.id, other_user.id)
    } else {
        (other_user.id, current_user.id)
    };
    let dm_name = format!("dm:{}:{}", id1, id2);

    // Optimistic fast path: check if room already exists before attempting INSERT.
    // This avoids the overhead of INSERT + ON CONFLICT in the common case where
    // the DM conversation already exists.
    if let Some(row) = sqlx::query("SELECT id FROM rooms WHERE name = $1")
        .bind(&dm_name)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    {
        let room_id: Uuid = row.get("id");
        return Ok((StatusCode::OK, Json(serde_json::json!({
            "room_id": room_id,
            "other_user": other_user.username,
        }))));
    }

    // Slow path: INSERT with ON CONFLICT to handle concurrent creation.
    // If another request creates the room between our check and this INSERT,
    // the UNIQUE constraint on rooms.name causes ON CONFLICT DO NOTHING to fire,
    // and we fall through to the final SELECT below.
    let room = sqlx::query_as::<_, Room>(
        "INSERT INTO rooms (name, is_dm) VALUES ($1, TRUE)
         ON CONFLICT (name) DO NOTHING RETURNING *",
    )
    .bind(&dm_name)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke oprette DM"))?;

    let room_id = if let Some(room) = room {
        let rid = room.id;
        sqlx::query(
            "INSERT INTO room_members (room_id, user_id) VALUES ($1, $2), ($3, $4)
             ON CONFLICT (room_id, user_id) DO NOTHING",
        )
        .bind(rid)
        .bind(id1)
        .bind(rid)
        .bind(id2)
        .execute(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
        rid
    } else {
        // Lost the race — another request created the room between our check and INSERT.
        // This SELECT should always succeed because the other request's INSERT committed.
        let row = sqlx::query("SELECT id FROM rooms WHERE name = $1")
            .bind(&dm_name)
            .fetch_one(&state.db)
            .await
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke hente DM-rum"))?;
        row.get("id")
    };

    tracing::info!(
        user_id = %current_user.id,
        other_user_id = %other_user.id,
        room_id = %room_id,
        "DM conversation created or accessed"
    );

    Ok((StatusCode::OK, Json(serde_json::json!({
        "room_id": room_id,
        "other_user": other_user.username,
    }))))
}

/// `GET /dms` — returns the authenticated user's DM conversations with the
/// other participant's user ID and username.
pub async fn list_dms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DmConversation>>, ApiError> {
    let current_user = authenticate(&state, &headers).await?;

    let dms = sqlx::query_as::<_, DmConversation>(
        r#"
        SELECT r.id AS room_id,
               u.id AS other_user_id,
               u.username AS other_username,
               r.created_at
        FROM rooms r
        JOIN room_members rm1 ON rm1.room_id = r.id AND rm1.user_id = $1
        JOIN room_members rm2 ON rm2.room_id = r.id AND rm2.user_id != $1
        JOIN users u ON u.id = rm2.user_id
        WHERE r.is_dm = TRUE
        ORDER BY r.created_at
        "#,
    )
    .bind(current_user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(dms))
}
