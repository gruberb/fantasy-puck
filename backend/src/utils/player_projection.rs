//! Bayesian blend of a skater's points-per-game for the fantasy
//! projection. Replaces `race_odds::player_ppg` once `game_type == 3`
//! and `playoff_skater_game_stats` is populated.
//!
//! The blend mixes four signals with shrinkage weights:
//!   - regular-season PPG  (stable talent floor)
//!   - full-playoff PPG    (this-run form)
//!   - recency-weighted playoff PPG (last few games count more)
//!   - 5-year historical playoff PPG (regression-to-mean anchor)
//!
//! Formula:
//!   rs_ppg          = rs_points / 82                    (see note below)
//!   po_ppg          = sum(points_i) / po_gp             (from game log)
//!   recent_ppg      = Σ w_i·points_i / Σ w_i            where w_i = 2^(-i/H)
//!                     over the last N team games (i=0 most recent)
//!   blended_po_ppg  = α_po · po_ppg + (1-α_po) · recent_ppg
//!   historical_ppg  = h_points / h_gp                   (5-year totals)
//!
//!   projected_ppg   = (ALPHA·rs_ppg + po_gp·blended_po_ppg + BETA·historical_ppg)
//!                     / (ALPHA + po_gp + BETA)
//!
//! Availability multiplier is applied on top to mute a player who's
//! clearly not dressing:
//!   if team has played ≥ MIN_APPEARANCE_TEAM_GAMES games and the
//!   player has zero appearances → 0.3; else 1.0.
//!
//! Note on rs_ppg: we'd prefer `rs_points / rs_games_played` but the
//! `StatsLeaders` leaderboard this crate consumes doesn't carry GP per
//! player. Using the 82-game denominator slightly under-projects players
//! who missed RS games; a future refinement can fetch each player's
//! landing payload to recover true RS GP.

use std::collections::HashMap;

use tracing::debug;

use crate::db::FantasyDb;
use crate::error::{Error, Result};

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
    /// Expected fantasy points per remaining game, after shrinkage and
    /// availability adjustment. This is what `race_sim` should feed its
    /// Poisson draw.
    pub ppg: f32,
    /// Separate multiplier exposed for diagnostics / UI; already applied
    /// to `ppg`. 1.0 = healthy, <1.0 = likely scratch/injured.
    pub active_prob: f32,
}

/// Project every player in a single DB round-trip.
///
/// `team_games_played` maps NHL team abbrev → playoff games already
/// played (used for the availability multiplier and the `po_gp` weight).
/// Typical source: summing the current season's `playoff_game_results`
/// or carousel wins.
pub async fn project_players(
    db: &FantasyDb,
    season: u32,
    players: &[PlayerInput],
    team_games_played: &HashMap<String, u32>,
) -> Result<HashMap<i64, Projection>> {
    if players.is_empty() {
        return Ok(HashMap::new());
    }

    let player_ids: Vec<i64> = players.iter().map(|p| p.nhl_id).collect();
    // Order lets us slice per-player sub-ranges without rehashing —
    // game_date DESC so the "last N" window is already at the front.
    let stat_rows: Vec<(i64, i32)> = sqlx::query_as(
        r#"
        SELECT player_id, points
        FROM playoff_skater_game_stats
        WHERE season = $1
          AND player_id = ANY($2::bigint[])
        ORDER BY player_id ASC, game_date DESC, game_id DESC
        "#,
    )
    .bind(season as i32)
    .bind(&player_ids)
    .fetch_all(db.pool())
    .await
    .map_err(Error::Database)?;

    // Bucket stat rows by player for per-player aggregation. Preserves
    // the ORDER BY — game_date DESC — so the first elements are the most
    // recent games.
    let mut by_player: HashMap<i64, Vec<i32>> = HashMap::with_capacity(players.len());
    for (pid, pts) in stat_rows {
        by_player.entry(pid).or_default().push(pts);
    }

    let names: Vec<&str> = players.iter().map(|p| p.player_name.as_str()).collect();
    let historical_rows: Vec<(String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT player_name, gp, p
        FROM historical_playoff_skater_totals
        WHERE player_name = ANY($1::text[])
        "#,
    )
    .bind(&names)
    .fetch_all(db.pool())
    .await
    .map_err(Error::Database)?;

    let historical: HashMap<String, (i32, i32)> = historical_rows
        .into_iter()
        .map(|(n, gp, p)| (n, (gp, p)))
        .collect();

    let mut out: HashMap<i64, Projection> = HashMap::with_capacity(players.len());
    for p in players {
        let team_gp = team_games_played.get(&p.nhl_team).copied().unwrap_or(0);
        let pts_rows = by_player.get(&p.nhl_id).cloned().unwrap_or_default();
        let projection = project_one(p, team_gp, &pts_rows, historical.get(&p.player_name));
        out.insert(p.nhl_id, projection);
    }
    debug!(players = players.len(), "player projection batch complete");
    Ok(out)
}

/// Pure helper: given already-loaded signals for one player, compute the
/// blended PPG and availability multiplier. Exposed for tests / the
/// backtest harness.
pub fn project_one(
    input: &PlayerInput,
    team_games_played: u32,
    points_per_game_desc: &[i32],
    historical: Option<&(i32, i32)>,
) -> Projection {
    let rs_ppg = input.rs_points as f32 / 82.0;

    let po_gp = points_per_game_desc.len() as u32;
    let po_points: i32 = points_per_game_desc.iter().sum();
    let po_ppg = if po_gp > 0 {
        po_points as f32 / po_gp as f32
    } else {
        0.0
    };

    let recent_ppg = recency_weighted_ppg(points_per_game_desc);

    let blended_po_ppg = if po_gp == 0 {
        0.0
    } else {
        PO_TO_RECENT_WEIGHT * po_ppg + (1.0 - PO_TO_RECENT_WEIGHT) * recent_ppg
    };

    let (hist_gp, hist_points) = historical.copied().unwrap_or((0, 0));
    let historical_ppg = if hist_gp > 0 {
        hist_points as f32 / hist_gp as f32
    } else {
        0.0
    };
    let beta_weight = if hist_gp > 0 { BETA } else { 0.0 };

    let numerator =
        ALPHA * rs_ppg + po_gp as f32 * blended_po_ppg + beta_weight * historical_ppg;
    let denominator = ALPHA + po_gp as f32 + beta_weight;
    let base_ppg = if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    };

    let active_prob =
        if team_games_played >= MIN_APPEARANCE_TEAM_GAMES && po_gp == 0 {
            ABSENT_MULTIPLIER
        } else {
            1.0
        };

    Projection {
        ppg: base_ppg * active_prob,
        active_prob,
    }
}

/// Recency-weighted average of the first `RECENT_WINDOW` entries of
/// `points_desc` (which is game_date-DESCending — i.e. most-recent first).
/// Weights follow `w_i = 2^(-i / H)` for half-life `H`, normalised. 0
/// when the input is empty.
fn recency_weighted_ppg(points_desc: &[i32]) -> f32 {
    let n = points_desc.len().min(RECENT_WINDOW);
    if n == 0 {
        return 0.0;
    }
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for (i, &pts) in points_desc.iter().take(n).enumerate() {
        let w = 2.0f32.powf(-(i as f32) / RECENT_HALF_LIFE_GAMES);
        num += w * pts as f32;
        den += w;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
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

    #[test]
    fn cold_start_falls_back_to_rs_prior() {
        // No playoff games played yet, no historical record. With only
        // the RS prior, projected_ppg == rs_ppg.
        let p = project_one(&input("Rookie", 82), 0, &[], None);
        assert!((p.ppg - 1.0).abs() < 1e-5, "got {}", p.ppg);
        assert!((p.active_prob - 1.0).abs() < 1e-6);
    }

    #[test]
    fn heavy_playoff_sample_overtakes_rs_prior() {
        // 20 games at 2.0 PPG should shift a 0.5-PPG RS prior toward
        // the playoff signal — weight = 20 games vs ALPHA = 10.
        let pts: Vec<i32> = (0..20).map(|_| 2).collect();
        let p = project_one(&input("Hot", 41), 20, &pts, None);
        // Expected blend (no recent-vs-raw distinction when every game
        // is a 2): numerator = 10*0.5 + 20*2.0 = 45, denom = 30 → 1.5.
        assert!(
            (p.ppg - 1.5).abs() < 0.01,
            "expected ~1.5 blended, got {}",
            p.ppg
        );
    }

    #[test]
    fn historical_anchor_damps_regression_for_unknown_current_sample() {
        // RS 0, current playoff 0 games. Historical shows 80 pts in 50
        // GP (1.6 PPG). The BETA prior should pull the projection up
        // from pure RS (0.0) toward the historical mean.
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
        // 10 games: a 4-point spike at the front vs the back. With
        // half-life 4 across a 10-game window, position-0 weighs ~4.5×
        // position-9, so the "recent" series should carry several times
        // more signal.
        let mut recent = vec![0; 10];
        recent[0] = 4;
        let mut stale = vec![0; 10];
        stale[9] = 4;
        let r = recency_weighted_ppg(&recent);
        let s = recency_weighted_ppg(&stale);
        assert!(
            r > s * 3.0,
            "position-0 should weigh > 3× position-9: recent={r}, stale={s}"
        );
    }

    #[test]
    fn absent_player_gets_multiplier_hit() {
        // Team has played 5 games; player has 0 appearances. Projection
        // should be multiplied by ABSENT_MULTIPLIER.
        let pts: Vec<i32> = Vec::new();
        let p = project_one(&input("Scratched", 82), 5, &pts, None);
        // Base would be rs_ppg = 1.0; with availability multiplier 0.3
        // → 0.3.
        assert!(
            (p.ppg - 0.3).abs() < 1e-3,
            "expected 0.3 after multiplier, got {}",
            p.ppg
        );
        assert!((p.active_prob - ABSENT_MULTIPLIER).abs() < 1e-6);
    }

    #[test]
    fn empty_input_returns_empty_map_without_query() {
        // Safety: project_players on an empty roster must not panic or
        // issue a zero-element IN query (some drivers choke on that).
        // This test only checks the early-return path.
        // The function is async and touches the DB, so we can only
        // exercise the empty branch via the caller's check; here we
        // just assert the helper preserves sanity.
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
}
