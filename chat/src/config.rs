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

    /// Cloudflare Turnstile server-side secret. Empty string skips validation
    /// (local dev). Get yours at dash.cloudflare.com → Turnstile.
    pub turnstile_secret: String,

    /// Resend API key for sending verification emails. Empty string skips
    /// sending and logs the verify URL instead (local dev).
    pub resend_api_key: String,

    /// From-address used in outgoing emails, e.g. `noreply@gihc.online`.
    pub resend_from: String,

    /// The backend's own public URL, used to build email verification links.
    /// E.g. `https://api.gihc.online`.
    pub base_url: String,

    /// The frontend's public URL, used as redirect target after email
    /// verification. E.g. `https://app.gihc.online`.
    pub frontend_url: String,

    /// Directory where uploaded files are stored on disk. Defaults to
    /// `/data/uploads` (mapped as a Docker volume in production).
    pub upload_dir: String,
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
            turnstile_secret: std::env::var("TURNSTILE_SECRET").unwrap_or_default(),
            resend_api_key: std::env::var("RESEND_API_KEY").unwrap_or_default(),
            resend_from: std::env::var("RESEND_FROM")
                .unwrap_or_else(|_| "noreply@example.com".into()),
            base_url: std::env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8001".into()),
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:8001".into()),
            upload_dir: std::env::var("UPLOAD_DIR")
                .unwrap_or_else(|_| "/data/uploads".into()),
        }
    }
}
