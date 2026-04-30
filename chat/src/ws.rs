use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};

pub type RoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;

pub async fn get_or_create_sender(rooms: &RoomMap, room_id: &str) -> broadcast::Sender<String> {
    {
        let read = rooms.read().await;
        if let Some(tx) = read.get(room_id) {
            return tx.clone();
        }
    }
    let mut write = rooms.write().await;
    // Double-check after acquiring write lock
    if let Some(tx) = write.get(room_id) {
        return tx.clone();
    }
    let (tx, _) = broadcast::channel(256);
    write.insert(room_id.to_string(), tx.clone());
    tx
}
