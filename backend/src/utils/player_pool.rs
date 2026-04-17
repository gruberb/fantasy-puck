use std::collections::HashMap;

use futures::future::try_join_all;
use serde_json::Value;
use tracing::warn;

use crate::api::dtos::PlayoffCarouselResponse;
use crate::error::{Error, Result};
use crate::nhl_api::nhl::NhlClient;

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

    let rosters = try_join_all(abbrevs.iter().map(|abbrev| client.get_team_roster(abbrev)))
        .await
        .map_err(|e| Error::NhlApi(format!("Failed to fetch playoff team roster: {}", e)))?;

    let mut map: PoolMap = HashMap::new();
    for roster in rosters {
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
