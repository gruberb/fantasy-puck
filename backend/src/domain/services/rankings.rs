//! Pure ranking math. Given raw boxscore/leaderboard data from the
//! mirror tables and a league's roster, produce ranked fantasy
//! results. No IO, no SQL, no HTTP.

use std::collections::HashMap;

use crate::domain::models::db::FantasyTeamWithPlayers;
use crate::domain::models::fantasy::{
    DailyRanking, PlayerGamePerformance, PlayerHighlight, TeamDailyPerformance, TeamRanking,
};

/// Flat shape the rankings services consume, independent of the
/// wire format used by `infra::db::nhl_mirror::SkaterSeasonRow`. The
/// handler adapts one to the other on the way in.
pub struct SeasonSkaterStat {
    pub nhl_id: i64,
    pub goals: i32,
    pub assists: i32,
}

/// Flat shape for daily rankings input. One row per rostered-player
/// appearance in a completed game on the day.
pub struct DailyPlayerStat {
    pub team_id: i64,
    pub team_name: String,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

/// Season-overall fantasy rankings. For each team, sum each rostered
/// player's goals/assists from the season leaderboard, then rank
/// descending by total points.
///
/// Duplicate nhl_ids within a team (possible if the roster has the
/// same player listed twice for any reason) are deduplicated before
/// summing — matches the pre-mirror behaviour that used a HashSet.
pub fn calculate_team_rankings(
    teams: Vec<FantasyTeamWithPlayers>,
    skater_stats: &[SeasonSkaterStat],
) -> Vec<TeamRanking> {
    let by_player: HashMap<i64, (i32, i32)> = skater_stats
        .iter()
        .map(|s| (s.nhl_id, (s.goals, s.assists)))
        .collect();

    let mut rankings: Vec<TeamRanking> = teams
        .into_iter()
        .map(|team| {
            let mut seen = std::collections::HashSet::new();
            let mut goals = 0;
            let mut assists = 0;
            for p in &team.players {
                if !seen.insert(p.nhl_id) {
                    continue;
                }
                if let Some((g, a)) = by_player.get(&p.nhl_id) {
                    goals += g;
                    assists += a;
                }
            }
            TeamRanking {
                rank: 0,
                team_id: team.id,
                team_name: team.name,
                goals,
                assists,
                total_points: goals + assists,
            }
        })
        .collect();

    rankings.sort_by(|a, b| b.total_points.cmp(&a.total_points));
    for (i, r) in rankings.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    rankings
}

/// Daily fantasy rankings. Groups per-player rows by fantasy team,
/// sums team totals, takes the top-3 scorers per team as
/// `player_highlights`, then sorts by `daily_points` desc.
pub fn build_daily_rankings(rows: Vec<DailyPlayerStat>) -> Vec<DailyRanking> {
    let mut by_team: HashMap<i64, TeamDailyPerformance> = HashMap::new();
    for r in rows {
        let entry = by_team
            .entry(r.team_id)
            .or_insert_with(|| TeamDailyPerformance {
                team_id: r.team_id,
                team_name: r.team_name.clone(),
                player_performances: Vec::new(),
                total_points: 0,
                total_goals: 0,
                total_assists: 0,
            });
        // Only include players who scored, matching the pre-mirror
        // behaviour — zero-point skaters don't appear in highlights.
        if r.points > 0 {
            entry.player_performances.push(PlayerGamePerformance {
                player_id: 0, // not surfaced in the response DTO
                nhl_id: r.nhl_id,
                player_name: r.player_name,
                nhl_team: r.nhl_team,
                goals: r.goals,
                assists: r.assists,
                points: r.points,
            });
        }
        entry.total_goals += r.goals;
        entry.total_assists += r.assists;
        entry.total_points += r.points;
    }

    let mut rankings: Vec<DailyRanking> = by_team
        .into_values()
        .map(|perf| {
            let mut players = perf.player_performances;
            players.sort_by(|a, b| b.points.cmp(&a.points));
            let highlights: Vec<PlayerHighlight> = players
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
                rank: 0,
                team_id: perf.team_id,
                team_name: perf.team_name,
                daily_points: perf.total_points,
                daily_goals: perf.total_goals,
                daily_assists: perf.total_assists,
                player_highlights: highlights,
            }
        })
        .collect();

    rankings.sort_by(|a, b| b.daily_points.cmp(&a.daily_points));
    for (i, r) in rankings.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    rankings
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::db::FantasyPlayer;

    fn mk_player(nhl_id: i64, name: &str) -> FantasyPlayer {
        FantasyPlayer {
            id: nhl_id,
            team_id: 0,
            nhl_id,
            name: name.to_string(),
            position: String::new(),
            nhl_team: String::new(),
        }
    }

    #[test]
    fn team_ranking_sums_goals_and_assists_from_leaderboard() {
        let team_a = FantasyTeamWithPlayers {
            id: 1,
            name: "A".into(),
            players: vec![mk_player(10, "Ovi"), mk_player(20, "McDavid")],
        };
        let team_b = FantasyTeamWithPlayers {
            id: 2,
            name: "B".into(),
            players: vec![mk_player(30, "Crosby")],
        };
        let stats = vec![
            SeasonSkaterStat { nhl_id: 10, goals: 5, assists: 3 },
            SeasonSkaterStat { nhl_id: 20, goals: 2, assists: 4 },
            SeasonSkaterStat { nhl_id: 30, goals: 1, assists: 1 },
        ];
        let r = calculate_team_rankings(vec![team_a, team_b], &stats);
        assert_eq!(r[0].team_id, 1);
        assert_eq!(r[0].goals, 7);
        assert_eq!(r[0].assists, 7);
        assert_eq!(r[0].total_points, 14);
        assert_eq!(r[0].rank, 1);
        assert_eq!(r[1].rank, 2);
    }

    #[test]
    fn team_ranking_deduplicates_repeat_nhl_ids() {
        let team = FantasyTeamWithPlayers {
            id: 1,
            name: "A".into(),
            players: vec![mk_player(10, "Ovi"), mk_player(10, "Ovi dup")],
        };
        let stats = vec![SeasonSkaterStat { nhl_id: 10, goals: 5, assists: 3 }];
        let r = calculate_team_rankings(vec![team], &stats);
        assert_eq!(r[0].total_points, 8);
    }

    #[test]
    fn daily_rankings_filter_zero_point_players_from_highlights_but_sum_totals() {
        let rows = vec![
            DailyPlayerStat {
                team_id: 1,
                team_name: "A".into(),
                nhl_id: 10,
                player_name: "Ovi".into(),
                nhl_team: "WSH".into(),
                goals: 1,
                assists: 1,
                points: 2,
            },
            DailyPlayerStat {
                team_id: 1,
                team_name: "A".into(),
                nhl_id: 11,
                player_name: "Backstrom".into(),
                nhl_team: "WSH".into(),
                goals: 0,
                assists: 0,
                points: 0,
            },
        ];
        let r = build_daily_rankings(rows);
        assert_eq!(r[0].daily_points, 2);
        assert_eq!(r[0].player_highlights.len(), 1, "zero-point players not in highlights");
        assert_eq!(r[0].player_highlights[0].nhl_id, 10);
    }
}
