use std::sync::Arc;

use clap::Parser;
use dotenv::dotenv;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use fantasy_hockey::config::Config;
use fantasy_hockey::utils::scheduler;
use fantasy_hockey::utils::scheduler::{init_rankings_scheduler, populate_historical_rankings};
use fantasy_hockey::FantasyDb;
use fantasy_hockey::{api, NhlClient};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct App {
    /// Run the web server (for backward compatibility)
    #[arg(default_value = "serve")]
    command: String,

    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Load all configuration eagerly — panics immediately if anything required is missing
    let config = Config::from_env();

    // Initialize tracing with env-filter support (RUST_LOG=debug, etc.)
    if config.log_json {
        fmt::Subscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .init();
    } else {
        fmt::Subscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    // Parse command line arguments
    let args = App::parse();

    info!("Starting fantasy hockey application");

    // Initialize season config from typed Config (populates OnceLock accessors)
    api::init_season_config(&config);
    info!("Season config: {} game_type={} playoffs={} end={}",
        api::season(), api::game_type(), api::playoff_start(), api::season_end());

    // Override port from CLI arg if provided
    let config = if let Some(port) = args.port {
        Config { port, ..config }
    } else {
        config
    };
    let config = Arc::new(config);

    // Initialize services
    let nhl_client = NhlClient::new();
    nhl_client.start_cache_cleanup(std::time::Duration::from_secs(300));
    let db = FantasyDb::new(&config.database_url).await?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Initialize the rankings scheduler
    init_rankings_scheduler(Arc::new(db.clone()), Arc::new(nhl_client.clone())).await?;

    // Run backfill in background (non-blocking) so the server starts immediately
    if today.as_str() >= api::playoff_start() {
        if scheduler::is_rankings_table_empty(&db).await? {
            let db_bg = db.clone();
            let nhl_bg = nhl_client.clone();
            let start = api::playoff_start().to_string();
            let end = today.as_str().min(api::season_end()).to_string();
            tokio::spawn(async move {
                info!("Background: populating historical rankings from {} to {}", start, end);
                if let Err(e) = populate_historical_rankings(&db_bg, &nhl_bg, &start, &end).await {
                    tracing::error!("Background backfill failed: {}", e);
                } else {
                    info!("Background: historical rankings populated successfully");
                }
            });
        }
    }

    // Run the API server
    info!("Starting web server on port {}", config.port);
    api::run_server(db, nhl_client, config).await
}
