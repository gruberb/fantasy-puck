use std::collections::HashMap;

use serde_json::Value;
use tracing::{info, warn};

use crate::api::dtos::PlayoffCarouselResponse;
use crate::error::{Error, Result};
use crate::infra::db::FantasyDb;
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// (name, position, team_abbrev, headshot_url)
pub type PoolEntry = (String, String, String, String);
pub type PoolMap = HashMap<i64, PoolEntry>;

fn headshot_url(player_id: i64) -> String {
    format!("https://assets.nhle.com/mugs/nhl/latest/{}.png", player_id)
}

/// Build a pool from `skater-stats-leaders` across 9 stat categories.
/// Used for the regular-season path (`game_type == 2`).
pub async fn fetch_stats_leader_pool(
    client: &NhlClient,
    season: u32,
    game_type: u8,
) -> Result<PoolMap> {
    let stats = client
        .get_skater_stats(&season, game_type)
        .await
        .map_err(|e| Error::NhlApi(format!("Failed to fetch skater stats: {}", e)))?;

    let categories = [
        &stats.goals,
        &stats.assists,
        &stats.points,
        &stats.goals_pp,
        &stats.goals_sh,
        &stats.plus_minus,
        &stats.faceoff_leaders,
        &stats.penalty_mins,
        &stats.toi,
    ];

    let mut map: PoolMap = HashMap::new();
    for category in categories {
        for player in category {
            let player_id = player.id as i64;
            map.entry(player_id).or_insert_with(|| {
                let first = player.first_name.get("default").cloned().unwrap_or_default();
                let last = player.last_name.get("default").cloned().unwrap_or_default();
                (
                    format!("{} {}", first, last),
                    player.position.clone(),
                    player.team_abbrev.clone(),
                    headshot_url(player_id),
                )
            });
        }
    }
    Ok(map)
}

/// Build a pool from the rosters of every team currently in the playoff bracket.
/// Skater-stats leaders return 0 players for playoffs until games have been
/// played, so we compose the pool from the 16 playoff team rosters instead.
pub async fn fetch_playoff_roster_pool(client: &NhlClient, season: u32) -> Result<PoolMap> {
    let abbrevs = playoff_team_abbrevs(client, season).await?;

    let mut map: PoolMap = HashMap::new();
    for (i, abbrev) in abbrevs.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
        }
        let roster = client
            .get_team_roster(abbrev)
            .await
            .map_err(|e| Error::NhlApi(format!("Failed to fetch playoff team roster: {}", e)))?;
        for player in roster {
            let player_id = player.id as i64;
            map.entry(player_id).or_insert_with(|| {
                let first = player.first_name.get("default").cloned().unwrap_or_default();
                let last = player.last_name.get("default").cloned().unwrap_or_default();
                (
                    format!("{} {}", first, last),
                    player.position.clone(),
                    player.team_abbrev.clone(),
                    headshot_url(player_id),
                )
            });
        }
    }
    Ok(map)
}

/// Playoff roster pool with a Postgres cache in front.
///
/// Cold reads cost 16 `get_team_roster` calls to NHL — enough to
/// burst the rate limit if fired in parallel when the in-memory client
/// cache expires (30m TTL) or on every restart. The scheduler's 10:00
/// UTC prewarm refreshes this row with paced roster fetches, and every
/// subsequent read is a single SELECT.
///
/// Cache miss semantics: fetch from NHL, warm the row, return. Even if
/// the NHL fetch fails we propagate the error to the caller — we'd
/// rather a stale but present cache, which `refresh_playoff_roster_cache`
/// is responsible for. In practice the scheduler prewarm keeps it fresh.
pub async fn fetch_playoff_roster_pool_cached(
    db: &FantasyDb,
    client: &NhlClient,
    season: u32,
    game_type: u8,
) -> Result<PoolMap> {
    if let Some(cached) = db
        .get_playoff_roster_cache(season as i32, game_type as i16)
        .await?
    {
        if let Ok(map) = serde_json::from_value::<PoolMap>(cached) {
            return Ok(map);
        }
        warn!(
            "playoff_roster_cache row for (season={}, game_type={}) failed to deserialize; falling back to NHL fetch",
            season, game_type
        );
    }

    let map = fetch_playoff_roster_pool(client, season).await?;
    if let Ok(value) = serde_json::to_value(&map) {
        if let Err(e) = db
            .upsert_playoff_roster_cache(season as i32, game_type as i16, &value)
            .await
        {
            warn!("Failed to persist playoff_roster_cache: {}", e);
        }
    }
    Ok(map)
}

/// Force-refresh the playoff roster cache row. Called from the 10:00 UTC
/// prewarm and from the admin prewarm endpoint. Returns the count of
/// players available for logging.
pub async fn refresh_playoff_roster_cache(
    db: &FantasyDb,
    client: &NhlClient,
    season: u32,
    game_type: u8,
) -> Result<usize> {
    let map = match fetch_playoff_roster_pool(client, season).await {
        Ok(map) => map,
        Err(fetch_err) => {
            if let Some(cached) = db
                .get_playoff_roster_cache(season as i32, game_type as i16)
                .await?
            {
                if let Ok(map) = serde_json::from_value::<PoolMap>(cached) {
                    warn!(
                        season,
                        game_type,
                        players = map.len(),
                        "Keeping existing playoff_roster_cache after refresh failure: {}",
                        fetch_err
                    );
                    return Ok(map.len());
                }
            }
            return Err(fetch_err);
        }
    };
    let value = serde_json::to_value(&map)
        .map_err(|e| Error::Internal(format!("Failed to serialize roster pool: {}", e)))?;
    db.upsert_playoff_roster_cache(season as i32, game_type as i16, &value)
        .await?;
    info!(
        season,
        game_type,
        players = map.len(),
        "Refreshed playoff_roster_cache"
    );
    Ok(map.len())
}

/// Returns 16 playoff team abbreviations. Prefers the carousel endpoint;
/// falls back to the top 16 teams from the standings if the carousel
/// hasn't been published yet (can happen briefly between season-end and
/// when the NHL posts Round 1 matchups).
async fn playoff_team_abbrevs(client: &NhlClient, season: u32) -> Result<Vec<String>> {
    if let Ok(Some(carousel)) = client.get_playoff_carousel(season.to_string()).await {
        if let Ok(val) = serde_json::to_value(&carousel) {
            if let Ok(resp) = serde_json::from_value::<PlayoffCarouselResponse>(val) {
                let computed = resp.with_computed_state();
                if computed.teams_in_playoffs.len() >= 16 {
                    return Ok(computed.teams_in_playoffs);
                }
            }
        }
    }

    warn!(
        "Playoff carousel empty or incomplete for season {}; falling back to top 16 from standings",
        season
    );
    standings_top_16(client).await
}

/// Parse `/v1/standings/now` and take the top 16 teams by point percentage.
/// The NHL standings endpoint returns teams pre-sorted within each division;
/// we flatten and sort by `pointPctg` desc to approximate playoff seeding.
async fn standings_top_16(client: &NhlClient) -> Result<Vec<String>> {
    let raw = client
        .get_standings_raw()
        .await
        .map_err(|e| Error::NhlApi(format!("Failed to fetch standings: {}", e)))?;

    let Some(teams) = raw.get("standings").and_then(Value::as_array) else {
        return Err(Error::NhlApi(
            "Standings response missing 'standings' array".into(),
        ));
    };

    let mut ranked: Vec<(String, f64)> = teams
        .iter()
        .filter_map(|team| {
            let abbrev = team
                .get("teamAbbrev")
                .and_then(|a| a.get("default"))
                .and_then(Value::as_str)?
                .to_string();
            let point_pctg = team
                .get("pointPctg")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            Some((abbrev, point_pctg))
        })
        .collect();

    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(ranked.into_iter().take(16).map(|(abbrev, _)| abbrev).collect())
}
