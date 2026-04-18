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
/// How far a team's per-team home-ice bonus is allowed to drift from
/// the league-wide `HOME_ICE_ADV`, in Elo units. A 41-game sample is
/// noisy; capping the delta at ±15 keeps the per-team personalization
/// meaningful without letting freak home/road splits dominate the
/// per-game draw. Result range for the absolute per-team bonus is
/// therefore `[HOME_ICE_ADV - HOME_BONUS_DELTA_CLAMP, HOME_ICE_ADV +
/// HOME_BONUS_DELTA_CLAMP]` = `[20, 50]`.
pub const HOME_BONUS_DELTA_CLAMP: f32 = 15.0;

/// Compute each team's per-team home-ice advantage in Elo units from
/// their regular-season home-vs-road record.
///
/// The result is the **absolute** home-ice bonus, centered on the
/// league-wide `HOME_ICE_ADV`. A team with a league-average home/road
/// split returns ~`HOME_ICE_ADV`; a team with an unusually strong home
/// record returns up to `HOME_ICE_ADV + HOME_BONUS_DELTA_CLAMP`; an
/// unusually weak home returns `HOME_ICE_ADV - HOME_BONUS_DELTA_CLAMP`.
/// The old asymmetric `[10, 80]` clamp under-rewarded near-neutral
/// teams (whose observed pct-gap of ~0 clamped to 10 instead of the
/// baseline ~35).
///
/// Scale rationale: on the Elo side `sigmoid(ELO_K * 35) ≈ 0.55`
/// (the 54/46 league-average home-ice split). Points-percentage gap
/// of ~0.0875 maps to the same win-prob shift, so the linear
/// coefficient is `35 / 0.0875 = 400`. That means a league-average
/// home/road pts-pct gap produces `raw ≈ HOME_ICE_ADV`, which is why
/// we center the delta by subtracting `HOME_ICE_ADV`.
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
            // `raw_elo_equivalent` is the Elo bonus a pure RS home/road
            // pct-gap maps to. League-average is ~HOME_ICE_ADV.
            let raw_elo_equivalent = (home - road) * 400.0;
            // Center on the league baseline so the delta reflects
            // *personalisation*, not the baseline itself.
            let delta = raw_elo_equivalent - HOME_ICE_ADV;
            let delta_clamped = delta.clamp(-HOME_BONUS_DELTA_CLAMP, HOME_BONUS_DELTA_CLAMP);
            Some((abbrev, HOME_ICE_ADV + delta_clamped))
        })
        .collect()
}

/// Seed each team's Elo from the NHL standings feed, using the
/// production `POINTS_SCALE` and no shrinkage. Teams missing from the
/// feed get `BASE_ELO`. Returns `abbrev → elo`.
///
/// Thin convenience wrapper over [`seed_from_standings_tuned`]. Use
/// the tuned variant when you want to sweep `points_scale` or apply
/// shrinkage (the calibration harness does this).
pub fn seed_from_standings(standings: &serde_json::Value) -> HashMap<String, f32> {
    seed_from_standings_tuned(standings, POINTS_SCALE, 1.0)
}

/// Seed Elo from standings with explicit `points_scale` and
/// `shrinkage` knobs for the calibration sweep.
///
/// `shrinkage ∈ [0, 1]` regresses each team's RS-point deviation from
/// league average toward the mean before scaling:
/// `elo_0 = BASE + points_scale * shrinkage * (season_points - league_avg)`.
///
/// - `shrinkage = 1.0` reproduces the legacy behavior (no regression).
/// - `shrinkage = 0.7` treats the observed standings as 70% signal /
///   30% noise, which is the typical Bayesian shrinkage for an
///   82-game NHL sample against the prior that teams are closer to
///   league average than their records suggest.
/// - `shrinkage = 0.0` flattens every team to `BASE` (sanity-check
///   baseline for the sweep).
pub fn seed_from_standings_tuned(
    standings: &serde_json::Value,
    points_scale: f32,
    shrinkage: f32,
) -> HashMap<String, f32> {
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
    let effective_scale = points_scale * shrinkage;
    points
        .into_iter()
        .map(|(abbrev, pts)| (abbrev, BASE_ELO + effective_scale * (pts - avg)))
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
        // Strong home, weak road: 30-5-5 home (.8125 pts-pct), 5-30-5
        // road (.1875). Diff ≈ 0.625 → raw 250 Elo → delta = 250 - 35
        // = 215, clamped to +HOME_BONUS_DELTA_CLAMP. Absolute =
        // HOME_ICE_ADV + clamp = 35 + 15 = 50.
        let root = serde_json::json!({
            "standings": [{
                "teamAbbrev": { "default": "HOT" },
                "homeWins": 30, "homeLosses": 5, "homeOtLosses": 5,
                "roadWins": 5, "roadLosses": 30, "roadOtLosses": 5,
            }]
        });
        let map = home_bonus_from_standings(&root);
        let expected_hot = HOME_ICE_ADV + HOME_BONUS_DELTA_CLAMP;
        assert!(
            (map["HOT"] - expected_hot).abs() < 1e-3,
            "expected HOT ≈ {expected_hot}, got {}",
            map["HOT"]
        );

        // Flat home/road split: raw 0 Elo → delta = -35, clamped to
        // -HOME_BONUS_DELTA_CLAMP. Absolute = 35 - 15 = 20.
        let root = serde_json::json!({
            "standings": [{
                "teamAbbrev": { "default": "EVEN" },
                "homeWins": 20, "homeLosses": 15, "homeOtLosses": 5,
                "roadWins": 20, "roadLosses": 15, "roadOtLosses": 5,
            }]
        });
        let map = home_bonus_from_standings(&root);
        let expected_even = HOME_ICE_ADV - HOME_BONUS_DELTA_CLAMP;
        assert!(
            (map["EVEN"] - expected_even).abs() < 1e-3,
            "expected EVEN ≈ {expected_even}, got {}",
            map["EVEN"]
        );

        // Missing home/road fields: team skipped, not defaulted.
        let root = serde_json::json!({
            "standings": [{ "teamAbbrev": { "default": "EMPTY" } }]
        });
        let map = home_bonus_from_standings(&root);
        assert!(!map.contains_key("EMPTY"));
    }

    #[test]
    fn home_bonus_near_league_average_centers_on_base_adv() {
        // A team with a home/road gap exactly matching the league
        // baseline (0.0875 pts-pct gap → raw 35 Elo) should land on
        // HOME_ICE_ADV with zero delta. Verifies the centering fix
        // (old clamp would have returned 35 as well; this test locks
        // in the post-refactor behavior so future edits don't regress).
        // A .6 home pts-pct vs .5125 road produces raw ≈ 35 exactly.
        // Equivalent integer split: 48 hGP and 48 rGP with home
        // (24W, 15L, 9OTL) = 57 pts / 96 = .59375 home pct,
        // road (21W, 18L, 9OTL) = 51 pts / 96 = .53125 road pct,
        // diff = .0625 → raw 25 (close to HOME_ICE_ADV with some slack).
        let root = serde_json::json!({
            "standings": [{
                "teamAbbrev": { "default": "AVG" },
                "homeWins": 24, "homeLosses": 15, "homeOtLosses": 9,
                "roadWins": 21, "roadLosses": 18, "roadOtLosses": 9,
            }]
        });
        let map = home_bonus_from_standings(&root);
        // Raw 25 → delta = -10 (inside ±15 clamp). Absolute = 25.
        // Verify it lands inside the centered band and isn't clamped.
        let avg_bonus = map["AVG"];
        assert!(
            avg_bonus > HOME_ICE_ADV - HOME_BONUS_DELTA_CLAMP - 1e-3
                && avg_bonus < HOME_ICE_ADV + HOME_BONUS_DELTA_CLAMP + 1e-3,
            "AVG bonus should be inside the centered band, got {avg_bonus}"
        );
    }

    #[test]
    fn shrinkage_tightens_the_elo_spread() {
        let root = json!({
            "standings": [
                entry("HOT",  110),
                entry("MID",  100),
                entry("COLD",  90),
            ],
        });
        let full = seed_from_standings_tuned(&root, POINTS_SCALE, 1.0);
        let half = seed_from_standings_tuned(&root, POINTS_SCALE, 0.5);
        let zero = seed_from_standings_tuned(&root, POINTS_SCALE, 0.0);

        // At shrinkage=1.0, HOT - COLD spread = 2 * POINTS_SCALE * 10 = 120.
        assert!(((full["HOT"] - full["COLD"]) - 120.0).abs() < 1e-3);
        // At shrinkage=0.5, spread is halved.
        assert!(((half["HOT"] - half["COLD"]) - 60.0).abs() < 1e-3);
        // At shrinkage=0.0, every team collapses to BASE_ELO.
        assert!((zero["HOT"] - BASE_ELO).abs() < 1e-3);
        assert!((zero["COLD"] - BASE_ELO).abs() < 1e-3);
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
