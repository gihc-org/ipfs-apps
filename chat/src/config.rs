#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expire_hours: i64,
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
