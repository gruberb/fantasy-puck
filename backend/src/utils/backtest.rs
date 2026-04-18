//! Backtest scaffolding — calibration metrics + state reconstruction
//! helpers for evaluating race-odds predictions against realized outcomes.
//!
//! Current scope is deliberately minimal: pure metric helpers that take
//! paired (prediction, outcome) vectors and return Brier score, log-loss,
//! calibration-curve buckets, MAE, RMSE, and p10–p90 coverage. A full
//! "run the sim as-of historical date X" pipeline requires per-day
//! snapshots of the bracket state and roster compositions that we
//! haven't persisted yet; that belongs to a later iteration.
//!
//! The one non-metric function exposed here —
//! [`reconstruct_bracket_from_results`] — re-derives a full `BracketState`
//! from a chronological stream of [`playoff_game_results`] rows by
//! grouping consecutive games between the same two teams into series.
//! This is the hook a future full-backtest harness will build on.

use std::collections::HashMap;

use crate::utils::race_sim::{BracketState, SeriesState};

// ---------------------------------------------------------------------------
// Binary-outcome metrics (series winners, cup winners, …)
// ---------------------------------------------------------------------------

/// Brier score — mean squared error between predicted probability and
/// observed 0/1 outcome. Range: `[0, 1]`, lower is better. Perfect
/// prediction on a true event gets 0; a 50/50 forecast on any outcome
/// gets 0.25.
pub fn brier_score(predictions: &[(f32, bool)]) -> f32 {
    if predictions.is_empty() {
        return 0.0;
    }
    let n = predictions.len() as f32;
    predictions
        .iter()
        .map(|(p, o)| {
            let y = if *o { 1.0 } else { 0.0 };
            (p - y).powi(2)
        })
        .sum::<f32>()
        / n
}

/// Log-loss (cross-entropy) over binary outcomes. Range: `[0, ∞)`,
/// lower is better. `predictions` is clipped to `[ε, 1-ε]` to avoid
/// `-inf` when a model assigns 0 probability to an event that happened.
pub fn log_loss(predictions: &[(f32, bool)]) -> f32 {
    const EPS: f32 = 1e-7;
    if predictions.is_empty() {
        return 0.0;
    }
    let n = predictions.len() as f32;
    predictions
        .iter()
        .map(|(p, o)| {
            let p = p.clamp(EPS, 1.0 - EPS);
            if *o {
                -p.ln()
            } else {
                -(1.0 - p).ln()
            }
        })
        .sum::<f32>()
        / n
}

/// A single calibration bucket: all predictions whose value falls inside
/// `[lower, upper)`, their average predicted probability, and the
/// empirical fraction of true outcomes inside the bucket. Target: for a
/// well-calibrated model, `avg_predicted ≈ observed_rate` per bucket.
#[derive(Debug, Clone)]
pub struct CalibrationBucket {
    pub lower: f32,
    pub upper: f32,
    pub count: usize,
    pub avg_predicted: f32,
    pub observed_rate: f32,
}

/// Group predictions into equal-width probability buckets and report
/// the per-bucket (avg predicted, observed rate). `bucket_count = 10`
/// gives the standard 0.0–0.1, 0.1–0.2, … split. Empty buckets are
/// omitted.
pub fn calibration_curve(predictions: &[(f32, bool)], bucket_count: usize) -> Vec<CalibrationBucket> {
    if predictions.is_empty() || bucket_count == 0 {
        return Vec::new();
    }
    let mut sums = vec![0.0f32; bucket_count];
    let mut hits = vec![0u32; bucket_count];
    let mut counts = vec![0usize; bucket_count];

    for (p, o) in predictions {
        let p = p.clamp(0.0, 1.0);
        let idx = ((p * bucket_count as f32) as usize).min(bucket_count - 1);
        sums[idx] += p;
        counts[idx] += 1;
        if *o {
            hits[idx] += 1;
        }
    }

    (0..bucket_count)
        .filter(|i| counts[*i] > 0)
        .map(|i| {
            let c = counts[i] as f32;
            CalibrationBucket {
                lower: i as f32 / bucket_count as f32,
                upper: (i + 1) as f32 / bucket_count as f32,
                count: counts[i],
                avg_predicted: sums[i] / c,
                observed_rate: hits[i] as f32 / c,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Regression metrics (player remaining points, team projected totals, …)
// ---------------------------------------------------------------------------

/// Mean absolute error between predicted and realized values.
pub fn mae(pairs: &[(f32, f32)]) -> f32 {
    if pairs.is_empty() {
        return 0.0;
    }
    let n = pairs.len() as f32;
    pairs.iter().map(|(p, a)| (p - a).abs()).sum::<f32>() / n
}

/// Root-mean-squared error.
pub fn rmse(pairs: &[(f32, f32)]) -> f32 {
    if pairs.is_empty() {
        return 0.0;
    }
    let n = pairs.len() as f32;
    (pairs.iter().map(|(p, a)| (p - a).powi(2)).sum::<f32>() / n).sqrt()
}

/// Fraction of realized values that fell inside the predicted
/// `[p10, p90]` interval. Target for a well-calibrated 80% interval is
/// 0.80. Input tuples are `(p10, p90, actual)`.
pub fn interval_coverage(samples: &[(f32, f32, f32)]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len() as f32;
    samples
        .iter()
        .filter(|(lo, hi, actual)| *actual >= *lo && *actual <= *hi)
        .count() as f32
        / n
}

// ---------------------------------------------------------------------------
// State reconstruction
// ---------------------------------------------------------------------------

/// One completed game from the DB — a thinner shape than the full model
/// row so callers can load whatever subset they have.
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub home_team: String,
    pub away_team: String,
    pub home_score: i32,
    pub away_score: i32,
    pub round: Option<u8>,
}

/// Reconstruct the `BracketState` implied by a chronological list of
/// completed playoff games.
///
/// Grouping rule: two consecutive-in-time games between the same pair
/// of teams belong to the same series. Each series terminates when one
/// team reaches 4 wins (best-of-7 convention).
///
/// For rounds without a full 8/4/2/1-slot slate (e.g. mid-round 2 when
/// only one series has been played), missing slots are padded with
/// `SeriesState::Future`. Positional pairing within each round is
/// preserved in the order the series were encountered.
///
/// Limitations:
/// - Doesn't distinguish East vs West conference without help; the
///   caller is responsible for passing games grouped by conference if
///   that matters to them.
/// - When `round` is `None` in the input, all games fall into round 1.
/// - If a series is interrupted mid-stream (no team hit 4 wins), it's
///   represented as `InProgress` with the current wins.
pub fn reconstruct_bracket_from_results(results: &[ResultRow]) -> BracketState {
    const SLOT_COUNTS: [usize; 4] = [8, 4, 2, 1];
    // Bucket games by round (default to round 1).
    let mut per_round: [Vec<&ResultRow>; 4] = Default::default();
    for r in results {
        let idx = r.round.map(|x| x as usize).unwrap_or(1).saturating_sub(1);
        if idx < 4 {
            per_round[idx].push(r);
        }
    }

    let mut rounds: Vec<Vec<SeriesState>> = Vec::with_capacity(4);
    for (idx, bucket) in per_round.iter().enumerate() {
        let mut series: Vec<SeriesState> = Vec::new();
        let mut series_wins: HashMap<(String, String), (u32, u32)> = HashMap::new();
        let mut encounter_order: Vec<(String, String)> = Vec::new();
        for g in bucket {
            // Canonical pair key — sort abbrevs alphabetically so
            // (BOS, BUF) and (BUF, BOS) hash the same.
            let (a, b) = if g.home_team <= g.away_team {
                (&g.home_team, &g.away_team)
            } else {
                (&g.away_team, &g.home_team)
            };
            let key = (a.clone(), b.clone());
            let wins = series_wins.entry(key.clone()).or_insert_with(|| {
                encounter_order.push(key.clone());
                (0, 0)
            });
            // `wins.0` counts wins for `a`, `wins.1` for `b`.
            if g.home_score > g.away_score {
                if &g.home_team == a {
                    wins.0 += 1;
                } else {
                    wins.1 += 1;
                }
            } else if &g.away_team == a {
                wins.0 += 1;
            } else {
                wins.1 += 1;
            }
        }
        for (a, b) in encounter_order {
            let (wa, wb) = series_wins.get(&(a.clone(), b.clone())).copied().unwrap_or((0, 0));
            let state = if wa >= 4 {
                SeriesState::Completed {
                    winner: a.clone(),
                    loser: b.clone(),
                    total_games: wa + wb,
                }
            } else if wb >= 4 {
                SeriesState::Completed {
                    winner: b.clone(),
                    loser: a.clone(),
                    total_games: wa + wb,
                }
            } else {
                SeriesState::InProgress {
                    top_team: a.clone(),
                    top_wins: wa,
                    bottom_team: b.clone(),
                    bottom_wins: wb,
                }
            };
            series.push(state);
        }
        while series.len() < SLOT_COUNTS[idx] {
            series.push(SeriesState::Future);
        }
        series.truncate(SLOT_COUNTS[idx]);
        rounds.push(series);
    }
    BracketState { rounds }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------- binary-outcome metrics -------

    #[test]
    fn brier_zero_for_perfect_prediction() {
        let preds = vec![(1.0, true), (0.0, false), (1.0, true)];
        assert!(brier_score(&preds).abs() < 1e-6);
    }

    #[test]
    fn brier_equals_quarter_for_coinflip_forecast() {
        let preds = vec![(0.5, true), (0.5, false), (0.5, true), (0.5, false)];
        assert!((brier_score(&preds) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn log_loss_is_zero_for_perfect_prediction() {
        let preds = vec![(1.0, true), (0.0, false)];
        // Exactly at the eps-clamp boundary, so log-loss is small but
        // nonzero. Assert it's under 1e-5 rather than exactly zero.
        assert!(log_loss(&preds) < 1e-5);
    }

    #[test]
    fn calibration_curve_groups_into_deciles() {
        // 20 predictions, 10 in bucket 0.2-0.3 (6 hits), 10 in 0.7-0.8 (7 hits).
        let preds: Vec<(f32, bool)> = (0..10)
            .map(|i| (0.25, i < 6))
            .chain((0..10).map(|i| (0.75, i < 7)))
            .collect();
        let curve = calibration_curve(&preds, 10);
        let b2 = curve.iter().find(|b| (b.lower - 0.2).abs() < 1e-3).unwrap();
        let b7 = curve.iter().find(|b| (b.lower - 0.7).abs() < 1e-3).unwrap();
        assert_eq!(b2.count, 10);
        assert!((b2.observed_rate - 0.6).abs() < 1e-3);
        assert_eq!(b7.count, 10);
        assert!((b7.observed_rate - 0.7).abs() < 1e-3);
    }

    // ------- regression metrics -------

    #[test]
    fn mae_rmse_on_known_set() {
        let pairs = vec![(10.0, 11.0), (20.0, 18.0), (5.0, 5.0)];
        assert!((mae(&pairs) - 1.0).abs() < 1e-5); // (1+2+0)/3
        // sqrt((1+4+0)/3) ≈ 1.291
        assert!((rmse(&pairs) - ((5.0_f32) / 3.0).sqrt()).abs() < 1e-5);
    }

    #[test]
    fn interval_coverage_reports_fraction_inside() {
        let samples = vec![
            (0.0f32, 10.0, 5.0), // in
            (0.0, 10.0, 12.0),   // out
            (0.0, 10.0, -1.0),   // out
            (0.0, 10.0, 0.0),    // in (boundary)
        ];
        assert!((interval_coverage(&samples) - 0.5).abs() < 1e-6);
    }

    // ------- bracket reconstruction -------

    #[test]
    fn reconstruct_single_completed_series_as_completed() {
        let rows = vec![
            ResultRow {
                home_team: "BOS".into(),
                away_team: "BUF".into(),
                home_score: 4,
                away_score: 2,
                round: Some(1),
            },
            ResultRow {
                home_team: "BUF".into(),
                away_team: "BOS".into(),
                home_score: 3,
                away_score: 2,
                round: Some(1),
            },
            ResultRow {
                home_team: "BOS".into(),
                away_team: "BUF".into(),
                home_score: 5,
                away_score: 4,
                round: Some(1),
            },
            ResultRow {
                home_team: "BUF".into(),
                away_team: "BOS".into(),
                home_score: 1,
                away_score: 3,
                round: Some(1),
            },
            ResultRow {
                home_team: "BOS".into(),
                away_team: "BUF".into(),
                home_score: 4,
                away_score: 1,
                round: Some(1),
            },
        ];
        let bracket = reconstruct_bracket_from_results(&rows);
        // BOS won 4-1.
        match &bracket.rounds[0][0] {
            SeriesState::Completed {
                winner,
                loser,
                total_games,
            } => {
                assert_eq!(winner, "BOS");
                assert_eq!(loser, "BUF");
                assert_eq!(*total_games, 5);
            }
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn reconstruct_partial_series_as_in_progress() {
        let rows = vec![
            ResultRow {
                home_team: "BOS".into(),
                away_team: "BUF".into(),
                home_score: 4,
                away_score: 2,
                round: Some(1),
            },
            ResultRow {
                home_team: "BUF".into(),
                away_team: "BOS".into(),
                home_score: 3,
                away_score: 2,
                round: Some(1),
            },
        ];
        let bracket = reconstruct_bracket_from_results(&rows);
        match &bracket.rounds[0][0] {
            SeriesState::InProgress {
                top_team,
                top_wins,
                bottom_team,
                bottom_wins,
            } => {
                // Alphabetical canonicalization: "BOS" < "BUF", so BOS
                // is the top slot. BOS won game 1, BUF won game 2.
                assert_eq!(top_team, "BOS");
                assert_eq!(*top_wins, 1);
                assert_eq!(bottom_team, "BUF");
                assert_eq!(*bottom_wins, 1);
            }
            other => panic!("expected InProgress, got {:?}", other),
        }
    }

    #[test]
    fn reconstruct_pads_missing_slots() {
        // Only one R1 series played. Other 7 R1 slots and all of R2–F
        // must be Future.
        let rows = vec![ResultRow {
            home_team: "BOS".into(),
            away_team: "BUF".into(),
            home_score: 4,
            away_score: 1,
            round: Some(1),
        }];
        let bracket = reconstruct_bracket_from_results(&rows);
        assert_eq!(bracket.rounds.len(), 4);
        assert_eq!(bracket.rounds[0].len(), 8);
        for slot in &bracket.rounds[0][1..] {
            assert!(matches!(slot, SeriesState::Future));
        }
        for round in &bracket.rounds[1..] {
            for slot in round {
                assert!(matches!(slot, SeriesState::Future));
            }
        }
    }
}
