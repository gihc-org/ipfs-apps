//! In-memory WebSocket broadcast channels, one per chat room.

use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};

/// Map from room UUID (as string) to its broadcast sender.
///
/// `Arc<RwLock<...>>` so the map can be shared across Tokio tasks without
/// blocking. The map itself is rarely written to (only when a new room gets
/// its first connection), so a read-write lock avoids contention.
pub type RoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;

/// Returns the existing broadcast sender for `room_id`, or creates a new one.
///
/// Uses a double-checked locking pattern: take a cheap read lock first to
/// handle the common case (room already exists), and only upgrade to a write
/// lock when the room is absent. A second check after acquiring the write lock
/// guards against two tasks racing to create the same room simultaneously.
///
/// The channel capacity is 256. If a receiver falls too far behind (e.g. a
/// slow WebSocket client), it receives a `RecvError::Lagged` and is
/// disconnected — this prevents one slow client from stalling the whole room.
pub async fn get_or_create_sender(rooms: &RoomMap, room_id: &str) -> broadcast::Sender<String> {
    {
        let read = rooms.read().await;
        if let Some(tx) = read.get(room_id) {
            return tx.clone();
        }
    }
    let mut write = rooms.write().await;
    if let Some(tx) = write.get(room_id) {
        return tx.clone();
    }
    let (tx, _) = broadcast::channel(256);
    write.insert(room_id.to_string(), tx.clone());
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn same_room_id_returns_same_channel() {
        let rooms: RoomMap = Arc::new(RwLock::new(HashMap::new()));
        let tx1 = get_or_create_sender(&rooms, "room-1").await;
        let tx2 = get_or_create_sender(&rooms, "room-1").await;
        // If both senders share the same channel, a message sent via tx1 arrives on rx2
        let mut rx2 = tx2.subscribe();
        tx1.send("hello".into()).unwrap();
        assert_eq!(rx2.recv().await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn different_room_ids_are_isolated() {
        let rooms: RoomMap = Arc::new(RwLock::new(HashMap::new()));
        let tx_a = get_or_create_sender(&rooms, "room-a").await;
        let tx_b = get_or_create_sender(&rooms, "room-b").await;
        let mut rx_b = tx_b.subscribe();
        // A broadcast send fails when there are zero subscribers, so we need
        // at least one receiver on tx_a for the send to succeed.
        let _rx_a = tx_a.subscribe();
        tx_a.send("hello".into()).unwrap();
        // room-b's receiver should see nothing from room-a's sender
        assert!(rx_b.try_recv().is_err());
    }

    #[tokio::test]
    async fn concurrent_creation_yields_single_channel() {
        let rooms: RoomMap = Arc::new(RwLock::new(HashMap::new()));
        let r = rooms.clone();
        let (tx1, tx2) = tokio::join!(
            get_or_create_sender(&rooms, "race"),
            get_or_create_sender(&r, "race"),
        );
        // Both handles should point to the same channel
        let mut rx = tx2.subscribe();
        tx1.send("ping".into()).unwrap();
        assert_eq!(rx.recv().await.unwrap(), "ping");
    }
}
