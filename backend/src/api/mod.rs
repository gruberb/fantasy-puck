use std::net::SocketAddr;

use axum::http::{header, Method};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::nhl_api::nhl::NhlClient;
use crate::FantasyDb;

pub mod dtos;
pub mod handlers;
pub mod response;
pub mod routes;

use std::sync::OnceLock;

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

static SEASON_CELL: OnceLock<u32> = OnceLock::new();
static GAME_TYPE_CELL: OnceLock<u8> = OnceLock::new();
static PLAYOFF_START_CELL: OnceLock<String> = OnceLock::new();
static SEASON_END_CELL: OnceLock<String> = OnceLock::new();

/// Call once at startup (after dotenv) to read season config from env vars.
pub fn init_season_config() {
    SEASON_CELL.get_or_init(|| env_or("NHL_SEASON", 20252026u32));
    GAME_TYPE_CELL.get_or_init(|| env_or("NHL_GAME_TYPE", 3u8));
    PLAYOFF_START_CELL.get_or_init(|| std::env::var("NHL_PLAYOFF_START").unwrap_or_else(|_| "2026-04-18".into()));
    SEASON_END_CELL.get_or_init(|| std::env::var("NHL_SEASON_END").unwrap_or_else(|_| "2026-06-15".into()));
}

pub fn season() -> u32 { *SEASON_CELL.get_or_init(|| env_or("NHL_SEASON", 20252026u32)) }
pub fn game_type() -> u8 { *GAME_TYPE_CELL.get_or_init(|| env_or("NHL_GAME_TYPE", 3u8)) }
pub fn playoff_start() -> &'static str { PLAYOFF_START_CELL.get_or_init(|| std::env::var("NHL_PLAYOFF_START").unwrap_or_else(|_| "2026-04-18".into())) }
pub fn season_end() -> &'static str { SEASON_END_CELL.get_or_init(|| std::env::var("NHL_SEASON_END").unwrap_or_else(|_| "2026-06-15".into())) }

pub async fn run_server(
    db: FantasyDb,
    nhl_client: NhlClient,
    jwt_secret: String,
    port: u16,
) -> anyhow::Result<()> {
    // Create CORS middleware
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_origin(Any);

    // Build our application with routes
    let app = routes::create_router(db, nhl_client, jwt_secret).layer(cors);

    // Create a TCP listener for our address
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Starting server on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
