use serde::{Deserialize, Serialize};

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

impl PlayoffCarouselResponse {
    /// Compute derived playoff state from the rounds data.
    pub fn with_computed_state(mut self) -> Self {
        let mut eliminated = std::collections::HashSet::new();
        let mut all_teams = std::collections::HashSet::new();
        let mut advanced = std::collections::HashSet::new();

        for round in &self.rounds {
            for series in &round.series {
                all_teams.insert(series.top_seed.abbrev.clone());
                all_teams.insert(series.bottom_seed.abbrev.clone());

                if series.top_seed.wins == 4 {
                    eliminated.insert(series.bottom_seed.abbrev.clone());
                    advanced.insert(series.top_seed.abbrev.clone());
                } else if series.bottom_seed.wins == 4 {
                    eliminated.insert(series.top_seed.abbrev.clone());
                    advanced.insert(series.bottom_seed.abbrev.clone());
                }
            }
        }

        // Teams in playoffs = all teams minus eliminated
        let in_playoffs: std::collections::HashSet<_> = all_teams
            .difference(&eliminated)
            .cloned()
            .collect();

        // Advanced but not eliminated
        let advanced_active: std::collections::HashSet<_> = advanced
            .difference(&eliminated)
            .cloned()
            .collect();

        self.eliminated_teams = eliminated.into_iter().collect();
        self.teams_in_playoffs = in_playoffs.into_iter().collect();
        self.advanced_teams = advanced_active.into_iter().collect();
        self
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
