use crate::models::nhl::{BoxscorePlayer, GameBoxscore, GameLogEntry};

/// Helper function to check if a team is the home team in a boxscore
fn is_home_team(boxscore: &GameBoxscore, team_abbrev: &str) -> bool {
    // This relies on team abbreviation matching, assuming BoxscorePlayer includes it or can be derived.
    // The original code checked player names which isn't robust.
    // Let's assume `GameBoxscore` structure might provide team abbrevs directly,
    // or we might need to adjust this based on the actual available data in `GameBoxscore`.
    // For now, we'll use a placeholder logic. A better approach might be needed.
    // A more robust way would be to have team IDs in the Boxscore structure.
    // We are checking players on the home team if their team matches the abbrev.

    // Check forwards first
    if boxscore
        .player_by_game_stats
        .home_team
        .forwards
        .iter()
        .any(|p| {
            // Assuming BoxscorePlayer has a team_abbrev field or similar
            // If not, this logic needs adaptation based on actual BoxscorePlayer structure.
            // Placeholder: let's assume a way to get the team abbrev exists.
            // If player struct has team abbrev: p.team_abbrev.to_lowercase() == team_abbrev.to_lowercase()
            // Using name check as fallback from original code [cite: 395]
            p.name
                .get("default")
                .is_some_and(|n| n.to_lowercase().contains(&team_abbrev.to_lowercase()))
        })
    {
        return true;
    }
    // Check defense
    if boxscore
        .player_by_game_stats
        .home_team
        .defense
        .iter()
        .any(|p| {
            p.name
                .get("default")
                .is_some_and(|n| n.to_lowercase().contains(&team_abbrev.to_lowercase()))
        })
    {
        return true;
    }
    // Check goalies
    if boxscore
        .player_by_game_stats
        .home_team
        .goalies
        .iter()
        .any(|p| {
            p.name
                .get("default")
                .is_some_and(|n| n.to_lowercase().contains(&team_abbrev.to_lowercase()))
        })
    {
        return true;
    }

    false
}

/// Helper function to check if player name matches (case-insensitive, checks last name)
fn is_name_match(search_name: &str, player: &BoxscorePlayer) -> bool {
    let empty_string = String::new();
    let default_name = player
        .name
        .get("default")
        .unwrap_or(&empty_string)
        .to_lowercase();
    // Extract last name from boxscore name which might be in format "C. McDavid"
    let boxscore_last_name = default_name
        .split_whitespace()
        .last()
        .unwrap_or("")
        .to_lowercase();
    // Match if either full name contains the other's last name
    default_name.contains(search_name)
        || search_name.contains(&boxscore_last_name)
        || boxscore_last_name == search_name
}

/// Find player stats by name in boxscore
pub fn find_player_stats_by_name(
    boxscore: &GameBoxscore,
    team_abbrev: &str,
    player_name: &str,
) -> (i32, i32) {
    // First try to find the player in the specified team
    let team_stats = if is_home_team(boxscore, team_abbrev) {
        &boxscore.player_by_game_stats.home_team
    } else {
        &boxscore.player_by_game_stats.away_team
    };
    // Convert player_name to lowercase for case-insensitive matching
    let search_name = player_name.to_lowercase();
    // Check forwards
    for player in &team_stats.forwards {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    // Check defense
    for player in &team_stats.defense {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    // Check goalies
    for player in &team_stats.goalies {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    // If we couldn't find the player in their expected team, try the other team
    // (in case of incorrect team assignment)
    let other_team_stats = if is_home_team(boxscore, team_abbrev) {
        &boxscore.player_by_game_stats.away_team
    } else {
        &boxscore.player_by_game_stats.home_team
    };
    // Check all players in the other team
    for player in &other_team_stats.forwards {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    for player in &other_team_stats.defense {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    for player in &other_team_stats.goalies {
        if is_name_match(&search_name, player) {
            return (player.goals.unwrap_or(0), player.assists.unwrap_or(0));
        }
    }

    (0, 0) // Return zeros if player not found
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
