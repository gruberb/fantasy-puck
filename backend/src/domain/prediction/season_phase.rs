//! Where the playoffs stand, derived straight from the NHL bracket carousel.
//!
//! Two consumers need this: the Insights and Pulse narrators. While the
//! bracket is live they want a one-line "we're in the Stanley Cup Final"
//! cue; once the Cup is decided they switch to a season recap. The trigger
//! is the bracket itself (a clinched final), not the calendar, so a Game 7
//! that runs past midnight Eastern doesn't flip the app into recap mode a
//! day early.
//!
//! Note the asymmetry with `insights::active_round_projections`, which drops
//! completed series: the champion lives in a *finished* series, so we read
//! the raw carousel here rather than the projections.

use crate::domain::models::nhl::{PlayoffCarousel, Series};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeasonPhase {
    /// The bracket is still being decided. `round_label` is the
    /// human-facing label of the deepest round with a known matchup
    /// (e.g. "Stanley Cup Final"); `summary` describes its live series.
    InProgress { round_label: String, summary: String },
    /// The final is clinched. Abbreviations, not full names — the domain
    /// layer has no team-name table; callers expand if they want prose.
    Over {
        champion: String,
        runner_up: String,
        /// Winning margin, e.g. "4-2".
        series_label: String,
    },
}

impl SeasonPhase {
    pub fn from_carousel(carousel: &PlayoffCarousel) -> SeasonPhase {
        // The final is structurally the highest-numbered round. If its
        // series is clinched, that decides the champion.
        if let Some(final_round) = carousel.rounds.iter().max_by_key(|r| r.round_number) {
            for series in &final_round.series {
                if let Some((champion, runner_up, series_label)) = decided_winner(series) {
                    return SeasonPhase::Over {
                        champion,
                        runner_up,
                        series_label,
                    };
                }
            }
        }

        // Not over: describe the deepest round that has a real matchup.
        // TBD slots in not-yet-seeded rounds are skipped so we don't
        // report "Stanley Cup Final" before the finalists are known.
        match carousel
            .rounds
            .iter()
            .filter(|r| r.series.iter().any(series_has_two_known_teams))
            .max_by_key(|r| r.round_number)
        {
            Some(round) => {
                let summary = round
                    .series
                    .iter()
                    .filter_map(describe_series)
                    .collect::<Vec<_>>()
                    .join("; ");
                SeasonPhase::InProgress {
                    round_label: normalize_round_label(&round.round_label, round.round_number),
                    summary,
                }
            }
            None => SeasonPhase::InProgress {
                round_label: "Playoffs".to_string(),
                summary: String::new(),
            },
        }
    }

    pub fn is_over(&self) -> bool {
        matches!(self, SeasonPhase::Over { .. })
    }

    /// One line for the `=== SEASON STATE ===` block handed to the narrator.
    pub fn prompt_line(&self) -> String {
        match self {
            SeasonPhase::Over {
                champion,
                runner_up,
                series_label,
            } => format!(
                "Season complete. {champion} won the Stanley Cup over {runner_up} ({series_label})."
            ),
            SeasonPhase::InProgress {
                round_label,
                summary,
            } => {
                if summary.is_empty() {
                    format!("{round_label} in progress.")
                } else {
                    format!("{round_label} in progress. {summary}.")
                }
            }
        }
    }
}

/// A best-of-7 needs 4 wins; treat a missing/zero `needed_to_win` as 4 so a
/// malformed carousel row can't be read as "already clinched" at 0-0.
fn games_to_clinch(series: &Series) -> i64 {
    if series.needed_to_win > 0 {
        series.needed_to_win
    } else {
        4
    }
}

fn team_known(abbrev: &str) -> bool {
    !abbrev.trim().is_empty() && !abbrev.eq_ignore_ascii_case("tbd")
}

fn series_has_two_known_teams(series: &Series) -> bool {
    team_known(&series.top_seed.abbrev) && team_known(&series.bottom_seed.abbrev)
}

/// `Some((winner, loser, "4-2"))` if a known matchup has clinched, else `None`.
fn decided_winner(series: &Series) -> Option<(String, String, String)> {
    if !series_has_two_known_teams(series) {
        return None;
    }
    let needed = games_to_clinch(series);
    let top = (&series.top_seed.abbrev, series.top_seed.wins);
    let bottom = (&series.bottom_seed.abbrev, series.bottom_seed.wins);
    let (winner, loser) = if top.1 >= needed {
        (top, bottom)
    } else if bottom.1 >= needed {
        (bottom, top)
    } else {
        return None;
    };
    Some((
        winner.0.clone(),
        loser.0.clone(),
        format!("{}-{}", winner.1, loser.1),
    ))
}

/// "FLA leads CAR 3-2", "FLA beat CAR 4-1", or "FLA and CAR tied 2-2".
fn describe_series(series: &Series) -> Option<String> {
    if !series_has_two_known_teams(series) {
        return None;
    }
    let top = (&series.top_seed.abbrev, series.top_seed.wins);
    let bottom = (&series.bottom_seed.abbrev, series.bottom_seed.wins);
    if top.1 == bottom.1 {
        return Some(format!("{} and {} tied {}-{}", top.0, bottom.0, top.1, bottom.1));
    }
    let (lead, trail) = if top.1 > bottom.1 { (top, bottom) } else { (bottom, top) };
    let verb = if lead.1 >= games_to_clinch(series) {
        "beat"
    } else {
        "leads"
    };
    Some(format!("{} {verb} {} {}-{}", lead.0, trail.0, lead.1, trail.1))
}

fn normalize_round_label(label: &str, round_number: i64) -> String {
    if label.trim().is_empty() {
        return format!("Round {round_number}");
    }
    label.replace('-', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::nhl::{BottomSeed, Round, Series, TopSeed};

    fn series(round: i64, top: &str, tw: i64, bottom: &str, bw: i64) -> Series {
        Series {
            series_letter: "A".into(),
            round_number: round,
            series_label: String::new(),
            series_link: String::new(),
            bottom_seed: BottomSeed {
                abbrev: bottom.into(),
                wins: bw,
                ..Default::default()
            },
            top_seed: TopSeed {
                abbrev: top.into(),
                wins: tw,
                ..Default::default()
            },
            needed_to_win: 4,
        }
    }

    fn round(number: i64, label: &str, series: Vec<Series>) -> Round {
        Round {
            round_number: number,
            round_label: label.into(),
            round_abbrev: String::new(),
            series,
        }
    }

    fn carousel(rounds: Vec<Round>) -> PlayoffCarousel {
        PlayoffCarousel {
            season_id: 20252026,
            current_round: rounds.iter().map(|r| r.round_number).max().unwrap_or(1),
            rounds,
        }
    }

    #[test]
    fn detects_champion_from_clinched_final() {
        let phase = SeasonPhase::from_carousel(&carousel(vec![round(
            4,
            "Stanley-Cup-Final",
            vec![series(4, "FLA", 4, "EDM", 2)],
        )]));
        assert_eq!(
            phase,
            SeasonPhase::Over {
                champion: "FLA".into(),
                runner_up: "EDM".into(),
                series_label: "4-2".into(),
            }
        );
        assert!(phase.is_over());
    }

    #[test]
    fn final_in_progress_is_not_over() {
        let phase = SeasonPhase::from_carousel(&carousel(vec![round(
            4,
            "Stanley-Cup-Final",
            vec![series(4, "FLA", 3, "EDM", 2)],
        )]));
        assert_eq!(
            phase,
            SeasonPhase::InProgress {
                round_label: "Stanley Cup Final".into(),
                summary: "FLA leads EDM 3-2".into(),
            }
        );
        assert!(!phase.is_over());
    }

    #[test]
    fn tbd_final_falls_back_to_deepest_seeded_round() {
        let phase = SeasonPhase::from_carousel(&carousel(vec![
            round(3, "Conference-Finals", vec![series(3, "FLA", 2, "CAR", 2)]),
            round(4, "Stanley-Cup-Final", vec![series(4, "TBD", 0, "TBD", 0)]),
        ]));
        assert_eq!(
            phase,
            SeasonPhase::InProgress {
                round_label: "Conference Finals".into(),
                summary: "FLA and CAR tied 2-2".into(),
            }
        );
    }
}
