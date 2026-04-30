use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
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

#[derive(sqlx::FromRow, Serialize)]
pub struct MessageWithUser {
    pub id: Uuid,
    pub user: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
