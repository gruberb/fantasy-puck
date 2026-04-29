use std::collections::HashMap;

use serde::Serialize;

/// Per-player counting-stat accumulator used by the team-breakdown
/// composition path to roll a list of `nhl_player_game_stats` rows up
/// into a single team total. Pure data; the aggregate logic itself
/// lives in `infra::db::nhl_mirror` (the SQL doing the SUMs) and the
/// handler that calls it.
#[derive(Debug, Default, Clone)]
pub struct PlayerStats {
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}

/// Per-team ranking row rendered on the dashboard's overall rankings
/// table. Built directly from `LeagueTeamSeasonTotalsRow`; the handler
/// owns the rank assignment so this struct stays a transport shape.
#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamRanking {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}
#[derive(Debug, Clone)]
pub struct PlayerGamePerformance {
    pub player_id: i64,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

#[derive(Debug, Clone)]
pub struct TeamDailyPerformance {
    pub team_id: i64,
    pub team_name: String,
    pub player_performances: Vec<PlayerGamePerformance>,
    pub total_points: i32,
    pub total_goals: i32,
    pub total_assists: i32,
}

/// Daily fantasy team ranking (domain model)
#[derive(Debug, Serialize, Clone)]
pub struct DailyRanking {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub daily_points: i32,
    pub daily_goals: i32,
    pub daily_assists: i32,
    pub player_highlights: Vec<PlayerHighlight>,
}

impl DailyRanking {
    pub fn build_rankings(
        team_performances: HashMap<i64, TeamDailyPerformance>,
    ) -> Vec<DailyRanking> {
        let mut rankings = team_performances
            .into_values()
            .map(|performance| {
                // Get top 3 players by points
                let mut players = performance.player_performances;
                players.sort_by(|a, b| b.points.cmp(&a.points));
                let top_players = players
                    .into_iter()
                    .take(3)
                    .map(|p| PlayerHighlight {
                        player_name: p.player_name,
                        points: p.points,
                        nhl_team: p.nhl_team,
                        nhl_id: p.nhl_id,
                    })
                    .collect();

                DailyRanking {
                    rank: 0, // Set after sorting
                    team_id: performance.team_id,
                    team_name: performance.team_name,
                    daily_points: performance.total_points,
                    daily_goals: performance.total_goals,
                    daily_assists: performance.total_assists,
                    player_highlights: top_players,
                }
            })
            .collect::<Vec<_>>();

        // Sort and assign ranks
        rankings.sort_by(|a, b| b.daily_points.cmp(&a.daily_points));
        for (i, ranking) in rankings.iter_mut().enumerate() {
            ranking.rank = i + 1;
        }

        rankings
    }
}

/// Player highlight information (domain model)
#[derive(Debug, Serialize, Clone)]
pub struct PlayerHighlight {
    pub player_name: String,
    pub points: i32,
    pub nhl_team: String,
    pub nhl_id: i64,
}

/// Fantasy team with players in a game
#[derive(Debug, Clone)]
pub struct FantasyTeamInGame {
    pub team_id: i64,
    pub team_name: String,
    pub players: Vec<PlayerInGame>,
}

/// Player information for game tracking
#[derive(Debug, Clone)]
pub struct PlayerInGame {
    pub player_id: i64,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub position: String,
}

