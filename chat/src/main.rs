use chat::{AppState, Config, RoomMap, build_app};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();

    let db = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    // Migrations are embedded at compile time via sqlx::migrate! and run on
    // every startup — safe to repeat because each migration is idempotent.
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run migrations");

    let rooms: RoomMap =
        std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let state = AppState { db, config, rooms };
    let app = build_app(state);

    let addr = "0.0.0.0:8001";
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
