//! Measure how calibrated the current race-odds model is against a
//! completed historical playoff season.
//!
//! Given a season whose `playoff_game_results` rows are already in the
//! DB (backfilled via `/api/admin/backfill-historical`), this module:
//!   1. Realizes the season's outcomes (advanced R1/R2/R3/won Cup per
//!      NHL team) by folding every game through
//!      `domain::prediction::backtest::reconstruct_bracket_from_results`.
//!   2. Rebuilds the day-1 `BracketState` — R1 series at 0-0, every
//!      later round `Future` — so the sim has the same starting state
//!      it had at real-life playoff start.
//!   3. Runs the current engine with production hyperparameters (Elo
//!      ratings seeded from that season's standings, current k_factor,
//!      current home-ice bonus) against that state.
//!   4. Scores the predicted round-advancement probabilities against the
//!      realized outcomes with Brier and log-loss, per round.
//!
//! Useful for answering "is the model calibrated?" without committing
//! to a grid-search tuning pass. If one-season Brier looks OK, further
//! hyperparameter tuning is probably low-leverage. If it's off, grid
//! search becomes justified and its target is the aggregate Brier
//! across all backfilled seasons.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tracing::debug;

use crate::db::FantasyDb;
use crate::domain::prediction::{
    backtest::{self, ResultRow},
    goalie_rating::{self, GoalieEntry},
    playoff_elo::{self},
    race_sim::{
        self, simulate_with_seed, BracketState, RaceSimInput, SeriesState, SimFantasyTeam,
        SimPlayer, TeamRating, DEFAULT_TRIALS, HOME_ICE_ELO,
    },
};
use crate::error::{Error, Result};

pub const ELO_K_FACTOR: f32 = std::f32::consts::LN_10 / 400.0;

// ---------------------------------------------------------------------------
// Tunable knobs
// ---------------------------------------------------------------------------

/// Knobs the calibration sweep varies per run. Every field has a
/// production default so `CalibrationKnobs::default()` reproduces
/// today's live-path behavior; the sweep endpoint overrides whichever
/// subset the caller wants to explore.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationKnobs {
    /// Elo points per RS-standings-point of separation from league
    /// average. Production constant is `playoff_elo::POINTS_SCALE = 6.0`.
    pub points_scale: f32,
    /// Shrinkage factor ∈ [0, 1] applied to the RS-point deviation
    /// before scaling. 1.0 = no shrinkage (legacy behavior); 0.7 is
    /// the usual Bayesian starting point for an 82-game sample.
    pub shrinkage: f32,
    /// Logistic scale (`k_factor`) on the Elo scale. Production uses
    /// `ln(10) / 400 ≈ 0.00576` so the standard Elo identity holds.
    pub k_factor: f32,
    /// Raw league-wide home-ice bonus in Elo units. Multiplied by
    /// `k_factor` to produce the pre-sigmoid fallback passed into the
    /// sim for teams with no per-team bonus.
    pub home_ice_elo: f32,
    /// Monte Carlo trial count. The sweep uses this to trade resolution
    /// for wall time when exploring a large grid.
    pub trials: usize,
}

impl Default for CalibrationKnobs {
    fn default() -> Self {
        Self {
            points_scale: playoff_elo::POINTS_SCALE,
            // Track the production `seed_from_standings` wrapper's
            // shrinkage so `/api/admin/calibrate` scores today's live
            // model. When the sweep picks a new shrinkage the update
            // here and in `playoff_elo::PRODUCTION_SHRINKAGE` move
            // together.
            shrinkage: playoff_elo::PRODUCTION_SHRINKAGE,
            k_factor: ELO_K_FACTOR,
            home_ice_elo: HOME_ICE_ELO,
            trials: DEFAULT_TRIALS,
        }
    }
}

// ---------------------------------------------------------------------------
// Output DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamOutcome {
    pub abbrev: String,
    pub advanced_r1: bool,
    pub advanced_r2: bool,
    pub advanced_r3: bool,
    pub won_cup: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerTeamCalibration {
    pub abbrev: String,
    pub predicted_advance_r1: f32,
    pub predicted_advance_r2: f32,
    pub predicted_advance_r3: f32,
    pub predicted_cup_win: f32,
    pub outcome: TeamOutcome,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationReport {
    pub season: u32,
    pub trials: usize,
    pub teams_evaluated: usize,
    pub brier_r1: f32,
    pub brier_r2: f32,
    pub brier_r3: f32,
    pub brier_cup: f32,
    pub log_loss_r1: f32,
    pub log_loss_r2: f32,
    pub log_loss_r3: f32,
    pub log_loss_cup: f32,
    pub teams: Vec<PerTeamCalibration>,
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Run one calibration pass against a completed historical season
/// using production hyperparameters. Thin wrapper over
/// [`calibrate_season_with_knobs`] with [`CalibrationKnobs::default`].
pub async fn calibrate_season(
    db: &FantasyDb,
    nhl: &crate::NhlClient,
    season: u32,
) -> Result<CalibrationReport> {
    calibrate_season_with_knobs(db, nhl, season, &CalibrationKnobs::default()).await
}

/// Run one calibration pass against a completed historical season with
/// an explicit set of knobs. Deterministic — uses a fixed RNG seed so
/// two runs over the same knobs produce identical Brier/log-loss.
/// That lets the sweep compare knob *signal* without Monte Carlo
/// noise masquerading as an improvement.
pub async fn calibrate_season_with_knobs(
    db: &FantasyDb,
    nhl: &crate::NhlClient,
    season: u32,
    knobs: &CalibrationKnobs,
) -> Result<CalibrationReport> {
    // 1. Load all completed games for this season.
    let rows = load_result_rows(db, season).await?;
    if rows.is_empty() {
        return Err(Error::Validation(format!(
            "No playoff_game_results rows for season {season}. Run the backfill first."
        )));
    }

    // 2. Realize outcomes from the full bracket reconstruction.
    let realized_bracket = backtest::reconstruct_bracket_from_results(&rows);
    let outcomes = extract_realized_outcomes(&realized_bracket);
    if outcomes.is_empty() {
        return Err(Error::Validation(
            "Could not infer any realized outcomes from the game log.".into(),
        ));
    }

    // 3. Build the day-1 BracketState from the same R1 series but with
    //    wins zeroed out, and later rounds all Future.
    let day1 = build_day1_bracket(&realized_bracket);

    // 4. Seed Elo + home-ice from the standings snapshot AS OF the
    //    day before this season's first playoff game. Falls back to a
    //    few days earlier if the NHL API returns empty for that date
    //    (there's a gap between regular-season end and playoff start
    //    where the endpoint serves nothing). Last-ditch fallback is
    //    today's live standings, which carries the current-roster
    //    bias but beats running with no ratings.
    let standings = fetch_historical_standings(db, nhl, season).await;
    // Goalie bonuses are the v1.15+ component. Fetch the historical
    // season's regular-season goalie leaderboard; on failure the
    // calibration falls back to zero bonuses (and underestimates the
    // production model accordingly).
    let goalie_bonuses = fetch_historical_goalie_bonuses(nhl, season).await;
    let (ratings, k_factor, home_ice_bonus) =
        build_ratings(standings.as_ref(), &goalie_bonuses, knobs);

    // 5. Run sim. No fantasy teams needed — we're only scoring NHL team
    //    advancement, so we hand the engine a single placeholder team
    //    with no players (the sim's nhl_teams output drives everything
    //    we score).
    let input = RaceSimInput {
        bracket: day1,
        ratings,
        k_factor,
        home_ice_bonus,
        fantasy_teams: vec![placeholder_team()],
    };
    let sim_input = input.clone();
    let trials = knobs.trials;
    let output = tokio::task::spawn_blocking(move || {
        simulate_with_seed(&sim_input, trials, CALIBRATION_RNG_SEED)
    })
    .await
    .map_err(|e| Error::Internal(format!("calibrate sim join error: {e}")))?;

    // 6. Score.
    let report = score(season, trials, &output, &outcomes);
    debug!(
        season,
        teams = report.teams_evaluated,
        brier_cup = report.brier_cup,
        "calibration complete"
    );
    Ok(report)
}

/// Fixed seed for the calibration RNG. Chosen arbitrarily; the point
/// is that two runs with identical inputs produce identical Brier
/// scores so the sweep can attribute every delta to a knob change,
/// not to MC jitter.
const CALIBRATION_RNG_SEED: u64 = 0xC0FFEE_CA11B8A7E;

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

async fn load_result_rows(db: &FantasyDb, season: u32) -> Result<Vec<ResultRow>> {
    let rows: Vec<(String, String, String, i32, i32, Option<i16>)> = sqlx::query_as(
        r#"
        SELECT
            TO_CHAR(game_date, 'YYYY-MM-DD') AS game_date,
            home_team, away_team,
            home_score, away_score,
            round
        FROM playoff_game_results
        WHERE season = $1
        ORDER BY game_date ASC, game_id ASC
        "#,
    )
    .bind(season as i32)
    .fetch_all(db.pool())
    .await
    .map_err(Error::Database)?;
    Ok(rows
        .into_iter()
        .map(
            |(game_date, home_team, away_team, home_score, away_score, round)| ResultRow {
                game_date,
                home_team,
                away_team,
                home_score,
                away_score,
                round: round.map(|r| r as u8),
            },
        )
        .collect())
}

/// Scan the realized bracket and record whether each NHL team advanced
/// out of each round.
///
/// "Advanced out of round r" = the team appears as `winner` in some
/// `Completed` series of round r. R1 winners advance to R2; R4 winner
/// is the Cup winner. Teams that don't appear in R1 at all are
/// excluded (didn't make the playoffs).
fn extract_realized_outcomes(bracket: &BracketState) -> HashMap<String, TeamOutcome> {
    let mut out: HashMap<String, TeamOutcome> = HashMap::new();
    // Seed with every team that participated in R1.
    let Some(r1) = bracket.rounds.first() else {
        return out;
    };
    for series in r1 {
        let (a, b) = match series {
            SeriesState::Completed { winner, loser, .. } => (winner.clone(), loser.clone()),
            SeriesState::InProgress {
                top_team,
                bottom_team,
                ..
            } => (top_team.clone(), bottom_team.clone()),
            SeriesState::Future => continue,
        };
        out.entry(a).or_insert(TeamOutcome {
            abbrev: String::new(),
            advanced_r1: false,
            advanced_r2: false,
            advanced_r3: false,
            won_cup: false,
        });
        out.entry(b).or_insert(TeamOutcome {
            abbrev: String::new(),
            advanced_r1: false,
            advanced_r2: false,
            advanced_r3: false,
            won_cup: false,
        });
    }
    // Populate abbrev.
    for (k, v) in out.iter_mut() {
        v.abbrev.clone_from(k);
    }
    // Record advances per round. round_idx 0 = R1; winning means advancing
    // out of that round.
    for (round_idx, round) in bracket.rounds.iter().enumerate() {
        for series in round {
            if let SeriesState::Completed { winner, .. } = series {
                if let Some(outcome) = out.get_mut(winner) {
                    match round_idx {
                        0 => outcome.advanced_r1 = true,
                        1 => outcome.advanced_r2 = true,
                        2 => outcome.advanced_r3 = true,
                        3 => outcome.won_cup = true,
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

/// Copy the round-1 pairings from the realized bracket, reset wins to
/// 0-0, and fill every later round with Future. That's the state the
/// league was in on day 1 of the playoffs.
fn build_day1_bracket(realized: &BracketState) -> BracketState {
    let r1: Vec<SeriesState> = realized
        .rounds
        .first()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| match s {
            SeriesState::Completed { winner, loser, .. } => SeriesState::InProgress {
                top_team: winner,
                top_wins: 0,
                bottom_team: loser,
                bottom_wins: 0,
            },
            SeriesState::InProgress {
                top_team,
                bottom_team,
                ..
            } => SeriesState::InProgress {
                top_team,
                top_wins: 0,
                bottom_team,
                bottom_wins: 0,
            },
            SeriesState::Future => SeriesState::Future,
        })
        .collect();

    BracketState {
        rounds: vec![
            r1,
            vec![SeriesState::Future; 4],
            vec![SeriesState::Future; 2],
            vec![SeriesState::Future; 1],
        ],
    }
}

fn build_ratings(
    standings: Option<&serde_json::Value>,
    goalie_bonuses: &HashMap<String, f32>,
    knobs: &CalibrationKnobs,
) -> (HashMap<String, TeamRating>, f32, f32) {
    let fallback_ice_bonus = knobs.k_factor * knobs.home_ice_elo;
    let Some(standings) = standings else {
        return (HashMap::new(), knobs.k_factor, fallback_ice_bonus);
    };
    let elo =
        playoff_elo::seed_from_standings_tuned(standings, knobs.points_scale, knobs.shrinkage);
    let home_bonus_map = playoff_elo::home_bonus_from_standings(standings);
    let map: HashMap<String, TeamRating> = elo
        .into_iter()
        .map(|(abbrev, base)| {
            let home_bonus = home_bonus_map.get(&abbrev).copied().unwrap_or(0.0);
            let goalie_bonus = goalie_bonuses.get(&abbrev).copied().unwrap_or(0.0);
            let rating = TeamRating::with_home_bonus(base, home_bonus)
                .with_goalie_bonus(goalie_bonus);
            (abbrev, rating)
        })
        .collect();
    (map, knobs.k_factor, fallback_ice_bonus)
}

/// Fetch the regular-season goalie leaderboard for the given
/// historical season and convert to per-team Elo bonuses via
/// `goalie_rating::compute_bonuses`. Returns an empty map on any
/// error (logged) — calibration still runs without goalie data, it
/// just underestimates the v1.15+ production model.
async fn fetch_historical_goalie_bonuses(
    nhl: &crate::NhlClient,
    season: u32,
) -> HashMap<String, f32> {
    let leaders = match nhl.get_goalie_stats(&season, 2).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(
                error = %e,
                season,
                "calibrate: goalie-stats fetch failed; skipping goalie component"
            );
            return HashMap::new();
        }
    };
    let sv_lookup: HashMap<i64, f32> = leaders
        .save_pctg
        .iter()
        .map(|p| (p.id as i64, p.value as f32))
        .collect();
    let entries: Vec<GoalieEntry> = leaders
        .wins
        .iter()
        .map(|p| GoalieEntry {
            player_id: p.id as i64,
            team_abbrev: p.team_abbrev.clone(),
            wins: p.value as f32,
            save_pct: sv_lookup.get(&(p.id as i64)).copied(),
        })
        .collect();
    goalie_rating::compute_bonuses(&entries)
}

/// Fetch the NHL standings snapshot for the day before `season`'s
/// first backfilled playoff game. Retries the call stepping one day
/// further back each attempt (up to 10 days) because the NHL API
/// returns an empty `standings` array for dates in the RS-to-playoffs
/// gap. Returns `None` if every attempt is empty — caller should fall
/// back to live standings and accept the bias.
async fn fetch_historical_standings(
    db: &FantasyDb,
    nhl: &crate::NhlClient,
    season: u32,
) -> Option<serde_json::Value> {
    // Pull the earliest playoff date for this season. This becomes
    // our starting "day before" anchor.
    let earliest: Option<String> = sqlx::query_scalar(
        r#"
        SELECT TO_CHAR(MIN(game_date), 'YYYY-MM-DD')
        FROM playoff_game_results
        WHERE season = $1
        "#,
    )
    .bind(season as i32)
    .fetch_one(db.pool())
    .await
    .ok()?;
    let earliest = earliest?;
    let anchor = chrono::NaiveDate::parse_from_str(&earliest, "%Y-%m-%d").ok()?;

    for days_back in 1..=10 {
        let candidate = anchor - chrono::Duration::days(days_back);
        let date_str = candidate.format("%Y-%m-%d").to_string();
        let Ok(json) = nhl.get_standings_for_date(&date_str).await else {
            continue;
        };
        let has_teams = json
            .get("standings")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_teams {
            tracing::debug!(
                season,
                standings_date = %date_str,
                "calibrate: using historical standings"
            );
            return Some(json);
        }
    }
    tracing::warn!(
        season,
        "calibrate: could not find non-empty historical standings within 10 days of playoff start; \
         falling back to live standings"
    );
    nhl.get_standings_raw().await.ok()
}

fn placeholder_team() -> SimFantasyTeam {
    // One empty fantasy team so the sim's ranking loop has something to
    // sort. We ignore `teams` / `players` outputs; only `nhl_teams`
    // feeds the calibration.
    SimFantasyTeam {
        team_id: 0,
        team_name: "placeholder".into(),
        players: Vec::<SimPlayer>::new(),
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

fn score(
    season: u32,
    trials: usize,
    output: &race_sim::RaceSimOutput,
    outcomes: &HashMap<String, TeamOutcome>,
) -> CalibrationReport {
    // Only score teams we actually have predictions for AND an outcome.
    let mut teams: Vec<PerTeamCalibration> = Vec::with_capacity(outcomes.len());
    let mut r1_pairs: Vec<(f32, bool)> = Vec::new();
    let mut r2_pairs: Vec<(f32, bool)> = Vec::new();
    let mut r3_pairs: Vec<(f32, bool)> = Vec::new();
    let mut cup_pairs: Vec<(f32, bool)> = Vec::new();

    let predicted: HashMap<&str, &race_sim::NhlTeamOdds> = output
        .nhl_teams
        .iter()
        .map(|t| (t.abbrev.as_str(), t))
        .collect();

    let mut seen: HashSet<&str> = HashSet::new();
    for (abbrev, outcome) in outcomes {
        let Some(pred) = predicted.get(abbrev.as_str()) else {
            continue;
        };
        seen.insert(abbrev.as_str());
        teams.push(PerTeamCalibration {
            abbrev: abbrev.clone(),
            predicted_advance_r1: pred.advance_round1_prob,
            predicted_advance_r2: pred.conference_finals_prob,
            predicted_advance_r3: pred.cup_finals_prob,
            predicted_cup_win: pred.cup_win_prob,
            outcome: outcome.clone(),
        });
        r1_pairs.push((pred.advance_round1_prob, outcome.advanced_r1));
        r2_pairs.push((pred.conference_finals_prob, outcome.advanced_r2));
        r3_pairs.push((pred.cup_finals_prob, outcome.advanced_r3));
        cup_pairs.push((pred.cup_win_prob, outcome.won_cup));
    }

    // Sort for stable reporting.
    teams.sort_by(|a, b| a.abbrev.cmp(&b.abbrev));

    CalibrationReport {
        season,
        trials,
        teams_evaluated: seen.len(),
        brier_r1: backtest::brier_score(&r1_pairs),
        brier_r2: backtest::brier_score(&r2_pairs),
        brier_r3: backtest::brier_score(&r3_pairs),
        brier_cup: backtest::brier_score(&cup_pairs),
        log_loss_r1: backtest::log_loss(&r1_pairs),
        log_loss_r2: backtest::log_loss(&r2_pairs),
        log_loss_r3: backtest::log_loss(&r3_pairs),
        log_loss_cup: backtest::log_loss(&cup_pairs),
        teams,
    }
}

// ---------------------------------------------------------------------------
// Sweep orchestration
// ---------------------------------------------------------------------------

/// Knob-value grid for the sweep. Each field is a list of candidate
/// values; missing / empty fields fall back to the matching default
/// in [`CalibrationKnobs::default`]. The sweep visits every element
/// of the Cartesian product.
#[derive(Debug, Clone, Default)]
pub struct CalibrationGrid {
    pub points_scale: Vec<f32>,
    pub shrinkage: Vec<f32>,
    pub k_factor: Vec<f32>,
    pub home_ice_elo: Vec<f32>,
    pub trials: Vec<usize>,
}

/// One entry in the sweep report. Strips the full per-team detail
/// that lives in [`CalibrationReport`]; the sweep cares about
/// aggregate scoring for ranking.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRun {
    pub knobs: CalibrationKnobs,
    pub brier_r1: f32,
    pub brier_r2: f32,
    pub brier_r3: f32,
    pub brier_cup: f32,
    pub log_loss_r1: f32,
    pub log_loss_cup: f32,
    /// Sum of per-round Brier. Primary ranking metric — a run that
    /// wins overall must do well at every round, not just the one
    /// the knob was tuned for.
    pub brier_aggregate: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepReport {
    pub season: u32,
    pub grid_size: usize,
    /// Best run by aggregate Brier (lowest first).
    pub best: SweepRun,
    /// Every run, sorted by `brier_aggregate` ascending.
    pub runs: Vec<SweepRun>,
}

/// Run `calibrate_season_with_knobs` over every cell in `grid` and
/// rank by aggregate Brier. Sequential — the sim is CPU-bound and
/// running grids in parallel would drown out the per-run NHL API
/// fetch for historical standings (cache hit after the first one).
pub async fn calibrate_sweep(
    db: &FantasyDb,
    nhl: &crate::NhlClient,
    season: u32,
    grid: &CalibrationGrid,
) -> Result<SweepReport> {
    let defaults = CalibrationKnobs::default();
    // Default-fill empty axes so callers can sweep a subset without
    // having to spell out the others.
    let axes_points = if grid.points_scale.is_empty() {
        vec![defaults.points_scale]
    } else {
        grid.points_scale.clone()
    };
    let axes_shrink = if grid.shrinkage.is_empty() {
        vec![defaults.shrinkage]
    } else {
        grid.shrinkage.clone()
    };
    let axes_k = if grid.k_factor.is_empty() {
        vec![defaults.k_factor]
    } else {
        grid.k_factor.clone()
    };
    let axes_home = if grid.home_ice_elo.is_empty() {
        vec![defaults.home_ice_elo]
    } else {
        grid.home_ice_elo.clone()
    };
    let axes_trials = if grid.trials.is_empty() {
        vec![defaults.trials]
    } else {
        grid.trials.clone()
    };

    let grid_size = axes_points.len()
        * axes_shrink.len()
        * axes_k.len()
        * axes_home.len()
        * axes_trials.len();
    if grid_size == 0 {
        return Err(Error::Validation("Empty calibration grid.".into()));
    }
    if grid_size > 200 {
        return Err(Error::Validation(format!(
            "Grid size {grid_size} exceeds the 200-cell cap; narrow the sweep."
        )));
    }

    let mut runs: Vec<SweepRun> = Vec::with_capacity(grid_size);
    for &points_scale in &axes_points {
        for &shrinkage in &axes_shrink {
            for &k_factor in &axes_k {
                for &home_ice_elo in &axes_home {
                    for &trials in &axes_trials {
                        let knobs = CalibrationKnobs {
                            points_scale,
                            shrinkage,
                            k_factor,
                            home_ice_elo,
                            trials,
                        };
                        let report =
                            calibrate_season_with_knobs(db, nhl, season, &knobs).await?;
                        let brier_aggregate = report.brier_r1
                            + report.brier_r2
                            + report.brier_r3
                            + report.brier_cup;
                        runs.push(SweepRun {
                            knobs,
                            brier_r1: report.brier_r1,
                            brier_r2: report.brier_r2,
                            brier_r3: report.brier_r3,
                            brier_cup: report.brier_cup,
                            log_loss_r1: report.log_loss_r1,
                            log_loss_cup: report.log_loss_cup,
                            brier_aggregate,
                        });
                    }
                }
            }
        }
    }

    runs.sort_by(|a, b| {
        a.brier_aggregate
            .partial_cmp(&b.brier_aggregate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best = runs
        .first()
        .cloned()
        .ok_or_else(|| Error::Internal("Sweep produced no runs".into()))?;
    Ok(SweepReport {
        season,
        grid_size,
        best,
        runs,
    })
}

