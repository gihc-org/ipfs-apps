//! SQLx row types — one struct per database table (plus one join projection).

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// A registered user. `password_hash` and `verification_token` are excluded
/// from JSON serialization so they can never appear in an API response.
#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub email: Option<String>,
    pub email_verified: bool,
    #[serde(skip)]
    pub verification_token: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub is_dm: bool,
    pub created_at: DateTime<Utc>,
}

/// Result of listing DM conversations for the authenticated user.
#[derive(sqlx::FromRow, Serialize)]
pub struct DmConversation {
    pub room_id: Uuid,
    pub other_user_id: Uuid,
    pub other_username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Message {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// Result of the `messages JOIN users` query. The `user` field is mapped from
/// the SQL alias `u.username AS "user"` and is the display name for the frontend.
#[derive(sqlx::FromRow, Serialize)]
pub struct MessageWithUser {
    pub id: Uuid,
    pub user: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
