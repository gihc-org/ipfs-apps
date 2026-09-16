//! SQLx row-typer og protokol-typer.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Et link-rum ("loft"). `last_active` opdateres når nogen joiner/forlader og
/// styrer TTL-oprydningen.
#[derive(sqlx::FromRow, Serialize, Clone, Debug)]
pub struct Loft {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    /// Kun til skaberen — eksponeres aldrig i API-svar (GET).
    #[serde(skip)]
    pub owner_token: Option<Uuid>,
}

/// En gæst der aktuelt er forbundet til et loft. Findes kun i hukommelsen
/// (state), aldrig i databasen — derfor ingen persondata på disk.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Participant {
    pub id: Uuid,
    pub name: String,
}
