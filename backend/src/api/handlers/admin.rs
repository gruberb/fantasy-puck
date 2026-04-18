use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{Error, Result};
use crate::infra::calibrate::{
    calibrate_season, calibrate_sweep, CalibrationGrid, CalibrationReport, SweepReport,
};
use crate::infra::jobs::playoff_ingest::{
    ingest_playoff_games_for_range, rebackfill_playoff_season_via_carousel,
};
use crate::infra::jobs::scheduler;

pub async fn process_rankings(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(date): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    // Process rankings for all leagues
    let league_ids = state.db.get_all_league_ids().await?;
    for league_id in &league_ids {
        info!("Processing rankings for league {} on {}", league_id, date);
        scheduler::process_daily_rankings(&state.db, &state.nhl_client, &date, league_id).await?;
    }
    Ok(json_success(format!(
        "Rankings processed for {} across {} leagues",
        date,
        league_ids.len()
    )))
}

#[derive(Deserialize)]
pub struct InvalidateCacheParams {
    scope: Option<String>, // "all", "today", or a specific date
}

pub async fn invalidate_cache(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<InvalidateCacheParams>,
) -> Result<Json<ApiResponse<String>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    match params.scope.as_deref() {
        Some("all") => {
            state.db.cache().invalidate_all().await?;
            state.nhl_client.invalidate_cache().await;
            Ok(json_success(
                "All cache entries invalidated (DB + NHL API in-memory)".to_string(),
            ))
        }
        Some("today") => {
            let today = crate::api::handlers::insights::hockey_today();
            state.db.cache().invalidate_by_date(&today).await?;
            Ok(json_success(format!(
                "Cache invalidated for today ({})",
                today
            )))
        }
        Some(date) => {
            state.db.cache().invalidate_by_date(date).await?;
            Ok(json_success(format!("Cache invalidated for date {}", date)))
        }
        None => {
            let today = crate::api::handlers::insights::hockey_today();
            let cache_key = format!("match_day:{}", today);
            state.db.cache().invalidate_cache(&cache_key).await?;
            Ok(json_success(format!(
                "Match day cache invalidated for today ({})",
                today
            )))
        }
    }
}

#[derive(Deserialize)]
pub struct BackfillHistoricalParams {
    /// Inclusive YYYY-MM-DD. Typically the playoff start of a past season.
    start: String,
    /// Inclusive YYYY-MM-DD. Typically the Cup Final end of the same season.
    end: String,
}

/// Backfill completed playoff games for a past date range into the
/// `playoff_game_results` + `playoff_skater_game_stats` tables. Only
/// completed, `game_type == 3` games are ingested — the existing
/// `ingest_playoff_games_for_date` filter handles this.
///
/// Meant to be run once per historical season to seed training data
/// for future Elo calibration work. Safe to re-run; upserts are
/// idempotent on `(game_id, player_id)` and `game_id`.
pub async fn backfill_historical_playoffs(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<BackfillHistoricalParams>,
) -> Result<Json<ApiResponse<String>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    let nhl = Arc::new(state.nhl_client.clone());
    let rows = ingest_playoff_games_for_range(&state.db, &nhl, &params.start, &params.end).await?;
    info!(
        start = %params.start,
        end = %params.end,
        rows,
        "historical-playoff backfill complete"
    );
    Ok(json_success(format!(
        "Backfilled {} skater rows for playoff games between {} and {}",
        rows, params.start, params.end
    )))
}

#[derive(Deserialize)]
pub struct RebackfillParams {
    /// 8-digit season, e.g. 20222023.
    season: u32,
}

/// Re-backfill a past season's `playoff_game_results` by walking the
/// playoff carousel + playoff-series-games endpoints instead of
/// iterating `/schedule/{date}`. The by-series endpoint reliably
/// returns every completed game for historical seasons, whereas the
/// schedule endpoint drops late-round games. Fixes the missing Cup
/// Final / conference-finals gaps that broke calibration ground truth.
///
/// `short_year` is derived automatically from the 8-digit season
/// (e.g. 20222023 → 2023) since that's what the series-games URL needs.
pub async fn rebackfill_carousel(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<RebackfillParams>,
) -> Result<Json<ApiResponse<String>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    let nhl = Arc::new(state.nhl_client.clone());
    let rows =
        rebackfill_playoff_season_via_carousel(&state.db, &nhl, params.season).await?;
    info!(season = params.season, rows, "carousel-driven rebackfill complete");
    Ok(json_success(format!(
        "Rebackfilled {} completed playoff games for season {}",
        rows, params.season
    )))
}

#[derive(Deserialize)]
pub struct CalibrateParams {
    /// 8-digit season, e.g. 20222023.
    season: u32,
}

/// Score the current race-odds model against a completed historical
/// season's realized outcomes. Returns per-team predicted-vs-outcome
/// deltas plus aggregate Brier / log-loss per round.
///
/// The season must already be backfilled via
/// `/api/admin/backfill-historical`. The sim runs with today's
/// production hyperparameters (Elo k, home-ice bonus, NB dispersion,
/// …) — if the aggregate Brier looks off, that's the signal we need
/// to invest in grid-search tuning.
pub async fn calibrate(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<CalibrateParams>,
) -> Result<Json<ApiResponse<CalibrationReport>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    let report = calibrate_season(&state.db, &state.nhl_client, params.season).await?;
    info!(
        season = params.season,
        teams = report.teams_evaluated,
        brier_r1 = report.brier_r1,
        brier_cup = report.brier_cup,
        "calibration run complete"
    );
    Ok(json_success(report))
}

#[derive(Deserialize)]
pub struct CalibrateSweepParams {
    /// 8-digit season, e.g. 20222023.
    season: u32,
    /// Comma-separated list of `POINTS_SCALE` values to try.
    /// Omitted/empty → production default.
    #[serde(default)]
    points_scale: Option<String>,
    /// Comma-separated list of shrinkage factors in `[0, 1]`.
    #[serde(default)]
    shrinkage: Option<String>,
    /// Comma-separated list of Elo `k_factor` values.
    #[serde(default)]
    k_factor: Option<String>,
    /// Comma-separated list of league-wide home-ice Elo bonuses.
    #[serde(default)]
    home_ice_elo: Option<String>,
    /// Comma-separated list of Monte Carlo trial counts.
    #[serde(default)]
    trials: Option<String>,
}

fn parse_f32_list(s: Option<&str>) -> std::result::Result<Vec<f32>, String> {
    let Some(s) = s else {
        return Ok(Vec::new());
    };
    let s = s.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split(',')
        .map(|piece| {
            piece
                .trim()
                .parse::<f32>()
                .map_err(|e| format!("invalid float '{piece}': {e}"))
        })
        .collect()
}

fn parse_usize_list(s: Option<&str>) -> std::result::Result<Vec<usize>, String> {
    let Some(s) = s else {
        return Ok(Vec::new());
    };
    let s = s.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split(',')
        .map(|piece| {
            piece
                .trim()
                .parse::<usize>()
                .map_err(|e| format!("invalid usize '{piece}': {e}"))
        })
        .collect()
}

/// Kick off the same pre-warm the 10am-UTC scheduler runs: insights +
/// race-odds for every league (plus the global no-league variants).
/// The work is spawned in a background tokio task so the HTTP response
/// returns immediately — otherwise a cold-cache warm-up (standings +
/// goalies + 5000-trial sim × every league) easily exceeds browser or
/// Fly edge timeouts. Watch the server logs for per-league completion.
pub async fn prewarm_cache(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<String>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    let db = state.db.clone();
    let nhl = state.nhl_client.clone();
    tokio::spawn(async move {
        info!("Admin-triggered pre-warm starting");
        scheduler::prewarm_derived_payloads(&db, &nhl).await;
        info!("Admin-triggered pre-warm complete");
    });
    Ok(json_success(
        "Pre-warm started in background; watch server logs for progress.".into(),
    ))
}

/// Grid-search calibration over a set of hyperparameter combinations.
/// One-off tool: you run it a handful of times to find winning knobs,
/// then bake the winners into the production constants and ship.
/// Not meant to be called from production code — it can spend minutes
/// of CPU and is not cached.
///
/// Grid is capped at 200 cells to keep a misconfigured sweep from
/// pegging the server for hours.
pub async fn calibrate_sweep_handler(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<CalibrateSweepParams>,
) -> Result<Json<ApiResponse<SweepReport>>> {
    if !auth.is_admin {
        return Err(Error::Forbidden("Admin access required".into()));
    }
    let grid = CalibrationGrid {
        points_scale: parse_f32_list(params.points_scale.as_deref())
            .map_err(Error::Validation)?,
        shrinkage: parse_f32_list(params.shrinkage.as_deref()).map_err(Error::Validation)?,
        k_factor: parse_f32_list(params.k_factor.as_deref()).map_err(Error::Validation)?,
        home_ice_elo: parse_f32_list(params.home_ice_elo.as_deref())
            .map_err(Error::Validation)?,
        trials: parse_usize_list(params.trials.as_deref()).map_err(Error::Validation)?,
    };
    let report = calibrate_sweep(&state.db, &state.nhl_client, params.season, &grid).await?;
    info!(
        season = params.season,
        grid_size = report.grid_size,
        best_brier_aggregate = report.best.brier_aggregate,
        best_brier_r1 = report.best.brier_r1,
        best_brier_cup = report.best.brier_cup,
        "calibration sweep complete"
    );
    Ok(json_success(report))
}
