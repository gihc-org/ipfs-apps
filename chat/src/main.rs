mod auth;
mod config;
mod models;
mod routes;
mod ws;

use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use config::Config;
use ws::RoomMap;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub rooms: RoomMap,
}

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

    let rooms: RoomMap = std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let cors = build_cors(&config.allowed_origin);

    let state = AppState { db, config, rooms };

    let app = Router::new()
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/token", post(routes::auth::login))
        .route("/auth/me", get(routes::auth::me))
        .route("/rooms", get(routes::rooms::list).post(routes::rooms::create))
        .route("/rooms/:id/messages", get(routes::rooms::messages))
        .route("/ws/:room_id", get(routes::chat::handler))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:8001";
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn build_cors(allowed_origin: &str) -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::PUT, Method::DELETE];
    let headers = [
        axum::http::header::AUTHORIZATION,
        axum::http::header::CONTENT_TYPE,
    ];

    if allowed_origin == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(methods)
            .allow_headers(headers)
    } else {
        CorsLayer::new()
            .allow_origin(
                allowed_origin
                    .parse::<HeaderValue>()
                    .expect("Invalid ALLOWED_ORIGIN"),
            )
            .allow_methods(methods)
            .allow_headers(headers)
    }
}
