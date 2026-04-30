//! Environment-based configuration loaded once at startup.

/// All runtime configuration read from environment variables.
///
/// `allowed_origin` is `"*"` during local development and the DNSLink domain
/// (`https://app.gihc.online`) in production. Setting it to a specific origin
/// locks CORS down to that single host, which is required when the frontend
/// is hosted via IPFS with a stable domain.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    /// Defaults to 24 if `JWT_EXPIRE_HOURS` is unset or not a valid integer.
    pub jwt_expire_hours: i64,
    /// `"*"` allows all origins (local dev). Any other value is used verbatim
    /// as the `Access-Control-Allow-Origin` header value.
    pub allowed_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL required"),
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET required"),
            jwt_expire_hours: std::env::var("JWT_EXPIRE_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            allowed_origin: std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "*".into()),
        }
    }
}
