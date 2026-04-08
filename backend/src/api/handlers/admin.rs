use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;
use crate::utils::scheduler;

pub async fn process_rankings(
    State(state): State<Arc<AppState>>,
    Path(date): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
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
    State(state): State<Arc<AppState>>,
    Query(params): Query<InvalidateCacheParams>,
) -> Result<Json<ApiResponse<String>>> {
    match params.scope.as_deref() {
        Some("all") => {
            state.db.cache().invalidate_all().await?;
            state.nhl_client.invalidate_cache().await;
            Ok(json_success(
                "All cache entries invalidated (DB + NHL API in-memory)".to_string(),
            ))
        }
        Some("today") => {
            // Calculate today's date in Eastern time (NHL's primary timezone)
            // Using -5 for EST, -4 for EDT. A rough heuristic: March-November is EDT.
            let now_utc = chrono::Utc::now();
            let month = now_utc.format("%m").to_string().parse::<u32>().unwrap_or(1);
            let nhl_tz_offset = if (3..=10).contains(&month) { -4 } else { -5 };
            let now = now_utc + chrono::Duration::hours(nhl_tz_offset);
            let today = now.format("%Y-%m-%d").to_string();
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
            // Default to invalidating today's cache
            let now_utc = chrono::Utc::now();
            let month = now_utc.format("%m").to_string().parse::<u32>().unwrap_or(1);
            let nhl_tz_offset = if (3..=10).contains(&month) { -4 } else { -5 };
            let now = now_utc + chrono::Duration::hours(nhl_tz_offset);
            let today = now.format("%Y-%m-%d").to_string();
            let cache_key = format!("match_day:{}", today);
            state.db.cache().invalidate_cache(&cache_key).await?;
            Ok(json_success(format!(
                "Match day cache invalidated for today ({})",
                today
            )))
        }
    }
}
