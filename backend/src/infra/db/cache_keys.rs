//! `response_cache` key formats. Handlers write these keys and the
//! pollers invalidate them with LIKE patterns, so both sides build them
//! here. Bump a family's version segment when its payload shape or
//! model changes, so a deploy doesn't serve same-day entries written by
//! the old code.

/// Race-odds model version. v4 sums playoff points from the full
/// per-game mirror instead of the top-25 leaderboard.
const RACE_ODDS_VERSION: &str = "v4";

pub fn insights(league_id: &str, season: u32, game_type: u8, date: &str) -> String {
    format!("insights:{league_id}:{season}:{game_type}:{date}")
}

/// LIKE pattern matching every league's insights for `date`.
pub fn insights_for_date(season: u32, game_type: u8, date: &str) -> String {
    format!("insights:%:{season}:{game_type}:{date}")
}

pub fn team_diagnosis(
    league_id: &str,
    team_id: i64,
    season: u32,
    game_type: u8,
    date: &str,
) -> String {
    format!("team_diagnosis:{league_id}:{team_id}:{season}:{game_type}:{date}:v2")
}

/// LIKE pattern matching every team-diagnosis narrative in a league.
/// Deliberately excludes the Pulse bundle, which carries its own suffix.
pub fn team_diagnosis_for_league(league_id: &str) -> String {
    format!("team_diagnosis:{league_id}:%:v2")
}

pub fn team_diagnosis_bundle(
    league_id: &str,
    team_id: i64,
    season: u32,
    game_type: u8,
    date: &str,
) -> String {
    format!("team_diagnosis:{league_id}:{team_id}:{season}:{game_type}:{date}:bundle:v1")
}

/// An empty `league_id` is the global (champion-mode) board.
pub fn race_odds(league_id: &str, season: u32, game_type: u8, date: &str) -> String {
    let scope = if league_id.is_empty() {
        "global"
    } else {
        league_id
    };
    format!("race_odds:{RACE_ODDS_VERSION}:{scope}:{season}:{game_type}:{date}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Postgres LIKE with only `%` wildcards, which is all these patterns use.
    fn like(pattern: &str, value: &str) -> bool {
        let parts: Vec<&str> = pattern.split('%').collect();
        let (first, rest) = parts.split_first().unwrap();
        let Some(mut tail) = value.strip_prefix(first) else {
            return false;
        };
        for (i, part) in rest.iter().enumerate() {
            if i == rest.len() - 1 {
                return tail.ends_with(part);
            }
            match tail.find(part) {
                Some(at) => tail = &tail[at + part.len()..],
                None => return false,
            }
        }
        tail.is_empty()
    }

    #[test]
    fn insights_pattern_matches_league_and_global_keys() {
        let pattern = insights_for_date(20252026, 3, "2026-05-01");
        assert!(like(&pattern, &insights("abc", 20252026, 3, "2026-05-01")));
        assert!(like(&pattern, &insights("", 20252026, 3, "2026-05-01")));
        assert!(!like(&pattern, &insights("abc", 20252026, 3, "2026-05-02")));
    }

    #[test]
    fn team_diagnosis_pattern_skips_pulse_bundle() {
        let pattern = team_diagnosis_for_league("abc");
        assert!(like(
            &pattern,
            &team_diagnosis("abc", 7, 20252026, 3, "2026-05-01")
        ));
        assert!(!like(
            &pattern,
            &team_diagnosis_bundle("abc", 7, 20252026, 3, "2026-05-01")
        ));
        assert!(!like(
            &pattern,
            &team_diagnosis("xyz", 7, 20252026, 3, "2026-05-01")
        ));
    }
}
