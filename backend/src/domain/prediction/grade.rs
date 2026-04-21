//! Per-player performance grading against projection.
//!
//! Produces a letter grade A..F plus a bucket label that maps to the
//! `KEEP FAITH / FINE BUT FRAGILE / NEED A MIRACLE / PROBLEM ASSET`
//! rubric the UI shows on a fantasy roster. Pure domain: no DB, no
//! HTTP, no clock. All inputs are already-computed values the caller
//! assembles from the mirror and the Monte Carlo cache.

use serde::{Deserialize, Serialize};

use crate::domain::prediction::player_projection::Projection;
use crate::domain::prediction::race_sim::NB_DISPERSION;
use crate::domain::prediction::series_projection::SeriesStateCode;

/// Minimum playoff games played before the grader emits a letter.
/// Below this, we surface `NotEnoughData` so a single goose-egg doesn't
/// brand a player as cold.
pub const MIN_GAMES_FOR_GRADE: u32 = 2;

/// A demotion signal derived from [`Projection::toi_multiplier`]. Below
/// this threshold the player is clearly skating fewer minutes in recent
/// games than earlier in the run (the multiplier itself is clamped at
/// 0.70 on the floor).
pub const TOI_DEMOTION_THRESHOLD: f32 = 0.80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
    NotEnoughData,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeReport {
    pub grade: Grade,
    pub z_score: f32,
    pub expected_points: f32,
    pub actual_points: i32,
    pub games_played: u32,
}

/// Score a skater's actual playoff output against their projection.
///
/// The z-score uses a Negative-Binomial variance model that matches the
/// Monte Carlo's per-game draw: `Var(n·X) = n·λ·(1 + λ/r)` for a NegBin
/// with mean λ. The `.max(0.5)` floor keeps z finite for fringe skaters
/// whose expected output is near zero.
pub fn grade(ppg: f32, games_played: u32, actual_points: i32) -> GradeReport {
    let expected_points = ppg * games_played as f32;

    if games_played < MIN_GAMES_FOR_GRADE || ppg <= 0.0 {
        return GradeReport {
            grade: Grade::NotEnoughData,
            z_score: 0.0,
            expected_points,
            actual_points,
            games_played,
        };
    }

    let variance = (expected_points * (1.0 + ppg / NB_DISPERSION)).max(0.5);
    let z = (actual_points as f32 - expected_points) / variance.sqrt();

    let letter = if z >= 1.0 {
        Grade::A
    } else if z >= 0.3 {
        Grade::B
    } else if z >= -0.3 {
        Grade::C
    } else if z >= -1.0 {
        Grade::D
    } else {
        Grade::F
    };

    GradeReport {
        grade: letter,
        z_score: z,
        expected_points,
        actual_points,
        games_played,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemainingImpact {
    pub expected_remaining_games: f32,
    pub expected_remaining_points: f32,
    pub nhl_team_eliminated: bool,
}

/// Project how much a player can still contribute given their NHL
/// team's remaining playoff road.
///
/// `expected_games_total` is the Monte Carlo output for the player's
/// NHL team — the mean number of games that team plays across the
/// whole bracket. Pass `None` when the race-odds cache hasn't warmed;
/// the result zeros out rather than crashes the page.
pub fn remaining_impact(
    ppg: f32,
    expected_games_total: Option<f32>,
    team_games_already_played: u32,
    nhl_team_eliminated: bool,
) -> RemainingImpact {
    if nhl_team_eliminated {
        return RemainingImpact {
            expected_remaining_games: 0.0,
            expected_remaining_points: 0.0,
            nhl_team_eliminated: true,
        };
    }
    let Some(expected_total) = expected_games_total else {
        return RemainingImpact {
            expected_remaining_games: 0.0,
            expected_remaining_points: 0.0,
            nhl_team_eliminated: false,
        };
    };
    let remaining = (expected_total - team_games_already_played as f32).max(0.0);
    RemainingImpact {
        expected_remaining_games: remaining,
        expected_remaining_points: ppg * remaining,
        nhl_team_eliminated: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayerBucket {
    /// Below [`MIN_GAMES_FOR_GRADE`] playoff games — not enough
    /// evidence to grade yet. Renders as a neutral pill rather than
    /// the green "on expected" because we have no signal either way.
    TooEarly,
    /// Role strong, finishing slump. Don't panic.
    KeepFaith,
    /// Grade C, series alive, role stable.
    OnPace,
    /// Grade A or B.
    Outperforming,
    /// Moderate grade with a merely-adequate role.
    FineButFragile,
    /// Bad grade plus a demoted role. Needs a heater.
    NeedMiracle,
    /// Scratched, injured, or demoted off the depth chart.
    ProblemAsset,
    /// NHL team is out. Zero upside left.
    TeamEliminated,
}

/// Map the numeric signals to a label the UI renders as a pill.
///
/// Priority order matters: a player on an eliminated team is always
/// `TeamEliminated` regardless of grade, and a scratch is always
/// `ProblemAsset` regardless of series state.
pub fn classify_bucket(
    report: &GradeReport,
    projection: &Projection,
    series_state: SeriesStateCode,
) -> PlayerBucket {
    if series_state == SeriesStateCode::Eliminated {
        return PlayerBucket::TeamEliminated;
    }
    if projection.active_prob < 1.0 {
        return PlayerBucket::ProblemAsset;
    }
    if projection.toi_multiplier < TOI_DEMOTION_THRESHOLD {
        return PlayerBucket::ProblemAsset;
    }
    match report.grade {
        Grade::NotEnoughData => PlayerBucket::TooEarly,
        Grade::A | Grade::B => PlayerBucket::Outperforming,
        Grade::D | Grade::F
            if projection.toi_multiplier >= 0.9
                && matches!(
                    series_state,
                    SeriesStateCode::FacingElim | SeriesStateCode::Trailing
                ) =>
        {
            PlayerBucket::KeepFaith
        }
        Grade::F => PlayerBucket::NeedMiracle,
        Grade::D => PlayerBucket::FineButFragile,
        Grade::C => PlayerBucket::OnPace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj(ppg: f32, active: f32, toi_mult: f32) -> Projection {
        Projection {
            ppg,
            active_prob: active,
            toi_multiplier: toi_mult,
        }
    }

    #[test]
    fn not_enough_games_returns_not_enough_data() {
        let r = grade(0.6, 1, 0);
        assert_eq!(r.grade, Grade::NotEnoughData);
    }

    #[test]
    fn zero_projection_returns_not_enough_data() {
        let r = grade(0.0, 5, 0);
        assert_eq!(r.grade, Grade::NotEnoughData);
    }

    #[test]
    fn on_pace_hits_c() {
        // 5 games at 0.6 ppg → expected 3 pts. Actual 3 → z = 0.
        let r = grade(0.6, 5, 3);
        assert_eq!(r.grade, Grade::C);
        assert!(r.z_score.abs() < 0.3);
    }

    #[test]
    fn hot_player_gets_a() {
        // Expected 3, actual 7 → big positive z.
        let r = grade(0.6, 5, 7);
        assert_eq!(r.grade, Grade::A);
        assert!(r.z_score >= 1.0);
    }

    #[test]
    fn cold_star_gets_f() {
        // Expected 6 pts (1.2 ppg × 5 GP), actual 0 → deep negative z.
        let r = grade(1.2, 5, 0);
        assert_eq!(r.grade, Grade::F);
        assert!(r.z_score <= -1.0);
    }

    #[test]
    fn modestly_below_gets_d() {
        // Expected 3.0, actual 2 → z ≈ -0.54.
        let r = grade(0.6, 5, 2);
        assert_eq!(r.grade, Grade::D);
    }

    #[test]
    fn modestly_above_gets_b() {
        // Expected 3.0, actual 4 → z ≈ +0.54.
        let r = grade(0.6, 5, 4);
        assert_eq!(r.grade, Grade::B);
    }

    #[test]
    fn eliminated_team_zeros_remaining_impact() {
        let r = remaining_impact(1.0, Some(10.0), 4, true);
        assert_eq!(r.expected_remaining_games, 0.0);
        assert_eq!(r.expected_remaining_points, 0.0);
        assert!(r.nhl_team_eliminated);
    }

    #[test]
    fn missing_expected_games_zeros_remaining_impact() {
        let r = remaining_impact(1.0, None, 4, false);
        assert_eq!(r.expected_remaining_games, 0.0);
        assert_eq!(r.expected_remaining_points, 0.0);
        assert!(!r.nhl_team_eliminated);
    }

    #[test]
    fn remaining_impact_multiplies_ppg_by_remaining_games() {
        let r = remaining_impact(0.8, Some(16.0), 5, false);
        assert!((r.expected_remaining_games - 11.0).abs() < 1e-6);
        assert!((r.expected_remaining_points - 8.8).abs() < 1e-6);
    }

    #[test]
    fn remaining_impact_clamps_to_zero_when_already_past_expected() {
        let r = remaining_impact(0.8, Some(4.0), 7, false);
        assert_eq!(r.expected_remaining_games, 0.0);
        assert_eq!(r.expected_remaining_points, 0.0);
    }

    #[test]
    fn eliminated_team_always_team_eliminated_bucket() {
        let rep = grade(0.6, 5, 3);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 1.0), SeriesStateCode::Eliminated);
        assert_eq!(b, PlayerBucket::TeamEliminated);
    }

    #[test]
    fn likely_scratch_is_problem_asset() {
        let rep = grade(0.6, 5, 3);
        let b = classify_bucket(&rep, &proj(0.18, 0.3, 1.0), SeriesStateCode::Tied);
        assert_eq!(b, PlayerBucket::ProblemAsset);
    }

    #[test]
    fn toi_demotion_is_problem_asset() {
        let rep = grade(0.6, 5, 3);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 0.72), SeriesStateCode::Tied);
        assert_eq!(b, PlayerBucket::ProblemAsset);
    }

    #[test]
    fn hot_player_is_outperforming() {
        let rep = grade(0.6, 5, 7);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 1.0), SeriesStateCode::Leading);
        assert_eq!(b, PlayerBucket::Outperforming);
    }

    #[test]
    fn cold_star_with_intact_role_in_trouble_is_keep_faith() {
        // Grade F, series trailing, TOI intact (≥ 0.9 multiplier) → keep faith.
        let rep = grade(1.2, 5, 0);
        let b = classify_bucket(&rep, &proj(1.2, 1.0, 1.0), SeriesStateCode::Trailing);
        assert_eq!(b, PlayerBucket::KeepFaith);
    }

    #[test]
    fn cold_star_with_demoted_role_is_need_miracle() {
        let rep = grade(1.2, 5, 0);
        let b = classify_bucket(&rep, &proj(1.2, 1.0, 0.85), SeriesStateCode::Trailing);
        assert_eq!(b, PlayerBucket::NeedMiracle);
    }

    #[test]
    fn grade_c_healthy_role_is_on_pace() {
        let rep = grade(0.6, 5, 3);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 1.0), SeriesStateCode::Leading);
        assert_eq!(b, PlayerBucket::OnPace);
    }

    #[test]
    fn grade_d_healthy_leading_is_fine_but_fragile() {
        let rep = grade(0.6, 5, 2);
        assert_eq!(rep.grade, Grade::D);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 1.0), SeriesStateCode::Leading);
        assert_eq!(b, PlayerBucket::FineButFragile);
    }

    #[test]
    fn not_enough_data_is_too_early() {
        let rep = grade(0.6, 0, 0);
        let b = classify_bucket(&rep, &proj(0.6, 1.0, 1.0), SeriesStateCode::Tied);
        assert_eq!(b, PlayerBucket::TooEarly);
    }
}
