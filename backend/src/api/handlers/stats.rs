use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::domain::models::nhl::GAME_TYPE_PLAYOFFS;
use crate::error::Result;
use crate::infra::db::nhl_mirror;
use crate::tuning;

/// Endpoint to fetch top skaters.
pub async fn get_top_skaters(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TopSkatersParams>,
) -> Result<Json<ApiResponse<Vec<ConsolidatedPlayerStats>>>> {
    // Use parameters from the query, with defaults if not provided
    let season = params.season;
    let game_type = params.game_type;
    let limit = params.limit.min(tuning::http::MAX_TOP_SKATERS_LIMIT) as i64;
    let form_games = params.form_games.min(tuning::http::MAX_FORM_GAMES);

    // Build fantasy team mapping if league_id is provided
    let mut fantasy_mapping: HashMap<i64, FantasyTeamInfo> = HashMap::new();
    if let Some(ref league_id) = params.league_id {
        let fantasy_players_groups = state.db.get_nhl_teams_and_players(league_id).await?;
        for group in fantasy_players_groups {
            for player in group.players {
                fantasy_mapping.insert(
                    player.nhl_id,
                    FantasyTeamInfo {
                        team_id: player.fantasy_team_id,
                        team_name: player.fantasy_team_name,
                    },
                );
            }
        }
    }

    let pool = state.db.pool();
    let headshot = |id: i64| state.nhl_client.get_player_image_url(id);
    let logo = |team: &str| state.nhl_client.get_team_logo_url(team);

    // Playoffs: a real leaderboard over every skater who has appeared in
    // a playoff game, aggregated from the per-game mirror.
    if game_type == GAME_TYPE_PLAYOFFS {
        let rows =
            nhl_mirror::list_top_skaters(pool, season as i32, game_type as i16, limit).await?;
        let players = rows
            .into_iter()
            .map(|r| {
                let (first_name, last_name) = split_name(&r.name);
                let mut stats = HashMap::from([
                    ("goals".to_string(), r.goals as i32),
                    ("assists".to_string(), r.assists as i32),
                    ("points".to_string(), r.points as i32),
                    ("plusMinus".to_string(), r.plus_minus as i32),
                    ("penaltyMins".to_string(), r.pim as i32),
                ]);
                if let Some(toi) = r.toi {
                    stats.insert("toi".to_string(), toi as i32);
                }
                ConsolidatedPlayerStats {
                    id: r.player_id,
                    first_name,
                    last_name,
                    sweater_number: None,
                    headshot: headshot(r.player_id),
                    team_name: r.team_abbrev.clone(),
                    team_logo: logo(&r.team_abbrev),
                    team_abbrev: r.team_abbrev,
                    position: r.position,
                    stats,
                    fantasy_team: fantasy_mapping.get(&r.player_id).cloned(),
                    form: None,
                }
            })
            .collect::<Vec<_>>();
        return Ok(json_success(players));
    }

    // Other game types: mirrored season lines. The mirror has no PIM for
    // season totals, so `penaltyMins` is absent here.
    let rows =
        nhl_mirror::list_skater_leaders(pool, season as i32, game_type as i16, limit).await?;
    let mut form_by_id: HashMap<i64, PlayerForm> = if params.include_form && form_games > 0 {
        let ids: Vec<i64> = rows.iter().map(|r| r.player_id).collect();
        nhl_mirror::list_player_form(pool, &ids, form_games as i32)
            .await?
            .into_iter()
            .map(|f| {
                let form = PlayerForm {
                    games: f.games as usize,
                    goals: f.goals as i32,
                    assists: f.assists as i32,
                    points: f.points as i32,
                };
                (f.player_id, form)
            })
            .collect()
    } else {
        HashMap::new()
    };

    let players = rows
        .into_iter()
        .map(|r| {
            let mut stats = HashMap::from([
                ("goals".to_string(), r.goals),
                ("assists".to_string(), r.assists),
                ("points".to_string(), r.points),
            ]);
            if let Some(pm) = r.plus_minus {
                stats.insert("plusMinus".to_string(), pm);
            }
            if let Some(toi) = r.toi_per_game {
                stats.insert("toi".to_string(), toi);
            }
            ConsolidatedPlayerStats {
                id: r.player_id,
                first_name: r.first_name,
                last_name: r.last_name,
                sweater_number: None,
                headshot: headshot(r.player_id),
                team_name: r.team_abbrev.clone(),
                team_logo: logo(&r.team_abbrev),
                team_abbrev: r.team_abbrev,
                position: r.position,
                stats,
                fantasy_team: fantasy_mapping.get(&r.player_id).cloned(),
                form: form_by_id.remove(&r.player_id),
            }
        })
        .collect::<Vec<_>>();
    Ok(json_success(players))
}

/// Split a "First Last" (or "First Middle Last") name into (first, last).
fn split_name(full: &str) -> (String, String) {
    let trimmed = full.trim();
    match trimmed.rsplit_once(' ') {
        Some((first, last)) => (first.to_string(), last.to_string()),
        None => (trimmed.to_string(), String::new()),
    }
}
