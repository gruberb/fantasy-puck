use serde::{Deserialize, Serialize};

use crate::domain::models::nhl::SeriesStatus;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyTeamInfo {
    pub team_id: i64,
    pub team_name: String,
}

/// Form indicator for a player's recent performance
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerForm {
    pub games: usize,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatusResponse {
    pub round: u32,
    pub series_title: String,
    pub top_seed_team_abbrev: String,
    pub top_seed_wins: u32,
    pub bottom_seed_team_abbrev: String,
    pub bottom_seed_wins: u32,
    pub game_number_of_series: u32,
}

impl From<SeriesStatus> for SeriesStatusResponse {
    fn from(status: SeriesStatus) -> Self {
        Self {
            round: status.round,
            series_title: status.series_title,
            top_seed_team_abbrev: status.top_seed_team_abbrev,
            top_seed_wins: status.top_seed_wins,
            bottom_seed_team_abbrev: status.bottom_seed_team_abbrev,
            bottom_seed_wins: status.bottom_seed_wins,
            game_number_of_series: status.game_number_of_series,
        }
    }
}

pub fn series_context_label(status: &SeriesStatus) -> Option<String> {
    if status.round == 0 {
        return None;
    }

    let title = if status.series_title.trim().is_empty() {
        format!("Round {}", status.round)
    } else {
        status.series_title.clone()
    };

    let top_wins = status.top_seed_wins;
    let bottom_wins = status.bottom_seed_wins;
    let score = format!(
        "{}-{}",
        top_wins.max(bottom_wins),
        top_wins.min(bottom_wins)
    );

    let state = if top_wins == bottom_wins {
        format!("Series tied {}-{}", top_wins, bottom_wins)
    } else if top_wins >= 4 {
        format!("{} wins {}", status.top_seed_team_abbrev, score)
    } else if bottom_wins >= 4 {
        format!("{} wins {}", status.bottom_seed_team_abbrev, score)
    } else if top_wins > bottom_wins {
        format!("{} leads {}", status.top_seed_team_abbrev, score)
    } else {
        format!("{} leads {}", status.bottom_seed_team_abbrev, score)
    };

    Some(format!("{} - {}", title, state))
}

pub fn series_is_elimination_game(status: &SeriesStatus) -> bool {
    status.top_seed_wins == 3 || status.bottom_seed_wins == 3
}

#[cfg(test)]
mod series_context_tests {
    use super::{series_context_label, series_is_elimination_game};
    use crate::domain::models::nhl::SeriesStatus;

    fn status(top: u32, bottom: u32) -> SeriesStatus {
        SeriesStatus {
            round: 1,
            series_title: "1st Round".into(),
            top_seed_team_abbrev: "TBL".into(),
            top_seed_wins: top,
            bottom_seed_team_abbrev: "MTL".into(),
            bottom_seed_wins: bottom,
            game_number_of_series: top + bottom + 1,
        }
    }

    #[test]
    fn labels_tied_leading_elimination_and_completed_series() {
        assert_eq!(
            series_context_label(&status(3, 3)).as_deref(),
            Some("1st Round - Series tied 3-3")
        );
        assert_eq!(
            series_context_label(&status(3, 2)).as_deref(),
            Some("1st Round - TBL leads 3-2")
        );
        assert_eq!(
            series_context_label(&status(2, 3)).as_deref(),
            Some("1st Round - MTL leads 3-2")
        );
        assert_eq!(
            series_context_label(&status(4, 2)).as_deref(),
            Some("1st Round - TBL wins 4-2")
        );
        assert!(series_is_elimination_game(&status(3, 2)));
        assert!(!series_is_elimination_game(&status(4, 2)));
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesSummaryResponse {
    pub total_games: usize,
    pub total_teams_playing: usize,
    pub team_players_count: Vec<TeamPlayerCountResponse>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPlayerCountResponse {
    pub nhl_team: String,
    pub player_count: usize,
}
