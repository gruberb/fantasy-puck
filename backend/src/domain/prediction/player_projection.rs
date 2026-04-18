//! Bayesian blend of a skater's points-per-game for the fantasy
//! projection.
//!
//! Pure-domain module: no DB, no HTTP, no framework deps. The
//! database-backed batch entrypoint lives in
//! `infra::prediction::project_players` and calls into [`project_one`]
//! here.
//!
//! The blend mixes four signals with shrinkage weights:
//!   - regular-season PPG  (stable talent floor)
//!   - full-playoff PPG    (this-run form, volume-stabilized)
//!   - recency-weighted playoff PPG (last few games count more)
//!   - 5-year historical playoff PPG (regression-to-mean anchor)
//!
//! Since v1.14.0 the in-playoff rate is volume-stabilized via shot
//! totals when available: a player generating 4 shots/game with
//! zero goals (1 playoff game) regresses up toward shots × league
//! shooting %, and a player scoring on few shots regresses down.
//! Same shape of blend, better input. A TOI-ratio multiplier on top
//! of the blend derates players whose recent ice time is materially
//! below their earlier-playoff usage (lineup demotions).
//!
//! Formula (simplified):
//!   rs_ppg          = rs_points / 82                    (see note below)
//!   observed_goals  = Σ goals_i / po_gp
//!   observed_shots  = Σ shots_i / po_gp                 (when available)
//!   stable_goals    = (1-w)·observed_goals + w·(observed_shots · LEAGUE_SH_PCT)
//!   po_rate         = stable_goals + Σ assists_i / po_gp
//!   recent_rate     = Σ w_i·points_i / Σ w_i            where w_i = 2^(-i/H)
//!                     over the last N games (i=0 most recent)
//!   blended_po_rate = α_po · po_rate + (1-α_po) · recent_rate
//!   historical_ppg  = h_points / h_gp                   (5-year totals)
//!
//!   projected_ppg   = (ALPHA·rs_ppg + po_gp·blended_po_rate + BETA·historical_ppg)
//!                     / (ALPHA + po_gp + BETA)
//!   final_ppg       = projected_ppg · toi_mult · availability_mult
//!
//! Note on rs_ppg: we'd prefer `rs_points / rs_games_played` but the
//! `StatsLeaders` leaderboard this crate consumes doesn't carry GP per
//! player. Using the 82-game denominator slightly under-projects players
//! who missed RS games; a future refinement can fetch each player's
//! landing payload to recover true RS GP.

/// RS-prior strength, in games-equivalent. Higher = slower to trust the
/// current-playoff signal.
pub const ALPHA: f32 = 10.0;
/// 5-year-history prior strength, in games-equivalent.
pub const BETA: f32 = 4.0;
/// Mixing weight between raw playoff PPG and the recency-weighted rate.
pub const PO_TO_RECENT_WEIGHT: f32 = 0.65;
/// Half-life in games for the recency decay.
pub const RECENT_HALF_LIFE_GAMES: f32 = 4.0;
/// How many recent games to include in the recency window.
pub const RECENT_WINDOW: usize = 10;
/// Below-the-bar availability floor for players absent from all playoff
/// games while the team has clearly been playing.
pub const ABSENT_MULTIPLIER: f32 = 0.3;
/// Number of team games after which an untouched player is flagged as
/// likely-not-dressing.
pub const MIN_APPEARANCE_TEAM_GAMES: u32 = 3;
/// NHL league-average shooting percentage. Used as the regression
/// anchor when stabilising a player's goal rate with their shot
/// volume. Tracks between 9.2% and 9.7% year over year; 0.095 is a
/// defensible midpoint.
pub const LEAGUE_SH_PCT: f32 = 0.095;
/// Weight on the shot-volume-implied goal rate inside the stabilised
/// playoff goal rate. `stable = (1-w)·observed + w·(shots·LEAGUE_SH_PCT)`.
/// 0.40 means "trust the player's observed finishing, but pull them
/// ~40% of the way back to league shooting percentage on their shot
/// volume" — a reasonable regress-to-the-mean over small samples.
pub const SHOT_STABILIZATION_WEIGHT: f32 = 0.40;
/// How many of the most recent playoff games define the "recent TOI"
/// window for the lineup-role multiplier. A change in line assignment
/// usually shows up inside 3 games.
pub const TOI_RATIO_RECENT_WINDOW: usize = 3;
/// Minimum games of earlier-playoff TOI history before the TOI ratio
/// multiplier activates. We need a baseline that isn't itself the
/// "recent" sample.
pub const TOI_RATIO_BASELINE_MIN: usize = 3;
/// Lower bound on the TOI multiplier. A demoted player gets at most
/// a 30% derate — enough to matter for fantasy output without
/// crushing their projection on one noisy soft-minutes night.
pub const TOI_RATIO_DERATE_FLOOR: f32 = 0.70;
/// Upper bound on the TOI multiplier. Promotions are capped tighter
/// because a single high-TOI game (overtime, OT-heavy series) can
/// fake-inflate the signal.
pub const TOI_RATIO_BOOST_CAP: f32 = 1.10;

/// Per-game skater stats used by the projection. Columns mirror
/// `playoff_skater_game_stats` with nullable Option fields where the
/// upstream boxscore may omit the stat.
#[derive(Debug, Clone, Copy, Default)]
pub struct GameStats {
    pub goals: i32,
    pub assists: i32,
    pub shots: Option<i32>,
    pub pp_points: Option<i32>,
    pub toi_seconds: Option<i32>,
}

impl GameStats {
    pub fn points(&self) -> i32 {
        self.goals + self.assists
    }
}

/// Caller-supplied data for each player to project.
#[derive(Debug, Clone)]
pub struct PlayerInput {
    pub nhl_id: i64,
    /// Exact name used for historical-table lookup.
    pub player_name: String,
    pub nhl_team: String,
    /// Regular-season total points for the current season. Sourced
    /// externally (skater-stats leaderboard).
    pub rs_points: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Projection {
    /// Expected fantasy points per remaining game, after shrinkage,
    /// TOI-role adjustment, and availability. This is what `race_sim`
    /// should feed its NegBin draw.
    pub ppg: f32,
    /// Separate multiplier exposed for diagnostics / UI; already applied
    /// to `ppg`. 1.0 = healthy, <1.0 = likely scratch/injured.
    pub active_prob: f32,
    /// Lineup-role multiplier derived from recent vs earlier-playoff
    /// TOI (1.0 when insufficient data). Already applied to `ppg`.
    /// Exposed so UI can show "role change" badges.
    pub toi_multiplier: f32,
}

/// Pure helper: given already-loaded signals for one player, compute the
/// blended PPG and availability multiplier.
///
/// `game_log` is ordered most-recent-first (`game_date DESC`). Callers
/// receive this ordering from `infra::prediction::project_players`.
pub fn project_one(
    input: &PlayerInput,
    team_games_played: u32,
    game_log: &[GameStats],
    historical: Option<&(i32, i32)>,
) -> Projection {
    let rs_ppg = input.rs_points as f32 / 82.0;

    let po_gp = game_log.len() as u32;
    let po_rate = stabilized_po_rate(game_log);
    let recent_rate = recency_weighted_rate(game_log);

    let blended_po_rate = if po_gp == 0 {
        0.0
    } else {
        PO_TO_RECENT_WEIGHT * po_rate + (1.0 - PO_TO_RECENT_WEIGHT) * recent_rate
    };

    let (hist_gp, hist_points) = historical.copied().unwrap_or((0, 0));
    let historical_ppg = if hist_gp > 0 {
        hist_points as f32 / hist_gp as f32
    } else {
        0.0
    };
    let beta_weight = if hist_gp > 0 { BETA } else { 0.0 };

    let numerator =
        ALPHA * rs_ppg + po_gp as f32 * blended_po_rate + beta_weight * historical_ppg;
    let denominator = ALPHA + po_gp as f32 + beta_weight;
    let base_ppg = if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    };

    let toi_multiplier = toi_ratio_multiplier(game_log);

    let active_prob = if team_games_played >= MIN_APPEARANCE_TEAM_GAMES && po_gp == 0 {
        ABSENT_MULTIPLIER
    } else {
        1.0
    };

    Projection {
        ppg: base_ppg * toi_multiplier * active_prob,
        active_prob,
        toi_multiplier,
    }
}

/// Observed playoff points-per-game, with the goal component
/// stabilised against shot volume when shot data is available.
///
/// When at least one game in the log has a non-null `shots` count, we
/// compute `total_shots / gp` as the player's shots-per-game rate
/// (excluding games with null shots from both numerator and
/// denominator) and blend that rate × `LEAGUE_SH_PCT` into the
/// observed goal rate with weight `SHOT_STABILIZATION_WEIGHT`.
///
/// Assists stay raw — the boxscore records an assist on most goals,
/// so the observed assists rate over even a few games is close to a
/// true rate; shot volume doesn't inform it the way it informs goals.
fn stabilized_po_rate(game_log: &[GameStats]) -> f32 {
    let gp = game_log.len();
    if gp == 0 {
        return 0.0;
    }
    let gp_f = gp as f32;
    let goals_total: i32 = game_log.iter().map(|g| g.goals).sum();
    let assists_total: i32 = game_log.iter().map(|g| g.assists).sum();
    let obs_goals_rate = goals_total as f32 / gp_f;
    let assists_rate = assists_total as f32 / gp_f;

    let (shots_total, shots_gp) =
        game_log
            .iter()
            .fold((0i32, 0usize), |(acc, count), g| match g.shots {
                Some(s) => (acc + s, count + 1),
                None => (acc, count),
            });

    if shots_gp == 0 {
        return obs_goals_rate + assists_rate;
    }

    let shots_rate = shots_total as f32 / shots_gp as f32;
    let expected_goals_rate = shots_rate * LEAGUE_SH_PCT;
    let stable_goals_rate = (1.0 - SHOT_STABILIZATION_WEIGHT) * obs_goals_rate
        + SHOT_STABILIZATION_WEIGHT * expected_goals_rate;
    stable_goals_rate + assists_rate
}

/// Recency-weighted average points rate over the first `RECENT_WINDOW`
/// entries of `game_log` (which is game_date-DESCending — most-recent
/// first). Weights follow `w_i = 2^(-i / H)` for half-life `H`,
/// normalised. Returns 0 when the input is empty.
pub fn recency_weighted_rate(game_log: &[GameStats]) -> f32 {
    let n = game_log.len().min(RECENT_WINDOW);
    if n == 0 {
        return 0.0;
    }
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for (i, g) in game_log.iter().take(n).enumerate() {
        let w = 2.0f32.powf(-(i as f32) / RECENT_HALF_LIFE_GAMES);
        num += w * g.points() as f32;
        den += w;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// Lineup-role multiplier derived from recent vs earlier-playoff TOI.
/// Returns 1.0 unless we have both (a) ≥ `TOI_RATIO_RECENT_WINDOW`
/// recent games with TOI data and (b) ≥ `TOI_RATIO_BASELINE_MIN`
/// older games with TOI data. Otherwise clamps
/// `recent_avg / older_avg` to `[TOI_RATIO_DERATE_FLOOR,
/// TOI_RATIO_BOOST_CAP]`.
///
/// Pure — tested in isolation.
pub fn toi_ratio_multiplier(game_log: &[GameStats]) -> f32 {
    let recent: Vec<i32> = game_log
        .iter()
        .take(TOI_RATIO_RECENT_WINDOW)
        .filter_map(|g| g.toi_seconds)
        .collect();
    let older: Vec<i32> = game_log
        .iter()
        .skip(TOI_RATIO_RECENT_WINDOW)
        .filter_map(|g| g.toi_seconds)
        .collect();
    if recent.len() < TOI_RATIO_RECENT_WINDOW || older.len() < TOI_RATIO_BASELINE_MIN {
        return 1.0;
    }
    let recent_avg = recent.iter().sum::<i32>() as f32 / recent.len() as f32;
    let older_avg = older.iter().sum::<i32>() as f32 / older.len() as f32;
    if older_avg < 1.0 {
        return 1.0;
    }
    (recent_avg / older_avg).clamp(TOI_RATIO_DERATE_FLOOR, TOI_RATIO_BOOST_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, rs_pts: i32) -> PlayerInput {
        PlayerInput {
            nhl_id: 1,
            player_name: name.into(),
            nhl_team: "X".into(),
            rs_points: rs_pts,
        }
    }

    fn gs(goals: i32, assists: i32) -> GameStats {
        GameStats {
            goals,
            assists,
            shots: None,
            pp_points: None,
            toi_seconds: None,
        }
    }

    fn gs_with_shots(goals: i32, assists: i32, shots: i32) -> GameStats {
        GameStats {
            goals,
            assists,
            shots: Some(shots),
            pp_points: None,
            toi_seconds: None,
        }
    }

    fn gs_with_toi(goals: i32, assists: i32, toi: i32) -> GameStats {
        GameStats {
            goals,
            assists,
            shots: None,
            pp_points: None,
            toi_seconds: Some(toi),
        }
    }

    #[test]
    fn cold_start_falls_back_to_rs_prior() {
        let p = project_one(&input("Rookie", 82), 0, &[], None);
        assert!((p.ppg - 1.0).abs() < 1e-5, "got {}", p.ppg);
        assert!((p.active_prob - 1.0).abs() < 1e-6);
    }

    #[test]
    fn heavy_playoff_sample_overtakes_rs_prior() {
        let log: Vec<GameStats> = (0..20).map(|_| gs(1, 1)).collect();
        let p = project_one(&input("Hot", 41), 20, &log, None);
        assert!(
            (p.ppg - 1.5).abs() < 0.05,
            "expected ~1.5 blended, got {}",
            p.ppg
        );
    }

    #[test]
    fn historical_anchor_damps_regression_for_unknown_current_sample() {
        let no_history = project_one(&input("A", 0), 0, &[], None);
        let with_history = project_one(&input("A", 0), 0, &[], Some(&(50, 80)));
        assert_eq!(no_history.ppg, 0.0);
        assert!(
            with_history.ppg > 0.3,
            "historical anchor should pull PPG above 0.3, got {}",
            with_history.ppg
        );
    }

    #[test]
    fn recency_weights_last_game_heaviest() {
        let mut recent = vec![gs(0, 0); 10];
        recent[0] = gs(4, 0);
        let mut stale = vec![gs(0, 0); 10];
        stale[9] = gs(4, 0);
        let r = recency_weighted_rate(&recent);
        let s = recency_weighted_rate(&stale);
        assert!(
            r > s * 3.0,
            "position-0 should weigh > 3× position-9: recent={r}, stale={s}"
        );
    }

    #[test]
    fn absent_player_gets_multiplier_hit() {
        let p = project_one(&input("Scratched", 82), 5, &[], None);
        assert!(
            (p.ppg - 0.3).abs() < 1e-3,
            "expected 0.3 after multiplier, got {}",
            p.ppg
        );
        assert!((p.active_prob - ABSENT_MULTIPLIER).abs() < 1e-6);
    }

    #[test]
    fn empty_input_has_zero_projection() {
        let p = project_one(
            &PlayerInput {
                nhl_id: 0,
                player_name: String::new(),
                nhl_team: String::new(),
                rs_points: 0,
            },
            0,
            &[],
            None,
        );
        assert_eq!(p.ppg, 0.0);
    }

    // ---- Phase 2 additions ----

    #[test]
    fn high_shot_low_goal_sample_projects_above_raw() {
        // 4 shots/game, 0 goals, 0 assists over 5 playoff games. Raw
        // observed rate is 0. Shot-stabilised rate = 0.6·0 + 0.4·(4 ×
        // LEAGUE_SH_PCT) = 0.4 × 0.38 = 0.152. That's a small but
        // non-zero regression toward expected finishing.
        let log: Vec<GameStats> = (0..5).map(|_| gs_with_shots(0, 0, 4)).collect();
        let stable = stabilized_po_rate(&log);
        // 0.4 * (4 * 0.095) = 0.152
        assert!(
            (stable - 0.152).abs() < 1e-3,
            "expected stable_rate ≈ 0.152, got {stable}"
        );
    }

    #[test]
    fn low_shot_high_goal_sample_regresses_down() {
        // 1 shot/game, 1 goal/game: a 100% shooting-pct 3-game run.
        // Raw obs = 1.0 goal/gp. Stabilised: 0.6·1.0 + 0.4·(1 × 0.095)
        // = 0.6 + 0.038 = 0.638. Properly pulls a fluke-hot shooter
        // back toward something sustainable.
        let log: Vec<GameStats> = (0..3).map(|_| gs_with_shots(1, 0, 1)).collect();
        let stable = stabilized_po_rate(&log);
        assert!(
            (stable - 0.638).abs() < 1e-3,
            "expected stable_rate ≈ 0.638, got {stable}"
        );
    }

    #[test]
    fn missing_shots_falls_back_to_raw_points() {
        // gs(1, 2) = 3 pts, gs(0, 1) = 1 pt, gs(1, 0) = 1 pt → 5/3.
        let log = vec![gs(1, 2), gs(0, 1), gs(1, 0)];
        let raw = stabilized_po_rate(&log);
        assert!(
            (raw - 5.0 / 3.0).abs() < 1e-4,
            "fallback should equal raw PPG, got {raw}"
        );
    }

    #[test]
    fn toi_demotion_derates_projection() {
        // First 3 games (older) at 18 min; last 3 (recent) at 9 min.
        // Log is most-recent-first, so recent[0..3] = 9min, older[3..] = 18min.
        let log = vec![
            gs_with_toi(0, 0, 540), // recent 1
            gs_with_toi(0, 0, 540), // recent 2
            gs_with_toi(0, 0, 540), // recent 3
            gs_with_toi(0, 0, 1080), // older 1
            gs_with_toi(0, 0, 1080), // older 2
            gs_with_toi(0, 0, 1080), // older 3
        ];
        let m = toi_ratio_multiplier(&log);
        // 540 / 1080 = 0.5, clamped to TOI_RATIO_DERATE_FLOOR = 0.7
        assert!(
            (m - TOI_RATIO_DERATE_FLOOR).abs() < 1e-4,
            "demotion should clamp to floor, got {m}"
        );
    }

    #[test]
    fn toi_stable_usage_gives_multiplier_one() {
        let log = vec![gs_with_toi(0, 0, 900); 6];
        let m = toi_ratio_multiplier(&log);
        assert!((m - 1.0).abs() < 1e-6, "stable TOI → 1.0, got {m}");
    }

    #[test]
    fn toi_insufficient_data_gives_multiplier_one() {
        // Only 2 recent games — below TOI_RATIO_RECENT_WINDOW = 3.
        let log = vec![gs_with_toi(0, 0, 900); 2];
        assert!((toi_ratio_multiplier(&log) - 1.0).abs() < 1e-6);
        // Null TOI values skipped; not enough non-null data.
        let log = vec![gs(0, 0); 10];
        assert!((toi_ratio_multiplier(&log) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn toi_promotion_is_capped_tighter_than_demotion() {
        // 30 min recent, 10 min older: raw ratio 3.0, clamp to 1.10.
        let log = vec![
            gs_with_toi(0, 0, 1800),
            gs_with_toi(0, 0, 1800),
            gs_with_toi(0, 0, 1800),
            gs_with_toi(0, 0, 600),
            gs_with_toi(0, 0, 600),
            gs_with_toi(0, 0, 600),
        ];
        let m = toi_ratio_multiplier(&log);
        assert!(
            (m - TOI_RATIO_BOOST_CAP).abs() < 1e-4,
            "promotion should clamp to cap, got {m}"
        );
    }
}
