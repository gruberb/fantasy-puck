use crate::models::fantasy::{FantasyTeamInGame, PlayerGamePerformance, TeamDailyPerformance};
use crate::models::nhl::GameBoxscore;
use crate::utils::nhl::find_player_stats_by_name;

pub fn process_game_performances(
    fantasy_teams: &[FantasyTeamInGame],
    boxscore: &GameBoxscore,
) -> Vec<TeamDailyPerformance> {
    // Group players by team and calculate their points
    fantasy_teams
        .iter()
        .map(|team| {
            // Process all players for this team
            let player_performances: Vec<PlayerGamePerformance> = team
                .players
                .iter()
                .map(|player| {
                    let (goals, assists) =
                        find_player_stats_by_name(boxscore, &player.nhl_team, &player.player_name, Some(player.nhl_id));
                    let points = goals + assists;

                    PlayerGamePerformance {
                        player_id: player.player_id,
                        nhl_id: player.nhl_id,
                        player_name: player.player_name.clone(),
                        nhl_team: player.nhl_team.clone(),
                        goals,
                        assists,
                        points,
                    }
                })
                .filter(|perf| perf.points > 0) // Only include players who scored
                .collect();

            // Calculate team total
            let total_assists = player_performances.iter().map(|p| p.assists).sum();
            let total_goals = player_performances.iter().map(|p| p.goals).sum();
            let total_points = player_performances.iter().map(|p| p.points).sum();

            TeamDailyPerformance {
                team_id: team.team_id,
                team_name: team.team_name.clone(),
                player_performances,
                total_assists,
                total_goals,
                total_points,
            }
        })
        .filter(|team| team.total_points > 0) // Only include teams with points
        .collect()
}
