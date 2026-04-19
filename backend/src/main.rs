use std::sync::Arc;

use clap::Parser;
use dotenv::dotenv;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use fantasy_hockey::config::Config;
use fantasy_hockey::infra::jobs::historical_seed::seed_historical_skaters_if_empty;
use fantasy_hockey::infra::jobs::playoff_ingest::{
    ingest_playoff_games_for_range, is_playoff_skater_game_stats_empty,
};
use fantasy_hockey::infra::jobs::scheduler;
use fantasy_hockey::infra::jobs::scheduler::{init_rankings_scheduler, populate_historical_rankings};
use fantasy_hockey::domain::ports::prediction::PredictionService;
use fantasy_hockey::infra::jobs::{live_poller, meta_poller};
use fantasy_hockey::infra::prediction::claude::{ClaudeNarrator, NullNarrator};
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
    nhl_client.start_cache_cleanup(fantasy_hockey::tuning::nhl_client::CACHE_CLEANUP_INTERVAL);
    let db = FantasyDb::new(&config.database_url).await?;

    // Apply any pending database migrations on every boot. `sqlx::migrate!`
    // embeds the `.sql` files under supabase/migrations into the binary at
    // compile time and tracks what's been run via `_sqlx_migrations`.
    //
    // The Supabase CLI has its own migration tracker
    // (`supabase_migrations.schema_migrations`), but the two coexist fine
    // because they use disjoint tracking tables. On first boot after this
    // change, sqlx will re-"apply" every migration — every one of ours
    // uses `CREATE ... IF NOT EXISTS` / `DO $$` guards, so the operations
    // against an already-migrated prod DB are no-ops.
    sqlx::migrate!("./supabase/migrations")
        .run(db.pool())
        .await
        .map_err(|e| anyhow::anyhow!("database migration failed: {}", e))?;
    info!("Database migrations up to date");

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Initialize the rankings scheduler
    init_rankings_scheduler(Arc::new(db.clone()), Arc::new(nhl_client.clone())).await?;

    // Seed historical playoff skater totals if the table is empty. Runs
    // in the background so startup latency isn't affected; idempotent.
    {
        let db_bg = db.clone();
        tokio::spawn(async move {
            if let Err(e) = seed_historical_skaters_if_empty(&db_bg).await {
                tracing::error!("historical-skaters seed failed: {}", e);
            }
        });
    }

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

        // Backfill playoff skater game stats from playoff start through
        // today if the table is empty. Idempotent (UPSERT on conflict),
        // so a rerun is cheap but the emptiness gate avoids hitting the
        // NHL API unnecessarily on every deploy.
        let empty = is_playoff_skater_game_stats_empty(&db)
            .await
            .unwrap_or(true);
        if empty {
            let db_bg = db.clone();
            let nhl_bg = Arc::new(nhl_client.clone());
            let start = api::playoff_start().to_string();
            let end = today.as_str().min(api::season_end()).to_string();
            tokio::spawn(async move {
                info!("Background: backfilling playoff skater stats from {} to {}", start, end);
                match ingest_playoff_games_for_range(&db_bg, &nhl_bg, &start, &end).await {
                    Ok(rows) => info!(
                        rows,
                        "Background: playoff skater stats backfill complete"
                    ),
                    Err(e) => tracing::error!(
                        "Background: playoff skater stats backfill failed: {}",
                        e
                    ),
                }
            });
        }
    }

    // --------------------------------------------------------------
    // NHL mirror pollers
    // --------------------------------------------------------------
    //
    // Two background tasks continuously mirror the NHL API into
    // `nhl_*` Postgres tables so user-facing handlers only ever read
    // from Postgres. The pollers coordinate across replicas via
    // Postgres advisory locks (see `infra/db/nhl_mirror.rs`), so a
    // multi-replica deployment only runs the work on one replica per
    // tick.
    let poller_cancel = CancellationToken::new();
    {
        let db_meta = db.clone();
        let nhl_meta = Arc::new(nhl_client.clone());
        let cancel = poller_cancel.clone();
        tokio::spawn(async move {
            meta_poller::run(db_meta, nhl_meta, cancel).await;
        });
    }
    {
        let db_live = db.clone();
        let nhl_live = Arc::new(nhl_client.clone());
        let cancel = poller_cancel.clone();
        tokio::spawn(async move {
            live_poller::run(db_live, nhl_live, cancel).await;
        });
    }

    // --------------------------------------------------------------
    // Cold-start auto-seed
    // --------------------------------------------------------------
    //
    // The mirror tables exist as soon as migrations run, but they
    // start empty. The pollers populate `nhl_games` (today's
    // schedule) and `nhl_player_game_stats` (live games only) as
    // they tick — but they never re-fetch boxscores for games that
    // finalized BEFORE the live poller first saw them LIVE. After
    // a fresh deploy mid-day, that means every already-final game
    // has no row in `nhl_player_game_stats`, so handlers that sum
    // from it (Rankings, Race Odds Current) read as zeros.
    //
    // Detect that case once at boot — if the per-game stats table
    // is empty, fire a one-shot rehydrate to seed it. After the
    // first successful run the table has rows and this short-
    // circuits forever; manual `/api/admin/rehydrate` is still
    // available for explicit re-seeds.
    {
        let db_seed = db.clone();
        let nhl_seed = Arc::new(nhl_client.clone());
        tokio::spawn(async move {
            // Wait long enough for meta_poller's first tick to land
            // today's schedule into nhl_games — rehydrate iterates
            // those rows for boxscores. Stagger is 15 s; +30 s gives
            // the schedule fetch + upsert time to complete.
            tokio::time::sleep(std::time::Duration::from_secs(45)).await;
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM nhl_player_game_stats",
            )
            .fetch_one(db_seed.pool())
            .await
            .unwrap_or(0);
            if count > 0 {
                tracing::debug!(
                    rows = count,
                    "auto-seed: nhl_player_game_stats has data, skipping rehydrate"
                );
                return;
            }
            info!("auto-seed: nhl_player_game_stats is empty; running rehydrate to seed mirror");
            let summary =
                fantasy_hockey::infra::jobs::rehydrate::run(&db_seed, nhl_seed).await;
            info!(
                games = summary.games_upserted,
                player_rows = summary.boxscore_player_rows,
                landings = summary.landing_captures,
                errors = summary.errors.len(),
                "auto-seed: rehydrate complete"
            );
        });
    }

    // Compose the prediction adapter. In production with
    // `ANTHROPIC_API_KEY` set we use the Claude narrator; without a
    // key we wire a null adapter so handlers that optionally
    // include a narrative degrade to "no narrative" rather than
    // failing the whole server boot.
    let prediction: Arc<dyn PredictionService> = match ClaudeNarrator::from_env() {
        Some(n) => {
            info!("Prediction adapter: ClaudeNarrator");
            Arc::new(n)
        }
        None => {
            info!("Prediction adapter: NullNarrator (ANTHROPIC_API_KEY unset)");
            Arc::new(NullNarrator)
        }
    };

    // Run the API server. When the server's graceful shutdown fires
    // we cancel the pollers so they stop cleanly on SIGTERM.
    info!("Starting web server on port {}", config.port);
    let result = api::run_server(db, nhl_client, config, prediction).await;
    poller_cancel.cancel();
    result
}
