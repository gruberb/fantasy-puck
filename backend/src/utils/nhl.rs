use crate::models::nhl::{BoxscorePlayer, GameBoxscore, GameLogEntry};

/// Find player stats by NHL ID in boxscore (primary), falling back to name matching.
pub fn find_player_stats_by_name(
    boxscore: &GameBoxscore,
    _team_abbrev: &str,
    player_name: &str,
    nhl_id: Option<i64>,
) -> (i32, i32) {
    let all_players: Vec<&BoxscorePlayer> = boxscore
        .player_by_game_stats
        .home_team
        .forwards
        .iter()
        .chain(&boxscore.player_by_game_stats.home_team.defense)
        .chain(&boxscore.player_by_game_stats.home_team.goalies)
        .chain(&boxscore.player_by_game_stats.away_team.forwards)
        .chain(&boxscore.player_by_game_stats.away_team.defense)
        .chain(&boxscore.player_by_game_stats.away_team.goalies)
        .collect();

    // Primary: match by NHL player ID
    if let Some(id) = nhl_id {
        if let Some(player) = all_players.iter().find(|p| p.player_id as i64 == id) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    // Fallback: match by last name (case-insensitive)
    let search_last = player_name
        .split_whitespace()
        .last()
        .unwrap_or(player_name)
        .to_lowercase();

    for player in &all_players {
        let boxscore_name = player
            .name
            .get("default")
            .map(|n| n.to_lowercase())
            .unwrap_or_default();
        let boxscore_last = boxscore_name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_lowercase();
        if boxscore_last == search_last {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    (0, 0)
}

/// Helper function to calculate form data from game log entries
pub fn calculate_form_from_game_log(
    game_log: &[GameLogEntry],
    num_games: usize,
) -> (i32, i32, i32, Vec<GameLogEntry>) {
    // Take the most recent n games (or fewer if not enough)
    let recent_games: Vec<&GameLogEntry> = game_log.iter().take(num_games).collect();
    if recent_games.is_empty() {
        return (0, 0, 0, Vec::new());
    }

    // Calculate totals
    let goals: i32 = recent_games.iter().map(|g| g.goals).sum();
    let assists: i32 = recent_games.iter().map(|g| g.assists).sum();
    let points: i32 = recent_games.iter().map(|g| g.points).sum();
    // Return copies of the recent games for reference
    let game_copies = recent_games.iter().map(|g| (*g).clone()).collect();
    (goals, assists, points, game_copies)
}

/// Calculate total statistics from game log
pub fn calculate_totals_from_game_log(game_log: &[GameLogEntry]) -> (i32, i32, i32, i32) {
    if game_log.is_empty() {
        return (0, 0, 0, 0);
    }

    let goals: i32 = game_log.iter().map(|g| g.goals).sum();
    let assists: i32 = game_log.iter().map(|g| g.assists).sum();
    let points: i32 = game_log.iter().map(|g| g.points).sum();
    let games = game_log.len() as i32;
    (goals, assists, points, games)
}
