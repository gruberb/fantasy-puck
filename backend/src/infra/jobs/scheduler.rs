use std::sync::Arc;

use chrono::{Duration, NaiveDate, Utc};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

use crate::api::handlers::insights::generate_and_cache_insights;
use crate::api::handlers::race_odds::generate_and_cache_race_odds;
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::error::{Error, Result};
use crate::tuning::scheduler as tuning;
use crate::infra::jobs::player_pool::refresh_playoff_roster_cache;
use crate::infra::jobs::playoff_ingest::ingest_playoff_games_for_date;
use crate::ws::draft_hub::DraftHub;
use crate::NhlClient;

/// Process and store daily rankings for a specific date and league
pub async fn process_daily_rankings(
    db: &FantasyDb,
    _nhl_client: &NhlClient,
    date: &str,
    league_id: &str,
) -> Result<()> {
    info!(
        "Processing daily rankings for date: {}, league: {}",
        date, league_id
    );
    let pool = db.pool();

    // Safety gate: if any game on this date is still LIVE/CRIT/PRE
    // in the mirror, the daily total is still moving and we don't
    // want to snapshot it. The 9am / 3pm UTC crons run against
    // *yesterday*, so this gate is usually a no-op; it matters for
    // a manual /api/admin/process-rankings call fired mid-afternoon
    // against today.
    let still_live = nhl_mirror::list_live_game_ids_for_date(pool, date).await?;
    if !still_live.is_empty() {
        info!(
            date = %date,
            live = still_live.len(),
            "process_daily_rankings: games still in progress, skipping"
        );
        return Ok(());
    }

    // Read finalised per-team totals from the view. One query.
    let rows: Vec<(i64, i32, i32, i32)> = sqlx::query_as(
        r#"
        SELECT team_id, goals::int, assists::int, points::int
          FROM v_daily_fantasy_totals
         WHERE league_id = $1::uuid
           AND date       = $2::date
           AND points > 0
         ORDER BY points DESC, team_id
        "#,
    )
    .bind(league_id)
    .bind(date)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        info!(date = %date, "process_daily_rankings: no scoring in league today");
        return Ok(());
    }

    for (rank, (team_id, goals, assists, points)) in rows.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO daily_rankings (date, team_id, league_id, rank, points, goals, assists)
            VALUES ($1, $2, $3::uuid, $4, $5, $6, $7)
            ON CONFLICT (team_id, date, league_id) DO UPDATE SET
                rank = EXCLUDED.rank,
                points = EXCLUDED.points,
                goals = EXCLUDED.goals,
                assists = EXCLUDED.assists
            "#,
        )
        .bind(date)
        .bind(team_id)
        .bind(league_id)
        .bind(rank as i64 + 1)
        .bind(points)
        .bind(goals)
        .bind(assists)
        .execute(pool)
        .await?;
    }

    info!(
        date = %date,
        league = %league_id,
        rows = rows.len(),
        "process_daily_rankings: snapshot written from v_daily_fantasy_totals"
    );
    Ok(())
}

/// Process daily rankings for all leagues
async fn process_daily_rankings_all_leagues(
    db: &FantasyDb,
    nhl_client: &NhlClient,
    date: &str,
) {
    match db.get_all_league_ids().await {
        Ok(league_ids) => {
            for league_id in &league_ids {
                if let Err(e) = process_daily_rankings(db, nhl_client, date, league_id).await {
                    error!(
                        "Failed to process rankings for league {} on {}: {}",
                        league_id, date, e
                    );
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch league IDs: {}", e);
        }
    }
}

/// Ingest completed playoff box scores from yesterday into
/// `playoff_skater_game_stats`. Runs before the prewarm step so the
/// downstream player-projection model sees fresh data.
async fn ingest_yesterdays_playoff_games(db: &FantasyDb, nhl_client: &NhlClient) {
    let yesterday = match Utc::now().checked_sub_signed(Duration::days(1)) {
        Some(t) => t.naive_utc().format("%Y-%m-%d").to_string(),
        None => {
            error!("Failed to compute yesterday's date for playoff ingest");
            return;
        }
    };
    let nhl_arc = Arc::new(nhl_client.clone());
    match ingest_playoff_games_for_date(db, &nhl_arc, &yesterday).await {
        Ok(rows) => info!(
            date = %yesterday,
            rows,
            "Playoff ingest: yesterday's skater stats upserted"
        ),
        Err(e) => error!(
            date = %yesterday,
            error = %e,
            "Playoff ingest for yesterday failed"
        ),
    }
}

/// Pre-generate insights and race-odds for all leagues so they're cached
/// when users visit. Runs once per day from the 10am-UTC scheduler job
/// and on-demand via `GET /api/admin/prewarm` (usually after a cache
/// invalidation or a model-version bump that emptied the cache).
pub async fn prewarm_derived_payloads(db: &FantasyDb, nhl_client: &NhlClient) {
    // Prewarm builds its own AppState so it can call the same
    // handler entry points a real HTTP request would. The
    // prediction adapter gets rebuilt here because the scheduler
    // doesn't receive one from outside — this mirrors the main.rs
    // composition root.
    let prediction: Arc<dyn crate::domain::ports::prediction::PredictionService> =
        match crate::infra::prediction::claude::ClaudeNarrator::from_env() {
            Some(n) => Arc::new(n),
            None => Arc::new(crate::infra::prediction::claude::NullNarrator),
        };
    let state = Arc::new(AppState {
        db: db.clone(),
        nhl_client: nhl_client.clone(),
        config: Arc::new(crate::config::Config::from_env()),
        draft_hub: DraftHub::new(),
        prediction,
    });

    // Playoff roster pool — 16 team rosters written into Postgres so
    // every downstream cold read is one SELECT instead of a paced NHL
    // fan-out. Failures are logged but non-fatal; the cached fetch path
    // falls back to the NHL fan-out on first read.
    if game_type() == 3 {
        match refresh_playoff_roster_cache(db, nhl_client, season(), game_type()).await {
            Ok(n) => info!("Playoff roster cache ready ({} players)", n),
            Err(e) => error!("Failed to pre-warm playoff roster cache: {}", e),
        }
    }

    // Global (no-league) payloads.
    match generate_and_cache_insights(&state, "").await {
        Ok(_) => info!("Pre-warmed global insights"),
        Err(e) => error!("Failed to pre-warm global insights: {}", e),
    }
    match generate_and_cache_race_odds(&state, "", None).await {
        Ok(_) => info!("Pre-warmed global race-odds (Fantasy Champion)"),
        Err(e) => error!("Failed to pre-warm global race-odds: {}", e),
    }

    // Per-league payloads.
    let league_ids = match db.get_all_league_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            error!("Failed to fetch league IDs for pre-warming: {}", e);
            return;
        }
    };
    for league_id in &league_ids {
        match generate_and_cache_insights(&state, league_id).await {
            Ok(_) => info!("Pre-warmed insights for league {}", league_id),
            Err(e) => error!(
                "Failed to pre-warm insights for league {}: {}",
                league_id, e
            ),
        }
        match generate_and_cache_race_odds(&state, league_id, None).await {
            Ok(_) => info!("Pre-warmed race-odds for league {}", league_id),
            Err(e) => error!(
                "Failed to pre-warm race-odds for league {}: {}",
                league_id, e
            ),
        }
        // Per-team Pulse diagnosis. Runs after race-odds so
        // `compose_team_breakdown` reads a warm `race_odds:v4:*`
        // cache when it builds remaining-points figures. Order
        // matters; don't invert these two calls.
        if game_type() == 3 {
            prewarm_league_team_diagnoses(&state, league_id).await;
        }
    }
}

async fn prewarm_league_team_diagnoses(state: &Arc<AppState>, league_id: &str) {
    let teams = match state.db.get_all_teams(league_id).await {
        Ok(ts) => ts,
        Err(e) => {
            error!("Failed to list teams for diagnosis prewarm ({}): {}", league_id, e);
            return;
        }
    };
    for team in teams {
        let players = match state.db.get_team_players(team.id).await {
            Ok(p) => p,
            Err(e) => {
                error!(
                    "Failed to load players for diagnosis prewarm (team {}): {}",
                    team.id, e
                );
                continue;
            }
        };
        match crate::api::handlers::team_breakdown::compose_team_breakdown(
            state,
            league_id,
            team.id,
            &team.name,
            &players,
        )
        .await
        {
            Ok(_) => info!(
                "Pre-warmed team_diagnosis for league {} team {}",
                league_id, team.id
            ),
            Err(e) => error!(
                "Failed to pre-warm team_diagnosis for league {} team {}: {}",
                league_id, team.id, e
            ),
        }
    }
}

/// Initialize the rankings scheduler
pub async fn init_rankings_scheduler(
    db: Arc<FantasyDb>,
    nhl_client: Arc<NhlClient>,
) -> Result<JobScheduler> {
    // Create a new scheduler
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| Error::Internal(format!("Failed to create job scheduler: {}", e)))?;

    let db_clone_morning = db.clone();
    let nhl_client_clone_morning = nhl_client.clone();
    let db_clone_afternoon = db.clone();
    let nhl_client_clone_afternoon = nhl_client.clone();
    let db_clone_insights = db.clone();
    let nhl_client_clone_insights = nhl_client.clone();
    let db_clone_edge = db.clone();
    let nhl_client_clone_edge = nhl_client.clone();

    // Schedule job for 9am UTC
    let morning_job = Job::new_async(tuning::MORNING_RANKINGS_CRON, move |_, _| {
        let db = db_clone_morning.clone();
        let nhl_client = nhl_client_clone_morning.clone();
        Box::pin(async move {
            // Calculate yesterday's date
            let yesterday = Utc::now()
                .checked_sub_signed(Duration::days(1))
                .unwrap()
                .naive_utc()
                .format("%Y-%m-%d")
                .to_string();

            process_daily_rankings_all_leagues(&db, &nhl_client, &yesterday).await;

            // Prune cache rows older than `tuning::CACHE_RETENTION`.
            let retention_days = tuning::CACHE_RETENTION.as_secs() as i64 / 86_400;
            let week_ago = (Utc::now() - Duration::days(retention_days))
                .format("%Y-%m-%d")
                .to_string();
            if let Err(e) = sqlx::query("DELETE FROM response_cache WHERE date IS NOT NULL AND date < $1")
                .bind(&week_ago)
                .execute(db.pool())
                .await
            {
                error!("Failed to clean up old cache entries: {}", e);
            } else {
                info!("Cleaned up response_cache entries older than {}", week_ago);
            }
        })
    })
    .map_err(|e| Error::Internal(format!("Failed to create morning job: {}", e)))?;

    // Schedule job for 3pm UTC
    let afternoon_job = Job::new_async(tuning::AFTERNOON_RANKINGS_CRON, move |_, _| {
        let db = db_clone_afternoon.clone();
        let nhl_client = nhl_client_clone_afternoon.clone();
        Box::pin(async move {
            // Calculate yesterday's date
            let yesterday = Utc::now()
                .checked_sub_signed(Duration::days(1))
                .unwrap()
                .naive_utc()
                .format("%Y-%m-%d")
                .to_string();

            process_daily_rankings_all_leagues(&db, &nhl_client, &yesterday).await;
        })
    })
    .map_err(|e| Error::Internal(format!("Failed to create afternoon job: {}", e)))?;

    // Schedule derived-payload pre-warming at 10am UTC daily. Ingest
    // yesterday's completed playoff box scores first so the downstream
    // race-odds prewarm reads fresh player facts.
    let insights_job = Job::new_async(tuning::DAILY_PREWARM_CRON, move |_, _| {
        let db = db_clone_insights.clone();
        let nhl_client = nhl_client_clone_insights.clone();
        Box::pin(async move {
            info!("Running daily pre-warming job (playoff ingest + insights + race-odds)");
            ingest_yesterdays_playoff_games(&db, &nhl_client).await;
            prewarm_derived_payloads(&db, &nhl_client).await;
        })
    })
    .map_err(|e| Error::Internal(format!("Failed to create pre-warming job: {}", e)))?;

    // Schedule the nightly NHL Edge refresh at 09:30 UTC. Runs 30 min
    // ahead of the daily prewarm so the insights pre-warm reads fresh
    // top-speed / top-shot-speed telemetry from the mirror.
    let edge_job = Job::new_async(tuning::EDGE_REFRESH_CRON, move |_, _| {
        let db = db_clone_edge.clone();
        let nhl_client = nhl_client_clone_edge.clone();
        Box::pin(async move {
            info!("Running nightly NHL Edge refresh");
            let _ = crate::infra::jobs::edge_refresher::run(&db, nhl_client, false).await;
        })
    })
    .map_err(|e| Error::Internal(format!("Failed to create edge refresh job: {}", e)))?;

    // Add jobs to the scheduler
    scheduler
        .add(morning_job)
        .await
        .map_err(|e| Error::Internal(format!("Failed to add morning job: {}", e)))?;

    scheduler
        .add(afternoon_job)
        .await
        .map_err(|e| Error::Internal(format!("Failed to add afternoon job: {}", e)))?;

    scheduler
        .add(insights_job)
        .await
        .map_err(|e| Error::Internal(format!("Failed to add insights job: {}", e)))?;

    scheduler
        .add(edge_job)
        .await
        .map_err(|e| Error::Internal(format!("Failed to add edge refresh job: {}", e)))?;

    // Start the scheduler
    scheduler
        .start()
        .await
        .map_err(|e| Error::Internal(format!("Failed to start scheduler: {}", e)))?;

    info!("Scheduler initialized: rankings at 9am/3pm UTC, edge at 09:30 UTC, insights + race-odds at 10am UTC");
    Ok(scheduler)
}

/// Populate historical rankings from start_date to end_date inclusive, for all leagues
pub async fn populate_historical_rankings(
    db: &FantasyDb,
    nhl_client: &NhlClient,
    start_date: &str,
    end_date: &str,
) -> Result<()> {
    info!(
        "Populating historical rankings from {} to {}",
        start_date, end_date
    );

    let league_ids = db.get_all_league_ids().await?;
    if league_ids.is_empty() {
        info!("No leagues found - skipping historical rankings population");
        return Ok(());
    }

    // Parse dates
    let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .map_err(|e| Error::Validation(format!("Invalid start date: {}", e)))?;

    let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .map_err(|e| Error::Validation(format!("Invalid end date: {}", e)))?;

    // Iterate through each date
    let mut current = start;
    let mut success_count = 0;
    let mut failure_count = 0;

    while current <= end {
        let date_str = current.format("%Y-%m-%d").to_string();

        // Process this date for each league
        for league_id in &league_ids {
            match process_daily_rankings(db, nhl_client, &date_str, league_id).await {
                Ok(_) => {
                    info!("Processed rankings for {} league {}", date_str, league_id);
                    success_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to process rankings for {} league {}: {}",
                        date_str, league_id, e
                    );
                    failure_count += 1;
                }
            }
        }

        // Move to next day
        current = match current.checked_add_signed(Duration::days(1)) {
            Some(next) => next,
            None => break,
        };
    }

    info!(
        "Completed historical rankings population. Success: {}, Failures: {}",
        success_count, failure_count
    );
    Ok(())
}

pub async fn is_rankings_table_empty(db: &FantasyDb) -> Result<bool> {
    // Check if the table has any rows
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM daily_rankings")
        .fetch_one(db.pool())
        .await?;

    Ok(count == 0)
}
