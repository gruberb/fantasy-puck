use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, NaiveDate, Utc};
use futures::{stream, StreamExt, TryStreamExt};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

use crate::api::handlers::insights::generate_and_cache_insights;
use crate::api::handlers::race_odds::generate_and_cache_race_odds;
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::db::FantasyDb;
use crate::error::Result;
use crate::models::fantasy::{DailyRanking, TeamDailyPerformance};
use crate::tuning::scheduler as tuning;
use crate::utils::fantasy::process_game_performances;
use crate::utils::player_pool::refresh_playoff_roster_cache;
use crate::utils::playoff_ingest::ingest_playoff_games_for_date;
use crate::ws::draft_hub::DraftHub;
use crate::{Error, NhlClient};

/// Process and store daily rankings for a specific date and league
pub async fn process_daily_rankings(
    db: &FantasyDb,
    nhl_client: &NhlClient,
    date: &str,
    league_id: &str,
) -> Result<()> {
    info!(
        "Processing daily rankings for date: {}, league: {}",
        date, league_id
    );

    // Fetch the games for the specified date
    let games_response = nhl_client.get_schedule_by_date(date).await?;
    let all_games = games_response.games_for_date(date);

    // Check if we have any games for this date
    if all_games.is_empty() {
        info!("No games found for date {}", date);
        return Ok(());
    }

    // Check if ALL games are completed (none are live)
    let any_games_live = all_games.iter().any(|game| game.game_state.is_live());
    if any_games_live {
        info!(
            "Some games are still in progress for date {}. Skipping rankings processing.",
            date
        );
        return Ok(());
    }

    // Get only completed games
    let completed_games = all_games
        .into_iter()
        .filter(|game| game.game_state.is_completed())
        .collect::<Vec<_>>();

    // If no completed games
    if completed_games.is_empty() {
        info!("No completed games found for date {}", date);
        return Ok(());
    }

    // Process all completed games and aggregate team performances
    let all_team_performances = stream::iter(completed_games)
        .map(|game| {
            let db_clone = db.clone();
            let nhl_client_clone = nhl_client.clone();
            let league_id_owned = league_id.to_string();
            async move {
                // Try to get boxscore for this game
                let boxscore = nhl_client_clone
                    .get_game_boxscore(game.id)
                    .await
                    .map_err(|e| {
                        error!(
                            "Warning: Could not fetch boxscore for game {}: {}",
                            game.id, e
                        );
                        Error::Internal(
                            "Internal Server Error trying to get NHL Game information".to_string(),
                        )
                    })?;

                // Get fantasy players for both teams, scoped to league
                let home_team = game.home_team.abbrev.as_str();
                let away_team = game.away_team.abbrev.as_str();
                let fantasy_players = db_clone
                    .get_fantasy_players_for_nhl_teams(&[home_team, away_team], &league_id_owned)
                    .await?;

                // Process performances for this game
                Ok::<Vec<TeamDailyPerformance>, Error>(process_game_performances(
                    &fantasy_players,
                    &boxscore,
                ))
            }
        })
        .buffer_unordered(4) // Process up to 4 games concurrently
        .try_fold(
            HashMap::<i64, TeamDailyPerformance>::new(),
            |mut acc, performances| async move {
                // Merge these performances into the accumulator
                for perf in performances {
                    acc.entry(perf.team_id)
                        .and_modify(|existing| {
                            existing
                                .player_performances
                                .extend(perf.player_performances.clone());
                            existing.total_points += perf.total_points;
                            existing.total_assists += perf.total_assists;
                            existing.total_goals += perf.total_goals;
                        })
                        .or_insert(perf);
                }
                Ok(acc)
            },
        )
        .await?;

    // Convert to rankings domain model
    let daily_rankings = DailyRanking::build_rankings(all_team_performances);

    // Store rankings in the database (with league_id)
    for ranking in &daily_rankings {
        sqlx::query(
            "INSERT INTO daily_rankings (date, team_id, league_id, rank, points, goals, assists)
                    VALUES ($1, $2, $3::uuid, $4, $5, $6, $7)
                    ON CONFLICT (team_id, date, league_id) DO UPDATE SET
                        rank = EXCLUDED.rank,
                        points = EXCLUDED.points,
                        goals = EXCLUDED.goals,
                        assists = EXCLUDED.assists",
        )
        .bind(date)
        .bind(ranking.team_id)
        .bind(league_id)
        .bind(ranking.rank as i64)
        .bind(ranking.daily_points)
        .bind(ranking.daily_goals)
        .bind(ranking.daily_assists)
        .execute(db.pool())
        .await?;
    }

    info!(
        "Successfully stored daily rankings for date: {}, league: {}",
        date, league_id
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
    let state = Arc::new(AppState {
        db: db.clone(),
        nhl_client: nhl_client.clone(),
        config: Arc::new(crate::config::Config::from_env()),
        draft_hub: DraftHub::new(),
    });

    // Playoff roster pool — 16 team rosters written into Postgres so
    // every downstream cold read is one SELECT instead of 16 parallel
    // NHL calls. Failures are logged but non-fatal; the cached fetch
    // path falls back to the NHL fanout on first read.
    if game_type() == 3 {
        match refresh_playoff_roster_cache(db, nhl_client, season(), game_type()).await {
            Ok(n) => info!("Pre-warmed playoff roster cache ({} players)", n),
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

    // Start the scheduler
    scheduler
        .start()
        .await
        .map_err(|e| Error::Internal(format!("Failed to start scheduler: {}", e)))?;

    info!("Scheduler initialized: rankings at 9am/3pm UTC, insights + race-odds at 10am UTC");
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
