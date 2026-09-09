use chat::{build_app, cleanup_expired_lofts, AppState, Config};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    let db = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run migrations");

    let state = AppState {
        db,
        config,
        loft_channels: Arc::new(RwLock::new(HashMap::new())),
        loft_registry: Arc::new(RwLock::new(HashMap::new())),
    };

    // TTL-oprydning: hver time slettes lofts der ikke har været aktive inden
    // for LOFT_TTL_HOURS (default 168). Kun i binæren — ikke i tests.
    if state.config.loft_ttl_hours > 0 {
        let cleanup_state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                cleanup_expired_lofts(&cleanup_state).await;
            }
        });
    }

    let app = build_app(state);
    let addr = "0.0.0.0:8080";
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
