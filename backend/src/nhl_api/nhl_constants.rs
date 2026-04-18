//! Constants for NHL API endpoints
//!
//! This file contains all of the NHL API endpoints organized by resource type
//! to improve maintainability and avoid duplicating string literals.

/// Base URL for NHL API
pub const NHL_API_BASE_URL: &str = "https://api-web.nhle.com";
pub const NHL_FETCH_LIMIT: u32 = 1000;

/// Player related endpoints
pub mod players {
    use super::NHL_API_BASE_URL;

    /// Get a player's details
    pub fn player_details(player_id: i64) -> String {
        format!("{}/v1/player/{}/landing", NHL_API_BASE_URL, player_id)
    }

    /// Get a player's image
    pub fn player_image(player_id: i64) -> String {
        format!("https://assets.nhle.com/mugs/nhl/latest/{}.png", player_id)
    }

    /// Get a team's current roster
    pub fn team_roster(team_abbrev: &str) -> String {
        format!("{}/v1/roster/{}/current", NHL_API_BASE_URL, team_abbrev)
    }

    /// Get a player's game log for a specific season and game type
    pub fn player_game_log(player_id: i64, season: &u32, game_type: u8) -> String {
        format!(
            "{}/v1/player/{}/game-log/{}/{}",
            NHL_API_BASE_URL, player_id, season, game_type
        )
    }
}

/// Team related endpoints
pub mod teams {
    use super::NHL_API_BASE_URL;

    /// Get current standings (also used to get all teams)
    pub const STANDINGS: &str = "/v1/standings/now";

    /// Get a team's logo
    pub fn team_logo(team_abbrev: &str) -> String {
        format!(
            "https://assets.nhle.com/logos/nhl/svg/{}_light.svg",
            team_abbrev
        )
    }

    /// Get full standings URL
    pub fn standings_url() -> String {
        format!("{}{}", NHL_API_BASE_URL, STANDINGS)
    }
}

/// Playoffs related endpoints
pub mod playoffs {
    use super::NHL_API_BASE_URL;

    pub const CAROUSEL: &str = "/v1/playoff-series/carousel/";

    /// Get playoff carousel for specific season
    pub fn carousel_for_season(season: String) -> String {
        format!("{}{}{}", NHL_API_BASE_URL, CAROUSEL, season)
    }

    /// Get the games list for one playoff series. Season is the
    /// 4-digit year of the second half (e.g. `2023` for 2022-23);
    /// letter is lowercase (`a`..`m` etc., values from the carousel's
    /// `seriesLetter`).
    pub fn series_games(season: u32, letter: &str) -> String {
        format!(
            "{}/v1/schedule/playoff-series/{}/{}",
            NHL_API_BASE_URL,
            season,
            letter.to_lowercase()
        )
    }
}

/// Game related endpoints
pub mod games {
    use super::NHL_API_BASE_URL;

    /// Get today's schedule
    pub const TODAY_SCHEDULE: &str = "/v1/schedule/now";

    /// Get today's schedule URL
    pub fn today_schedule_url() -> String {
        format!("{}{}", NHL_API_BASE_URL, TODAY_SCHEDULE)
    }

    /// Get schedule for a specific date
    pub fn schedule_by_date(date: &str) -> String {
        format!("{}/v1/schedule/{}", NHL_API_BASE_URL, date)
    }

    /// Get game center information
    pub fn game_center(game_id: u32) -> String {
        format!("{}/v1/gamecenter/{}/landing", NHL_API_BASE_URL, game_id)
    }

    /// Get game boxscore
    pub fn game_boxscore(game_id: u32) -> String {
        format!("{}/v1/gamecenter/{}/boxscore", NHL_API_BASE_URL, game_id)
    }
}

/// Score/results endpoints
pub mod scores {
    use super::NHL_API_BASE_URL;

    /// Get scores for a specific date
    pub fn scores_by_date(date: &str) -> String {
        format!("{}/v1/score/{}", NHL_API_BASE_URL, date)
    }
}

/// NHL Edge advanced analytics endpoints
pub mod edge {
    use super::NHL_API_BASE_URL;

    /// Get Edge analytics detail for a skater (skating speed, shot speed, etc.)
    pub fn skater_detail(player_id: i64) -> String {
        format!(
            "{}/v1/edge/skater-detail/{}/now",
            NHL_API_BASE_URL, player_id
        )
    }
}

/// Stats related endpoints
pub mod stats {
    use crate::nhl_api::nhl_constants::NHL_FETCH_LIMIT;

    use super::NHL_API_BASE_URL;

    /// Get skater stats leaders for a season and game type
    pub fn skater_stats_leaders(season: &u32, game_type: u8) -> String {
        format!(
            "{}/v1/skater-stats-leaders/{}/{}?limit={}",
            NHL_API_BASE_URL, season, game_type, NHL_FETCH_LIMIT
        )
    }
}

pub mod team_names {
    use std::collections::HashMap;
    use std::sync::OnceLock;

    // Create a static mapping that's initialized once
    pub fn get_team_names() -> &'static HashMap<&'static str, &'static str> {
        static TEAM_NAMES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
        TEAM_NAMES.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert("ANA", "Anaheim Ducks");
            // Arizona Coyotes relocated to Utah after 2023-24 season
            // map.insert("ARI", "Arizona Coyotes");
            map.insert("BOS", "Boston Bruins");
            map.insert("BUF", "Buffalo Sabres");
            map.insert("CGY", "Calgary Flames");
            map.insert("CAR", "Carolina Hurricanes");
            map.insert("CHI", "Chicago Blackhawks");
            map.insert("COL", "Colorado Avalanche");
            map.insert("CBJ", "Columbus Blue Jackets");
            map.insert("DAL", "Dallas Stars");
            map.insert("DET", "Detroit Red Wings");
            map.insert("EDM", "Edmonton Oilers");
            map.insert("FLA", "Florida Panthers");
            map.insert("LAK", "Los Angeles Kings");
            map.insert("MIN", "Minnesota Wild");
            map.insert("MTL", "Montreal Canadiens");
            map.insert("NSH", "Nashville Predators");
            map.insert("NJD", "New Jersey Devils");
            map.insert("NYI", "New York Islanders");
            map.insert("NYR", "New York Rangers");
            map.insert("OTT", "Ottawa Senators");
            map.insert("PHI", "Philadelphia Flyers");
            map.insert("PIT", "Pittsburgh Penguins");
            map.insert("SJS", "San Jose Sharks");
            map.insert("SEA", "Seattle Kraken");
            map.insert("STL", "St. Louis Blues");
            map.insert("TBL", "Tampa Bay Lightning");
            map.insert("TOR", "Toronto Maple Leafs");
            map.insert("UTA", "Utah Mammoth");
            map.insert("VAN", "Vancouver Canucks");
            map.insert("VGK", "Vegas Golden Knights");
            map.insert("WSH", "Washington Capitals");
            map.insert("WPG", "Winnipeg Jets");
            map
        })
    }

    // Helper function to get team name from abbreviation
    pub fn get_team_name(abbrev: &str) -> &'static str {
        get_team_names().get(abbrev).unwrap_or(&"Unknown Team")
    }
}
