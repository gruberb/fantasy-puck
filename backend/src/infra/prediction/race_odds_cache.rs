//! Read-side adapter over the cached `/api/race-odds` payload.
//!
//! The Monte Carlo simulator runs at 10:00 UTC as part of the daily
//! prewarm and lands a `RaceOddsResponse` JSON blob under
//! `race_odds:v4:{league|global}:{season}:{gt}:{date}` in
//! `response_cache`. Handlers that want projection outputs — per-NHL-
//! team cup odds, expected games, etc. — read through this module
//! instead of re-running the simulator on the request path.

use std::collections::HashMap;
use std::sync::Arc;

use crate::api::dtos::race_odds::RaceOddsResponse;
use crate::api::routes::AppState;
use crate::domain::prediction::race_sim::NhlTeamOdds;

/// Read the cached race-odds payload's per-NHL-team array. Returns an
/// empty map if the cache is cold, the deserialize fails, or the
/// Monte Carlo cron has not run against this league yet.
pub async fn load_nhl_team_odds(
    state: &Arc<AppState>,
    league_id: &str,
    season: u32,
    game_type_num: u8,
    today: &str,
) -> HashMap<String, NhlTeamOdds> {
    let key = crate::infra::db::cache_keys::race_odds(league_id, season, game_type_num, today);
    match state
        .db
        .cache()
        .get_cached_response::<RaceOddsResponse>(&key)
        .await
    {
        Ok(Some(race_odds)) => race_odds
            .nhl_teams
            .into_iter()
            .map(|t| (t.abbrev.clone(), t))
            .collect(),
        _ => HashMap::new(),
    }
}

/// Narrow projection — only the cup-win probability — for callers
/// that just need the `HashMap<abbrev, cup_win_prob>` shape (Pulse's
/// narrator input).
pub async fn load_cached_cup_odds(
    state: &Arc<AppState>,
    league_id: &str,
    season: u32,
    game_type_num: u8,
    today: &str,
) -> HashMap<String, f32> {
    load_nhl_team_odds(state, league_id, season, game_type_num, today)
        .await
        .into_iter()
        .map(|(k, t)| (k, t.cup_win_prob))
        .collect()
}
