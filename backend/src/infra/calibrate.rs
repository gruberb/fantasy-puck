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
    playoff_elo::{self},
    race_sim::{
        self, simulate, BracketState, RaceSimInput, SeriesState, SimFantasyTeam, SimPlayer,
        TeamRating, DEFAULT_K_FACTOR, DEFAULT_TRIALS, HOME_ICE_ELO,
    },
};
use crate::error::{Error, Result};

const ELO_K_FACTOR: f32 = std::f32::consts::LN_10 / 400.0;

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

/// Run one calibration pass against a completed historical season.
pub async fn calibrate_season(
    db: &FantasyDb,
    nhl: &crate::NhlClient,
    season: u32,
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

    // 4. Seed Elo + home-ice from the NHL standings endpoint. For
    //    historical seasons we have to pass the season; the NHL API
    //    returns current standings so this is a limitation of the MVP.
    //    A future improvement fetches `/standings/2022-05-02` style
    //    snapshots but requires plumbing not built yet.
    let standings = nhl.get_standings_raw().await.ok();
    let (ratings, k_factor, home_ice_bonus) = build_ratings(standings.as_ref());

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
    let trials = DEFAULT_TRIALS;
    let output = tokio::task::spawn_blocking(move || simulate(&sim_input, trials))
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
) -> (HashMap<String, TeamRating>, f32, f32) {
    let Some(standings) = standings else {
        return (HashMap::new(), DEFAULT_K_FACTOR, 0.0);
    };
    let elo = playoff_elo::seed_from_standings(standings);
    let home_bonus_map = playoff_elo::home_bonus_from_standings(standings);
    let map: HashMap<String, TeamRating> = elo
        .into_iter()
        .map(|(abbrev, base)| {
            let home_bonus = home_bonus_map.get(&abbrev).copied().unwrap_or(0.0);
            (abbrev, TeamRating::with_home_bonus(base, home_bonus))
        })
        .collect();
    let fallback_ice_bonus = ELO_K_FACTOR * HOME_ICE_ELO;
    (map, ELO_K_FACTOR, fallback_ice_bonus)
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

