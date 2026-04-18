use std::collections::HashMap;
use std::collections::HashSet;

use serde::Serialize;

use crate::domain::models::db::FantasyTeamWithPlayers;
use crate::domain::models::nhl::StatsLeaders;

/// Player stats with calculated fantasy points
#[derive(Debug, Default, Clone)]
pub struct PlayerStats {
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}

impl PlayerStats {
    pub fn calculate_player_points(
        &mut self,
        player_nhl_id: i64,
        stats: &StatsLeaders,
    ) -> PlayerStats {
        let mut goals = 0;
        let mut assists = 0;
        // Calculate points for goals
        if let Some(player) = stats.goals.iter().find(|p| p.id as i64 == player_nhl_id) {
            goals = player.value as i32;
        }

        // Calculate points for assists
        if let Some(player) = stats.assists.iter().find(|p| p.id as i64 == player_nhl_id) {
            assists = player.value as i32;
        }

        Self {
            goals,
            assists,
            total_points: goals + assists,
        }
    }
}

/// Team ranking with points
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

impl TeamRanking {
    /// Calculate rankings for all teams
    pub fn calculate_rankings(
        fantasy_teams: Vec<FantasyTeamWithPlayers>,
        top_skaters: StatsLeaders,
    ) -> Vec<TeamRanking> {
        // Calculate points for each team
        let mut rankings = Vec::new();

        for team in fantasy_teams {
            // Use a HashSet to track unique players by ID to avoid duplicates
            let mut seen_players = HashSet::new();

            // Calculate team totals
            let mut team_stats = PlayerStats::default();

            // Calculate points for each player and add to team totals
            for player in &team.players {
                // Skip if we've already processed this player
                if !seen_players.insert(player.nhl_id) {
                    continue;
                }

                let player_stats =
                    PlayerStats::default().calculate_player_points(player.nhl_id, &top_skaters);

                // Add to team totals
                team_stats.goals += player_stats.goals;
                team_stats.assists += player_stats.assists;
                team_stats.total_points += player_stats.total_points;
            }

            // Add team to rankings
            rankings.push(TeamRanking {
                rank: 0, // Will be set after sorting
                team_id: team.id,
                team_name: team.name,
                goals: team_stats.goals,
                assists: team_stats.assists,
                total_points: team_stats.total_points,
            });
        }

        // Sort by total points (descending)
        rankings.sort_by(|a, b| b.total_points.cmp(&a.total_points));

        // Assign ranks
        for (i, ranking) in rankings.iter_mut().enumerate() {
            ranking.rank = i + 1;
        }

        rankings
    }
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

