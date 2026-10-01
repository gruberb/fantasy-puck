//! Pure helpers over `PlayoffCarousel`. Shared by the race-sim
//! handler and the fantasy-team breakdown handler, which both need to
//! know how many playoff games each NHL team has played so far.

use std::collections::{BTreeSet, HashMap};

use crate::domain::models::nhl::{PlayoffCarousel, Series};

/// A best-of-7 needs 4 wins; treat a missing/zero `needed_to_win` as 4 so a
/// malformed carousel row can't be read as "already clinched" at 0-0.
pub fn games_to_clinch(series: &Series) -> i64 {
    if series.needed_to_win > 0 {
        series.needed_to_win
    } else {
        4
    }
}

/// `(winner, loser)` abbrevs once either side has clinched.
pub fn series_winner(series: &Series) -> Option<(&str, &str)> {
    let needed = games_to_clinch(series);
    let (top, bottom) = (&series.top_seed, &series.bottom_seed);
    if top.wins >= needed {
        Some((&top.abbrev, &bottom.abbrev))
    } else if bottom.wins >= needed {
        Some((&bottom.abbrev, &top.abbrev))
    } else {
        None
    }
}

/// Team sets derived from the bracket, sorted for stable API output.
#[derive(Debug, Default, PartialEq)]
pub struct BracketState {
    /// Every team that appears anywhere in the bracket.
    pub participants: BTreeSet<String>,
    pub eliminated: BTreeSet<String>,
    /// Participants not yet eliminated.
    pub alive: BTreeSet<String>,
    /// Alive teams that have won at least one series.
    pub advanced: BTreeSet<String>,
}

pub fn bracket_state(carousel: &PlayoffCarousel) -> BracketState {
    let mut state = BracketState::default();
    for series in carousel.rounds.iter().flat_map(|r| &r.series) {
        state.participants.insert(series.top_seed.abbrev.clone());
        state.participants.insert(series.bottom_seed.abbrev.clone());
        if let Some((winner, loser)) = series_winner(series) {
            state.advanced.insert(winner.to_string());
            state.eliminated.insert(loser.to_string());
        }
    }
    state.alive = &state.participants - &state.eliminated;
    state.advanced = &state.advanced - &state.eliminated;
    state
}

/// Sum of games played across every series in the carousel, keyed by
/// team abbrev. Each team in a series has played the same number of
/// games (`top_wins + bottom_wins`), so both sides get the same
/// contribution added. Teams that have completed one round and are
/// partway through the next accumulate across both.
pub fn games_played_from_carousel(carousel: Option<&PlayoffCarousel>) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let Some(c) = carousel else {
        return map;
    };
    for round in &c.rounds {
        for s in &round.series {
            let games = (s.top_seed.wins + s.bottom_seed.wins).max(0) as u32;
            *map.entry(s.top_seed.abbrev.clone()).or_insert(0) += games;
            *map.entry(s.bottom_seed.abbrev.clone()).or_insert(0) += games;
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::nhl::{BottomSeed, Round, Series, TopSeed};

    fn s(top: &str, top_wins: i64, bot: &str, bot_wins: i64) -> Series {
        Series {
            series_letter: "A".into(),
            round_number: 1,
            series_label: String::new(),
            series_link: String::new(),
            top_seed: TopSeed {
                id: 0,
                abbrev: top.into(),
                wins: top_wins,
                logo: String::new(),
                dark_logo: String::new(),
            },
            bottom_seed: BottomSeed {
                id: 0,
                abbrev: bot.into(),
                wins: bot_wins,
                logo: String::new(),
                dark_logo: String::new(),
            },
            needed_to_win: 4,
        }
    }

    #[test]
    fn empty_carousel_returns_empty_map() {
        assert!(games_played_from_carousel(None).is_empty());
    }

    #[test]
    fn sums_series_games_per_team() {
        let carousel = PlayoffCarousel {
            season_id: 20252026,
            current_round: 1,
            rounds: vec![Round {
                round_number: 1,
                round_label: String::new(),
                round_abbrev: String::new(),
                series: vec![s("BUF", 2, "BOS", 1), s("EDM", 1, "ANA", 0)],
            }],
        };
        let m = games_played_from_carousel(Some(&carousel));
        assert_eq!(m.get("BUF"), Some(&3));
        assert_eq!(m.get("BOS"), Some(&3));
        assert_eq!(m.get("EDM"), Some(&1));
        assert_eq!(m.get("ANA"), Some(&1));
    }

    #[test]
    fn team_appearing_in_multiple_rounds_accumulates() {
        let carousel = PlayoffCarousel {
            season_id: 20252026,
            current_round: 2,
            rounds: vec![
                Round {
                    round_number: 1,
                    round_label: String::new(),
                    round_abbrev: String::new(),
                    series: vec![s("BUF", 4, "BOS", 2)],
                },
                Round {
                    round_number: 2,
                    round_label: String::new(),
                    round_abbrev: String::new(),
                    series: vec![s("BUF", 1, "TOR", 2)],
                },
            ],
        };
        let m = games_played_from_carousel(Some(&carousel));
        assert_eq!(m.get("BUF"), Some(&(6 + 3)));
    }

    fn set(teams: &[&str]) -> BTreeSet<String> {
        teams.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn bracket_state_tracks_eliminations_across_rounds() {
        let carousel = PlayoffCarousel {
            season_id: 20252026,
            current_round: 2,
            rounds: vec![
                Round {
                    round_number: 1,
                    round_label: String::new(),
                    round_abbrev: String::new(),
                    series: vec![s("BUF", 4, "BOS", 2), s("TOR", 1, "OTT", 4)],
                },
                Round {
                    round_number: 2,
                    round_label: String::new(),
                    round_abbrev: String::new(),
                    series: vec![s("BUF", 4, "OTT", 3)],
                },
            ],
        };
        let state = bracket_state(&carousel);
        assert_eq!(state.participants, set(&["BOS", "BUF", "OTT", "TOR"]));
        assert_eq!(state.eliminated, set(&["BOS", "OTT", "TOR"]));
        assert_eq!(state.alive, set(&["BUF"]));
        assert_eq!(state.advanced, set(&["BUF"]));
    }

    #[test]
    fn zero_needed_to_win_does_not_clinch_at_zero() {
        let mut series = s("BUF", 0, "BOS", 0);
        series.needed_to_win = 0;
        assert_eq!(series_winner(&series), None);
        series.top_seed.wins = 4;
        assert_eq!(series_winner(&series), Some(("BUF", "BOS")));
    }
}
