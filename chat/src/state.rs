//! In-memory tilstand pr. loft: broadcast-kanaler og deltager-registry.

use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::{models::Participant, AppState};

/// Broadcast-kanal pr. loft (Uuid → sender). Én kanal pr. loft svarer til
/// ADR-0006-mønsteret og faner join/leave/roster/signal ud til alle i loftet.
pub type LoftChannelMap = Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>;

/// Deltagere pr. loft (loft-id → deltager-id → Participant). Registry'et er
/// kilden til roster-beskeder og gør at kun forbundne deltagere kan modtage
/// signaler. Ryddes implicit når en WebSocket lukker.
pub type LoftRegistry = Arc<RwLock<HashMap<Uuid, HashMap<Uuid, Participant>>>>;

/// Returnerer den eksisterende sender for `loft_id`, eller opretter en ny.
///
/// Double-checked locking: hurtig read-lock først, write-lock kun når loftet
/// mangler. Kapacitet 256 — en langsom klient der falder for langt bagud får
/// `RecvError::Lagged` og afbrydes frem for at blokere hele loftet.
pub async fn get_or_create_channel(
    channels: &LoftChannelMap,
    loft_id: Uuid,
) -> broadcast::Sender<String> {
    {
        let read = channels.read().await;
        if let Some(tx) = read.get(&loft_id) {
            return tx.clone();
        }
    }
    let mut write = channels.write().await;
    if let Some(tx) = write.get(&loft_id) {
        return tx.clone();
    }
    let (tx, _) = broadcast::channel(256);
    write.insert(loft_id, tx.clone());
    tx
}

pub async fn roster(registry: &LoftRegistry, loft_id: Uuid) -> Vec<Participant> {
    registry
        .read()
        .await
        .get(&loft_id)
        .map(|participants| participants.values().cloned().collect())
        .unwrap_or_default()
}

pub async fn add_participant(registry: &LoftRegistry, loft_id: Uuid, participant: Participant) {
    registry
        .write()
        .await
        .entry(loft_id)
        .or_default()
        .insert(participant.id, participant);
}

pub async fn remove_participant(
    registry: &LoftRegistry,
    loft_id: Uuid,
    participant_id: Uuid,
) -> Option<Participant> {
    let mut write = registry.write().await;
    let removed = write
        .get_mut(&loft_id)
        .and_then(|participants| participants.remove(&participant_id));
    if removed.is_some() && write.get(&loft_id).is_some_and(|p| p.is_empty()) {
        write.remove(&loft_id);
    }
    removed
}

pub async fn participant_exists(
    registry: &LoftRegistry,
    loft_id: Uuid,
    participant_id: Uuid,
) -> bool {
    registry
        .read()
        .await
        .get(&loft_id)
        .is_some_and(|participants| participants.contains_key(&participant_id))
}

/// Sletter lofts der ikke har været aktive inden for TTL'en, og rydder
/// broadcast-kanaler for slettede lofts. Aktive lofts (registry) røres aldrig.
pub async fn cleanup_expired_lofts(state: &AppState) {
    let active: Vec<Uuid> = state.loft_registry.read().await.keys().copied().collect();

    match sqlx::query(
        "DELETE FROM lofts
         WHERE last_active < NOW() - ($1 * INTERVAL '1 hour')
           AND NOT (id = ANY($2))",
    )
    .bind(state.config.loft_ttl_hours)
    .bind(&active)
    .execute(&state.db)
    .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                tracing::info!(deleted = result.rows_affected(), "loft TTL cleanup");
            }
        }
        Err(e) => tracing::warn!(error = %e, "loft TTL cleanup failed"),
    }

    // Fjern kanaler for lofts der ikke længere findes i databasen.
    let db_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM lofts")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    state
        .loft_channels
        .write()
        .await
        .retain(|id, _| db_ids.contains(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn same_loft_id_returns_same_channel() {
        let channels: LoftChannelMap = Arc::new(RwLock::new(HashMap::new()));
        let loft_id = Uuid::new_v4();
        let tx1 = get_or_create_channel(&channels, loft_id).await;
        let tx2 = get_or_create_channel(&channels, loft_id).await;
        let mut rx2 = tx2.subscribe();
        tx1.send("hello".into()).unwrap();
        assert_eq!(rx2.recv().await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn different_loft_ids_are_isolated() {
        let channels: LoftChannelMap = Arc::new(RwLock::new(HashMap::new()));
        let loft_a = Uuid::new_v4();
        let loft_b = Uuid::new_v4();
        let tx_a = get_or_create_channel(&channels, loft_a).await;
        let tx_b = get_or_create_channel(&channels, loft_b).await;
        let mut rx_b = tx_b.subscribe();
        let _rx_a = tx_a.subscribe();
        tx_a.send("hello".into()).unwrap();
        assert!(rx_b.try_recv().is_err());
    }

    #[tokio::test]
    async fn concurrent_creation_yields_single_channel() {
        let channels: LoftChannelMap = Arc::new(RwLock::new(HashMap::new()));
        let loft_id = Uuid::new_v4();
        let (tx1, tx2) = tokio::join!(
            get_or_create_channel(&channels, loft_id),
            get_or_create_channel(&channels, loft_id),
        );
        let mut rx = tx2.subscribe();
        tx1.send("ping".into()).unwrap();
        assert_eq!(rx.recv().await.unwrap(), "ping");
    }

    #[tokio::test]
    async fn participant_lifecycle() {
        let registry: LoftRegistry = Arc::new(RwLock::new(HashMap::new()));
        let loft_id = Uuid::new_v4();
        let alice = Participant {
            id: Uuid::new_v4(),
            name: "alice".into(),
        };

        assert!(!participant_exists(&registry, loft_id, alice.id).await);
        add_participant(&registry, loft_id, alice.clone()).await;
        assert!(participant_exists(&registry, loft_id, alice.id).await);
        assert_eq!(roster(&registry, loft_id).await, vec![alice.clone()]);

        let removed = remove_participant(&registry, loft_id, alice.id).await;
        assert_eq!(removed, Some(alice));
        assert!(roster(&registry, loft_id).await.is_empty());
    }
}
