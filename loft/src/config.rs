//! Environment-based configuration loaded once at startup.

/// All runtime configuration read from environment variables.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,

    /// `"*"` allows all origins (local dev). Any other value is used verbatim
    /// as the `Access-Control-Allow-Origin` header value.
    pub allowed_origin: String,

    /// Lofts slettes når `last_active` er ældre end dette antal timer.
    /// Default 168 (= 7 dage). 0 eller negativ deaktiverer oprydning.
    pub loft_ttl_hours: i64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL required"),
            allowed_origin: std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "*".into()),
            loft_ttl_hours: std::env::var("LOFT_TTL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(168),
        }
    }
}
