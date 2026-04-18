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
use crate::utils::playoff_ingest::ingest_playoff_games_for_range;
use crate::utils::scheduler;

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
