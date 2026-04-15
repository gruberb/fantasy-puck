use secrecy::SecretString;

/// Application configuration, loaded eagerly at startup.
/// If any required variable is missing the process panics immediately
/// with a clear message — no latent config bugs at runtime.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: SecretString,
    pub anthropic_api_key: Option<SecretString>,
    pub log_json: bool,

    // NHL season settings
    pub nhl_season: u32,
    pub nhl_game_type: u8,
    pub nhl_playoff_start: String,
    pub nhl_season_end: String,

    // Server
    pub port: u16,
    pub cors_origins: Vec<String>,
}

impl Config {
    /// Load configuration from environment variables.
    /// Panics with a descriptive message if a required variable is missing.
    pub fn from_env() -> Self {
        Self {
            database_url: required("DATABASE_URL"),
            jwt_secret: SecretString::from(required("JWT_SECRET")),
            anthropic_api_key: optional("ANTHROPIC_API_KEY").map(SecretString::from),
            log_json: optional("LOG_JSON").map(|v| v == "true").unwrap_or(false),

            nhl_season: optional_parsed("NHL_SEASON", 20252026u32),
            nhl_game_type: optional_parsed("NHL_GAME_TYPE", 3u8),
            nhl_playoff_start: optional("NHL_PLAYOFF_START")
                .unwrap_or_else(|| "2026-04-18".into()),
            nhl_season_end: optional("NHL_SEASON_END")
                .unwrap_or_else(|| "2026-06-15".into()),

            port: optional_parsed("PORT", 3000),
            cors_origins: optional("CORS_ORIGINS")
                .map(|s| s.split(',').map(|o| o.trim().to_string()).collect())
                .unwrap_or_default(),
        }
    }
}

fn required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set in environment"))
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn optional_parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
