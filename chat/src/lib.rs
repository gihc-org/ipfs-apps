pub mod auth;
pub mod captcha;
pub mod config;
pub mod email;
pub mod models;
pub mod routes;
pub mod ws;

use axum::{
    http::{HeaderValue, Method, Request},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer,
};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

pub use config::Config;
pub use ws::RoomMap;

/// Tæller aktive WS-forbindelser per bruger (til at håndtere flere tabs).
/// Brugeren er "online" så længe tælleren er > 0.
pub type OnlineUsers = Arc<RwLock<HashMap<Uuid, u32>>>;

/// Personlig broadcast-kanal per bruger til direkte signal-routing.
/// Gør det muligt at sende WebRTC-signaler til en bruger uanset hvilket rum
/// de er forbundet til — nødvendigt for fil-overførsel når parterne ikke er
/// i samme rum på samme tid.
pub type UserSenders = Arc<RwLock<HashMap<Uuid, tokio::sync::broadcast::Sender<String>>>>;

/// Shared state injected into every Axum handler via `State<AppState>`.
///
/// `db` is a connection pool — cloning it is cheap (Arc under the hood).
/// `http` is a shared reqwest client for external API calls (Turnstile, Resend).
/// `rooms` holds in-memory broadcast channels keyed by room UUID string;
/// restarting the server drops all active WebSocket connections.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub rooms: RoomMap,
    pub http: reqwest::Client,
    pub online_users: OnlineUsers,
    pub user_senders: UserSenders,
}

/// Extracts the first IP from `X-Forwarded-For` (injected by Caddy) so the rate
/// limiter sees the real client IP, not Caddy's container IP.
#[derive(Clone)]
struct ForwardedIpExtractor;

impl KeyExtractor for ForwardedIpExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .unwrap_or("0.0.0.0");
        Ok(ip.to_string())
    }
}

/// Builds the Axum router with all routes, CORS middleware, and shared state.
/// Used by both the binary entrypoint and integration tests.
pub fn build_app(state: AppState) -> Router {
    let cors = build_cors(&state.config.allowed_origin);

    // 3 req/s sustained, burst 30 — sustained rate stops brute force; burst covers
    // concurrent e2e test workers that all originate from the same IP.
    let auth_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(3)
            .burst_size(30)
            .key_extractor(ForwardedIpExtractor)
            .finish()
            .unwrap(),
    );
    let auth_routes = Router::new()
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/token", post(routes::auth::login))
        .layer(GovernorLayer { config: auth_rate_limit });

    let v1 = Router::new()
        .merge(auth_routes)
        .route("/auth/me", get(routes::auth::me).delete(routes::auth::delete_me))
        .route("/auth/verify", get(routes::auth::verify))
        .route("/rooms", get(routes::rooms::list).post(routes::rooms::create))
        .route("/rooms/:id/messages", get(routes::rooms::messages))
        .route("/users", get(routes::dms::list_users))
        .route("/dms", get(routes::dms::list_dms).post(routes::dms::create_or_get_dm))
        .route("/presence", get(routes::presence::list))
        .route("/files", post(routes::files::upload))
        .route("/files/:id", get(routes::files::download))
        .route("/ws/:room_id", get(routes::chat::handler));

    Router::new()
        .nest("/v1", v1)
        .layer(cors)
        .with_state(state)
}

/// Builds the CORS layer from an origin string.
///
/// `"*"` and a specific origin require different tower-http types (`Any` vs
/// `HeaderValue`), so they can't be unified into a single code path.
/// In production `ALLOWED_ORIGIN` is set to the DNSLink domain
/// (`https://app.gihc.online`) so the wildcard branch is only used locally.
pub fn build_cors(allowed_origin: &str) -> CorsLayer {
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
