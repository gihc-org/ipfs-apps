pub mod config;
pub mod models;
pub mod routes;
pub mod state;

use axum::{
    http::{HeaderValue, Method, Request},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer,
};
use tower_http::cors::{Any, CorsLayer};

pub use config::Config;
pub use state::{cleanup_expired_lofts, LoftChannelMap, LoftRegistry};

/// Delt tilstand for alle handlers. `db` er en billig klonbar connection pool;
/// loft-kanaler og deltager-registry lever i hukommelsen → præcis 1 replica.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub loft_channels: LoftChannelMap,
    pub loft_registry: LoftRegistry,
}

/// Udtrækker første IP fra `X-Forwarded-For` (sat af ingress-nginx) så
/// rate limiteren ser klientens IP, ikke proxyens.
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

/// Bygger Axum-routeren med alle routes, CORS og delt state.
/// Bruges af både binær-entrypoint og integrationstests.
pub fn build_app(state: AppState) -> Router {
    let cors = build_cors(&state.config.allowed_origin);

    // Loft-oprettelse er den eneste åbne write-endpoint — rate limit for at
    // forhindre link-spam. Burst dækker konkurrerende e2e-workers.
    let loft_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(10)
            .key_extractor(ForwardedIpExtractor)
            .finish()
            .unwrap(),
    );
    let loft_routes = Router::new()
        .route("/lofts", post(routes::lofts::create))
        .layer(GovernorLayer {
            config: loft_rate_limit,
        });

    let v1 = Router::new()
        .merge(loft_routes)
        .route(
            "/lofts/:id",
            get(routes::lofts::get).delete(routes::lofts::close),
        )
        .route("/ws/:loft_id", get(routes::lofts::ws_handler));

    Router::new()
        .nest("/v1", v1)
        .route("/healthz", get(routes::health::healthz))
        .layer(cors)
        .with_state(state)
}

/// Bygger CORS-laget fra en origin-streng.
///
/// `"*"` og en specifik origin kræver forskellige tower-http-typer. I k8s er
/// frontend og API samme origin, så ALLOWED_ORIGIN bruges kun til lokal udvikling.
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
