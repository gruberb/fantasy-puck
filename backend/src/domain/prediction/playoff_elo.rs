//! Dynamic playoff Elo — a team-strength rating that updates after every
//! completed playoff game instead of freezing at the regular-season mark.
//!
//! Pure-domain module: no DB, no HTTP, no framework deps. The
//! database-backed replay loop lives in `infra::prediction::compute_current_elo`
//! and calls into [`seed_from_standings`] + [`apply_game`] here.
//!
//! Algorithm:
//! 1. Seed each team's Elo from regular-season standings points:
//!    `elo_0 = BASE + POINTS_SCALE * (season_points - league_avg_points)`.
//!    For a 25-point RS spread at `POINTS_SCALE = 6` that's a ±75-point
//!    Elo window around the base 1500 — roughly how the public NHL Elo
//!    trackers seed.
//! 2. Replay every completed playoff game in chronological order,
//!    applying the standard logistic-Elo update with home-ice added to
//!    the home team's pre-game rating:
//!      `p_home = 1 / (1 + 10^(-(elo_home - elo_away + HOME_ICE_ADV) / 400))`
//!      `elo_new = elo_old + K * ln(|goal_diff| + 1) * (result - p_home)`
//!    The `ln(goal_diff + 1)` factor is the Silver-style blowout bonus
//!    so a 6-1 win moves ratings more than a 2-1 win, but with
//!    diminishing returns.

use std::collections::HashMap;

/// Starting Elo for a league-average team.
pub const BASE_ELO: f32 = 1500.0;
/// How many Elo points per RS-standings-point of separation above / below
/// the league average. 6.0 gives a ~150-point window across the RS, which
/// yields round-1 win probabilities within a few points of public models.
pub const POINTS_SCALE: f32 = 6.0;
/// Home-ice rating bonus applied to the home team before each game's
/// probability draw. 35 points ≈ 54/46 home/road split at equal Elo.
pub const HOME_ICE_ADV: f32 = 35.0;
/// Base update rate. Scaled per-game by `ln(|goal_diff| + 1)` so a 1-goal
/// win moves ratings by ~K and a 5-goal win by ~K·ln(6) ≈ 1.8·K.
pub const K_FACTOR: f32 = 6.0;

/// Compute each team's per-team home-ice advantage in Elo units from
/// their regular-season home-vs-road record. A team that earned a
/// points-percentage 8 pp higher at home than on the road gets a
/// ~32-point Elo home bonus; flat home/road split gets zero. Clamped
/// to `[10, 80]` to smooth small-sample noise and avoid negative
/// home-ice (rare but possible in a short season).
///
/// Scale rationale: on the Elo side `sigmoid(ELO_K * 35) ≈ 0.55`
/// (the 54/46 league-average home-ice split). Points-percentage gap
/// of ~0.08 maps to roughly the same win-prob shift, so the linear
/// coefficient is `35 / 0.08 ≈ 437`; we use 400 for a round number.
pub fn home_bonus_from_standings(standings: &serde_json::Value) -> HashMap<String, f32> {
    let Some(arr) = standings.get("standings").and_then(|v| v.as_array()) else {
        return HashMap::new();
    };
    arr.iter()
        .filter_map(|entry| {
            let abbrev = entry
                .get("teamAbbrev")
                .and_then(|a| a.get("default"))
                .and_then(|a| a.as_str())?
                .to_string();

            let pts_pct = |w_key: &str, l_key: &str, otl_key: &str| -> Option<f32> {
                let w = entry.get(w_key).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let l = entry.get(l_key).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let otl = entry.get(otl_key).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let gp = w + l + otl;
                if gp < 5.0 {
                    return None;
                }
                Some(((w * 2.0 + otl) / (gp * 2.0)) as f32)
            };

            let home = pts_pct("homeWins", "homeLosses", "homeOtLosses")?;
            let road = pts_pct("roadWins", "roadLosses", "roadOtLosses")?;
            let raw = (home - road) * 400.0;
            // Clamp: zero floor (no negative home-ice), 80-Elo ceiling
            // so a hot-home / cold-road small sample can't dominate
            // the per-game draw.
            let bonus = raw.clamp(10.0, 80.0);
            Some((abbrev, bonus))
        })
        .collect()
}

/// Seed each team's Elo from the NHL standings feed. Teams missing from
/// the feed get `BASE_ELO`. Returns `abbrev → elo`.
pub fn seed_from_standings(standings: &serde_json::Value) -> HashMap<String, f32> {
    let Some(arr) = standings.get("standings").and_then(|v| v.as_array()) else {
        return HashMap::new();
    };
    let points: Vec<(String, f32)> = arr
        .iter()
        .filter_map(|entry| {
            let abbrev = entry
                .get("teamAbbrev")
                .and_then(|a| a.get("default"))
                .and_then(|a| a.as_str())?
                .to_string();
            let pts = entry.get("points").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            Some((abbrev, pts))
        })
        .collect();
    if points.is_empty() {
        return HashMap::new();
    }
    let avg: f32 = points.iter().map(|(_, p)| *p).sum::<f32>() / points.len() as f32;
    points
        .into_iter()
        .map(|(abbrev, pts)| (abbrev, BASE_ELO + POINTS_SCALE * (pts - avg)))
        .collect()
}

/// A single completed game as the Elo updater needs it. Thin mirror of
/// the `playoff_game_results` columns we care about.
#[derive(Debug, Clone)]
pub struct GameResult {
    pub home_team: String,
    pub away_team: String,
    pub home_score: i32,
    pub away_score: i32,
}

/// Apply a single game update in place. Exposed for tests and the
/// backtest harness; production callers use
/// `infra::prediction::compute_current_elo` which loads games from the
/// DB and folds them through this function.
pub fn apply_game(ratings: &mut HashMap<String, f32>, game: &GameResult) {
    let elo_home = *ratings.get(&game.home_team).unwrap_or(&BASE_ELO);
    let elo_away = *ratings.get(&game.away_team).unwrap_or(&BASE_ELO);
    let diff = elo_home - elo_away + HOME_ICE_ADV;
    let p_home = 1.0 / (1.0 + (10.0_f32).powf(-diff / 400.0));

    let home_won = game.home_score > game.away_score;
    let result_home: f32 = if home_won { 1.0 } else { 0.0 };
    let goal_diff = (game.home_score - game.away_score).abs() as f32;
    let margin_mult = (goal_diff + 1.0).ln();

    let delta = K_FACTOR * margin_mult * (result_home - p_home);
    ratings.insert(game.home_team.clone(), elo_home + delta);
    ratings.insert(game.away_team.clone(), elo_away - delta);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(abbrev: &str, points: i64) -> serde_json::Value {
        json!({
            "teamAbbrev": { "default": abbrev },
            "points": points,
        })
    }

    #[test]
    fn seed_centers_on_base_with_league_average() {
        let root = json!({
            "standings": [
                entry("A", 100),
                entry("B", 110),
                entry("C", 90),
            ],
        });
        let seed = seed_from_standings(&root);
        // Average is 100; each team's Elo = 1500 + 6 * (pts - 100).
        assert!((seed["A"] - BASE_ELO).abs() < 1e-3);
        assert!((seed["B"] - (BASE_ELO + 60.0)).abs() < 1e-3);
        assert!((seed["C"] - (BASE_ELO - 60.0)).abs() < 1e-3);
    }

    #[test]
    fn home_bonus_reflects_home_road_gap() {
        // Strong home, weak road: 30-5-5 home (65 pts / 40 games = .8125
        // pts/game), 5-30-5 road (.1875 pts/game). Diff ≈ 0.625 → 250 Elo
        // raw, clamped to 80.
        let root = serde_json::json!({
            "standings": [{
                "teamAbbrev": { "default": "HOT" },
                "homeWins": 30, "homeLosses": 5, "homeOtLosses": 5,
                "roadWins": 5, "roadLosses": 30, "roadOtLosses": 5,
            }]
        });
        let map = home_bonus_from_standings(&root);
        assert!((map["HOT"] - 80.0).abs() < 1e-3);

        // Flat home/road: equal split → 0 diff → clamped to floor 10.
        let root = serde_json::json!({
            "standings": [{
                "teamAbbrev": { "default": "EVEN" },
                "homeWins": 20, "homeLosses": 15, "homeOtLosses": 5,
                "roadWins": 20, "roadLosses": 15, "roadOtLosses": 5,
            }]
        });
        let map = home_bonus_from_standings(&root);
        assert!((map["EVEN"] - 10.0).abs() < 1e-3);

        // Missing home/road fields: team skipped, not defaulted.
        let root = serde_json::json!({
            "standings": [{ "teamAbbrev": { "default": "EMPTY" } }]
        });
        let map = home_bonus_from_standings(&root);
        assert!(!map.contains_key("EMPTY"));
    }

    #[test]
    fn seed_handles_empty_and_missing_fields() {
        let empty = seed_from_standings(&json!({}));
        assert!(empty.is_empty());
        let no_standings = seed_from_standings(&json!({ "standings": [] }));
        assert!(no_standings.is_empty());
    }

    #[test]
    fn apply_game_rewards_upset_winner_more() {
        let mut small_upset = HashMap::new();
        small_upset.insert("BOS".into(), 1600.0);
        small_upset.insert("BUF".into(), 1400.0);
        apply_game(
            &mut small_upset,
            &GameResult {
                home_team: "BUF".into(),
                away_team: "BOS".into(),
                home_score: 3,
                away_score: 2,
            },
        );
        let buf_gain = small_upset["BUF"] - 1400.0;

        let mut expected = HashMap::new();
        expected.insert("BOS".into(), 1600.0);
        expected.insert("BUF".into(), 1400.0);
        apply_game(
            &mut expected,
            &GameResult {
                home_team: "BOS".into(),
                away_team: "BUF".into(),
                home_score: 3,
                away_score: 2,
            },
        );
        let bos_gain = expected["BOS"] - 1600.0;

        // Underdog winning gains more than favorite winning — this is
        // the whole point of an Elo over a fixed strength rating.
        assert!(
            buf_gain > bos_gain,
            "upset should pay more: upset_gain={buf_gain}, favorite_gain={bos_gain}"
        );
    }

    #[test]
    fn apply_game_rewards_bigger_blowout() {
        let mut close = HashMap::new();
        close.insert("A".into(), 1500.0);
        close.insert("B".into(), 1500.0);
        apply_game(
            &mut close,
            &GameResult {
                home_team: "A".into(),
                away_team: "B".into(),
                home_score: 3,
                away_score: 2,
            },
        );
        let close_gain = close["A"] - 1500.0;

        let mut blowout = HashMap::new();
        blowout.insert("A".into(), 1500.0);
        blowout.insert("B".into(), 1500.0);
        apply_game(
            &mut blowout,
            &GameResult {
                home_team: "A".into(),
                away_team: "B".into(),
                home_score: 7,
                away_score: 1,
            },
        );
        let blowout_gain = blowout["A"] - 1500.0;

        assert!(
            blowout_gain > close_gain,
            "6-goal blowout should move Elo more than 1-goal win: \
             blowout={blowout_gain}, close={close_gain}"
        );
    }

    #[test]
    fn elo_is_zero_sum_within_a_game() {
        let mut ratings = HashMap::new();
        ratings.insert("X".into(), 1500.0);
        ratings.insert("Y".into(), 1500.0);
        apply_game(
            &mut ratings,
            &GameResult {
                home_team: "X".into(),
                away_team: "Y".into(),
                home_score: 4,
                away_score: 2,
            },
        );
        let total = ratings["X"] + ratings["Y"];
        assert!(
            (total - 3000.0).abs() < 1e-3,
            "winner gain + loser loss must net to zero, got {total}"
        );
    }

    #[test]
    fn home_ice_advantage_helps_the_home_team() {
        // At equal pre-game Elo, a home win earns less than an away win
        // (the home team was already favored by HOME_ICE_ADV).
        let mut home_win = HashMap::new();
        home_win.insert("A".into(), 1500.0);
        home_win.insert("B".into(), 1500.0);
        apply_game(
            &mut home_win,
            &GameResult {
                home_team: "A".into(),
                away_team: "B".into(),
                home_score: 3,
                away_score: 2,
            },
        );
        let home_gain = home_win["A"] - 1500.0;

        let mut away_win = HashMap::new();
        away_win.insert("A".into(), 1500.0);
        away_win.insert("B".into(), 1500.0);
        apply_game(
            &mut away_win,
            &GameResult {
                home_team: "A".into(),
                away_team: "B".into(),
                home_score: 2,
                away_score: 3,
            },
        );
        let away_gain = away_win["B"] - 1500.0;
        assert!(
            away_gain > home_gain,
            "away winner should gain more than home winner at equal Elo: \
             away={away_gain}, home={home_gain}"
        );
    }
}
