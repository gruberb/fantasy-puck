use serde::{Deserialize, Serialize};

use crate::domain::models::nhl::PlayoffCarousel;
use crate::domain::prediction::carousel::bracket_state;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffCarouselResponse {
    pub current_round: i64,
    pub rounds: Vec<RoundResponse>,
    #[serde(default)]
    pub eliminated_teams: Vec<String>,
    #[serde(default)]
    pub teams_in_playoffs: Vec<String>,
    #[serde(default)]
    pub advanced_teams: Vec<String>,
}

impl From<PlayoffCarousel> for PlayoffCarouselResponse {
    fn from(carousel: PlayoffCarousel) -> Self {
        let state = bracket_state(&carousel);
        Self {
            current_round: carousel.current_round,
            rounds: carousel
                .rounds
                .into_iter()
                .map(|r| RoundResponse {
                    round_number: r.round_number,
                    round_label: r.round_label,
                    round_abbrev: r.round_abbrev,
                    series: r
                        .series
                        .into_iter()
                        .map(|s| SeriesResponse {
                            series_letter: s.series_letter,
                            round_number: s.round_number,
                            series_label: s.series_label,
                            bottom_seed: Seed {
                                id: s.bottom_seed.id,
                                abbrev: s.bottom_seed.abbrev,
                                wins: s.bottom_seed.wins,
                            },
                            top_seed: Seed {
                                id: s.top_seed.id,
                                abbrev: s.top_seed.abbrev,
                                wins: s.top_seed.wins,
                            },
                        })
                        .collect(),
                })
                .collect(),
            eliminated_teams: state.eliminated.into_iter().collect(),
            teams_in_playoffs: state.alive.into_iter().collect(),
            advanced_teams: state.advanced.into_iter().collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResponse {
    pub round_number: i64,
    pub round_label: String,
    pub round_abbrev: String,
    pub series: Vec<SeriesResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResponse {
    pub series_letter: String,
    pub round_number: i64,
    pub series_label: String,
    pub bottom_seed: Seed,
    pub top_seed: Seed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seed {
    pub id: i64,
    pub abbrev: String,
    pub wins: i64,
}
