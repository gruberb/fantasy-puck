use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{header, Method};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tracing::info;

use crate::config::Config;
use crate::nhl_api::nhl::NhlClient;
use crate::FantasyDb;

pub mod dtos;
pub mod handlers;
pub mod response;
pub mod routes;

use std::sync::OnceLock;

// Season config stored in OnceLock for zero-cost access from handlers.
// Initialized once from the Config struct at startup.
static SEASON_CELL: OnceLock<u32> = OnceLock::new();
static GAME_TYPE_CELL: OnceLock<u8> = OnceLock::new();
static PLAYOFF_START_CELL: OnceLock<String> = OnceLock::new();
static SEASON_END_CELL: OnceLock<String> = OnceLock::new();

/// Initialize season config from the typed Config struct.
pub fn init_season_config(config: &Config) {
    SEASON_CELL.get_or_init(|| config.nhl_season);
    GAME_TYPE_CELL.get_or_init(|| config.nhl_game_type);
    PLAYOFF_START_CELL.get_or_init(|| config.nhl_playoff_start.clone());
    SEASON_END_CELL.get_or_init(|| config.nhl_season_end.clone());
}

pub fn season() -> u32 { *SEASON_CELL.get().expect("season config not initialized") }
pub fn game_type() -> u8 { *GAME_TYPE_CELL.get().expect("game_type config not initialized") }
pub fn playoff_start() -> &'static str { PLAYOFF_START_CELL.get().expect("playoff_start config not initialized") }
pub fn season_end() -> &'static str { SEASON_END_CELL.get().expect("season_end config not initialized") }

pub async fn run_server(
    db: FantasyDb,
    nhl_client: NhlClient,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let port = config.port;

    // Create CORS middleware — use explicit origins in production, any in development
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let cors = if config.cors_origins.is_empty() {
        cors.allow_origin(Any)
    } else {
        let origins: Vec<_> = config.cors_origins.iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors.allow_origin(AllowOrigin::list(origins))
    };

    // Build our application with routes and middleware stack.
    // Layers wrap in reverse order: the last .layer() is the outermost.
    let app = routes::create_router(db, nhl_client, config)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1 MB
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(CompressionLayer::new());

    // Create a TCP listener for our address
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Starting server on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, starting graceful shutdown"),
        _ = terminate => info!("Received SIGTERM, starting graceful shutdown"),
    }
}
