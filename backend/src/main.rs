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
    // Load environment variables from .env file if present
    dotenv().ok();

    // Initialize tracing - only once
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = App::parse();

    info!("Starting fantasy hockey application");

    // Initialize services
    let nhl_client = NhlClient::new();
    nhl_client.start_cache_cleanup(std::time::Duration::from_secs(300));
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment");
    let db = FantasyDb::new(&database_url).await?;

    // Check if daily rankings need to be populated
    // Only backfill if the season has started and there's historical data to process
    let playoff_start = "2026-04-18";
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    if today.as_str() >= playoff_start {
        if scheduler::is_rankings_table_empty(&db).await? {
            info!("Daily rankings table is empty - populating historical data...");
            let end_date = today.as_str().min("2026-06-15");
            info!(
                "Populating historical rankings from {} to {}",
                playoff_start, end_date
            );
            populate_historical_rankings(&db, &nhl_client, playoff_start, end_date).await?;
        } else {
            info!("Daily rankings already populated - skipping initialization");
        }
    } else {
        info!(
            "Playoffs haven't started yet (starts {}). Skipping rankings population.",
            playoff_start
        );
    }

    // Initialize the rankings scheduler
    init_rankings_scheduler(Arc::new(db.clone()), Arc::new(nhl_client.clone())).await?;

    // Run the API server
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET must be set in environment");
    info!("Starting web server on port {}", args.port);
    api::run_server(db, nhl_client, jwt_secret, args.port).await
}
