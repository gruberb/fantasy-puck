use std::sync::Arc;

use clap::Parser;
use dotenv::dotenv;
use tracing::info;

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
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables: prefer .env.development for local dev, fall back to .env
    if dotenv::from_filename(".env.development").is_err() {
        dotenv().ok();
    }

    // Initialize tracing - only once
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = App::parse();

    info!("Starting fantasy hockey application");

    // Initialize season config from env vars (NHL_SEASON, NHL_GAME_TYPE, etc.)
    api::init_season_config();
    info!("Season config: {} game_type={} playoffs={} end={}",
        api::season(), api::game_type(), api::playoff_start(), api::season_end());

    // Initialize services
    let nhl_client = NhlClient::new();
    nhl_client.start_cache_cleanup(std::time::Duration::from_secs(300));
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment");
    let db = FantasyDb::new(&database_url).await?;

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
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET must be set in environment");
    info!("Starting web server on port {}", args.port);
    api::run_server(db, nhl_client, jwt_secret, args.port).await
}
