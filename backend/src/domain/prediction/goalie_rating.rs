//! Per-team goalie-strength component for the team-rating model.
//!
//! Pure-domain module: no DB, no HTTP, no framework deps. Callers
//! (the race-odds handler) build a `Vec<GoalieEntry>` from the NHL API
//! `goalie-stats-leaders` payload and hand it off to
//! [`compute_bonuses`].
//!
//! Why this matters: MoneyPuck weights goaltending at ~29% of team
//! strength. The v1.14 engine weighted it at 0. The biggest single
//! source of playoff variance we were ignoring was the identity and
//! quality of each team's starter. This module converts each team's
//! primary goalie's season SV% into an Elo delta on top of the
//! standings-derived base rating.
//!
//! Primary goalie = goalie with the most wins for their team in the
//! current season. Two-headed tandems (≥ 2 goalies within 3 wins)
//! split the credit. Backups with a handful of games are ignored.
//!
//! Bonus formula:
//!   `bonus_elo = ((sv_pct - LEAGUE_AVG_SVP) * GOALIE_BONUS_SCALE).clamp(±GOALIE_BONUS_CLAMP)`
//!
//! A .925 SV% starter vs a .905 league average = +16 Elo. A .895
//! starter = −8 Elo. The clamp at ±30 caps out at a .9425 or .8675
//! starter; empirically no modern NHL season produces a team-level
//! starter outside that band over a full 82.

use std::collections::HashMap;

/// NHL season-average starter save percentage. Stable around .905
/// year-to-year; adjust if the league trends materially.
pub const LEAGUE_AVG_SVP: f32 = 0.905;

/// How many Elo points per SV% above / below league average.
/// `(sv_pct - LEAGUE_AVG_SVP)` for a .925 starter is 0.020; × 800 = 16
/// Elo. A .940 starter (elite) is 0.035 × 800 = 28 Elo. Caps at
/// GOALIE_BONUS_CLAMP.
pub const GOALIE_BONUS_SCALE: f32 = 800.0;

/// Max Elo swing from the goalie component. Keeps a small-sample
/// anomaly (a goalie who ran .950 over 12 starts) from dominating the
/// per-game draw.
pub const GOALIE_BONUS_CLAMP: f32 = 30.0;

/// Minimum wins before a goalie is considered the starter. Stops a
/// one-start callup from claiming the team bonus when the actual
/// starter is injured and missing from the leaderboard.
pub const MIN_WINS_FOR_STARTER: f32 = 3.0;

/// A pure-domain projection of the NHL API goalie leaderboard entry.
/// Handlers build these from `models::nhl::GoalieStatsLeaders`.
#[derive(Debug, Clone)]
pub struct GoalieEntry {
    pub player_id: i64,
    pub team_abbrev: String,
    /// Season wins. Used to identify each team's primary starter.
    pub wins: f32,
    /// Season save percentage. `None` when the goalie isn't in the
    /// save_pctg leaderboard (e.g. fewer starts than the endpoint's
    /// minimum).
    pub save_pct: Option<f32>,
}

/// Compute each team's goalie-bonus map.
///
/// Input: every goalie across all teams, with their wins and SV%.
/// Output: `team_abbrev → bonus_elo` for teams where we can identify a
/// credible starter. Teams whose starter has < MIN_WINS_FOR_STARTER
/// wins OR no recorded save_pct contribute no entry — callers fall
/// back to a zero bonus.
///
/// When two goalies are within 3 wins of each other for the same
/// team, we treat it as a tandem and average their bonuses rather
/// than picking one. Matters for teams like BOS (Swayman / Ullmark)
/// or NYR historically.
pub fn compute_bonuses(entries: &[GoalieEntry]) -> HashMap<String, f32> {
    // Bucket per team, keep only entries with a save_pct and at least
    // MIN_WINS_FOR_STARTER wins — backups and callups can't carry the
    // signal.
    let mut per_team: HashMap<String, Vec<&GoalieEntry>> = HashMap::new();
    for e in entries {
        if e.save_pct.is_none() || e.wins < MIN_WINS_FOR_STARTER {
            continue;
        }
        per_team.entry(e.team_abbrev.clone()).or_default().push(e);
    }

    let mut out: HashMap<String, f32> = HashMap::with_capacity(per_team.len());
    for (team, mut goalies) in per_team {
        // Sort DESC by wins. Ties broken by higher save_pct (prefer
        // the more efficient goalie as the nominal starter when wins
        // are equal).
        goalies.sort_by(|a, b| {
            b.wins
                .partial_cmp(&a.wins)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.save_pct
                        .unwrap_or(0.0)
                        .partial_cmp(&a.save_pct.unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        let primary = goalies[0];

        // Tandem check: a secondary with wins within 3 of the primary
        // gets averaged in. Anything further back is a backup.
        let tandem: Vec<&GoalieEntry> = goalies
            .iter()
            .copied()
            .filter(|g| primary.wins - g.wins <= 3.0)
            .collect();

        let bonus_sum: f32 = tandem
            .iter()
            .map(|g| bonus_for_svp(g.save_pct.unwrap_or(LEAGUE_AVG_SVP)))
            .sum();
        let bonus = bonus_sum / tandem.len() as f32;
        out.insert(team, bonus);
    }
    out
}

/// Pure conversion: save-percentage → clamped Elo bonus. Exposed for
/// unit tests and to let callers spot-check a specific goalie.
pub fn bonus_for_svp(sv_pct: f32) -> f32 {
    ((sv_pct - LEAGUE_AVG_SVP) * GOALIE_BONUS_SCALE)
        .clamp(-GOALIE_BONUS_CLAMP, GOALIE_BONUS_CLAMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(player_id: i64, team: &str, wins: f32, sv: Option<f32>) -> GoalieEntry {
        GoalieEntry {
            player_id,
            team_abbrev: team.into(),
            wins,
            save_pct: sv,
        }
    }

    #[test]
    fn elite_starter_gets_positive_bonus_below_clamp() {
        let b = bonus_for_svp(0.925);
        // (0.925 - 0.905) * 800 = 16
        assert!((b - 16.0).abs() < 1e-4, "got {b}");
    }

    #[test]
    fn below_league_starter_gets_negative_bonus() {
        let b = bonus_for_svp(0.895);
        // (-0.010) * 800 = -8
        assert!((b + 8.0).abs() < 1e-4, "got {b}");
    }

    #[test]
    fn extreme_sv_pct_clamps_to_cap() {
        // .950 → (0.045 * 800) = 36, clamp to 30
        let high = bonus_for_svp(0.950);
        assert!((high - GOALIE_BONUS_CLAMP).abs() < 1e-4);

        // .860 → (-.045 * 800) = -36, clamp to -30
        let low = bonus_for_svp(0.860);
        assert!((low + GOALIE_BONUS_CLAMP).abs() < 1e-4);
    }

    #[test]
    fn single_starter_team_uses_their_bonus() {
        let entries = vec![entry(1, "BOS", 30.0, Some(0.925))];
        let bonuses = compute_bonuses(&entries);
        assert!((bonuses["BOS"] - 16.0).abs() < 1e-4);
    }

    #[test]
    fn backups_and_callups_are_filtered_out() {
        // The nominal starter is a .935 workhorse; the backup spot-
        // started twice and can't credibly carry the signal. We expect
        // only the starter to count.
        let entries = vec![
            entry(1, "DAL", 25.0, Some(0.925)),
            entry(2, "DAL", 2.0, Some(0.860)),
        ];
        let bonuses = compute_bonuses(&entries);
        assert!((bonuses["DAL"] - 16.0).abs() < 1e-4);
    }

    #[test]
    fn tandem_averages_both_goalies() {
        // Swayman/Ullmark-style split. Both within 3 wins; both have
        // SV%; tandem average should be between the two.
        let entries = vec![
            entry(1, "BOS", 20.0, Some(0.935)), // bonus 24
            entry(2, "BOS", 18.0, Some(0.915)), // bonus 8
        ];
        let bonuses = compute_bonuses(&entries);
        assert!((bonuses["BOS"] - 16.0).abs() < 1e-4, "got {}", bonuses["BOS"]);
    }

    #[test]
    fn missing_save_pct_yields_no_bonus_entry() {
        // A team whose starter isn't in the save_pctg leaderboard
        // (fewer starts than the endpoint's minimum) should simply be
        // absent from the output map — callers fall back to 0.
        let entries = vec![entry(1, "UTA", 25.0, None)];
        let bonuses = compute_bonuses(&entries);
        assert!(!bonuses.contains_key("UTA"));
    }

    #[test]
    fn wins_below_minimum_yields_no_bonus_entry() {
        let entries = vec![entry(1, "SJ", 1.0, Some(0.930))];
        let bonuses = compute_bonuses(&entries);
        assert!(!bonuses.contains_key("SJ"));
    }
}
