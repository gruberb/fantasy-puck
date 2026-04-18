//! Backtest scaffolding — calibration metrics + state reconstruction
//! helpers for evaluating race-odds predictions against realized outcomes.
//!
//! Pure-domain module: no DB, no HTTP. The metric helpers take paired
//! (prediction, outcome) vectors and return Brier, log-loss,
//! calibration-curve buckets, MAE, RMSE, and p10–p90 coverage.
//! [`reconstruct_bracket_from_results`] folds a chronological list of
//! completed-game rows into a `BracketState`, inferring rounds from
//! bracket topology rather than trusting the schedule endpoint's
//! unreliable `series_status.round` field.

use std::collections::{HashMap, HashSet};

use super::race_sim::{BracketState, SeriesState};

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

/// One completed game from the DB.
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub game_date: String,
    pub home_team: String,
    pub away_team: String,
    pub home_score: i32,
    pub away_score: i32,
    /// Declared playoff round, if the ingest recorded one. NOT trusted
    /// by [`reconstruct_bracket_from_results`] — rounds are inferred
    /// topologically because the NHL schedule endpoint returns this
    /// inconsistently for historical games.
    pub round: Option<u8>,
}

/// Reconstruct the `BracketState` implied by a list of completed
/// playoff games.
///
/// Approach: group every game into a series by canonical team-pair,
/// compute each series' winner + total games, then assign rounds by
/// **topology + chronology**:
///
///   1. **R1** = the first 8 series by first-game date.
///   2. **R2** = next 4 series in date order whose *both* participants
///      are R1 winners.
///   3. **R3** = next 2 series whose both participants are R2 winners.
///   4. **Cup Final** = the next series whose both participants are R3
///      winners (at most 1).
///
/// The `round` column on input rows is intentionally ignored. Historical
/// NHL schedule responses don't reliably populate `series_status.round`,
/// so trusting it would silently drop Cup Finals and conference-finals
/// into R1's bucket where they'd get truncated.
///
/// Missing slots at any round are padded with `SeriesState::Future`.
/// An in-progress series (neither side at 4 wins) still counts toward
/// slot allocation but its winner is `None`, so no downstream round
/// will pick it as a feeder.
pub fn reconstruct_bracket_from_results(results: &[ResultRow]) -> BracketState {
    const SLOT_COUNTS: [usize; 4] = [8, 4, 2, 1];

    // 1. Collapse per-game rows into series, keyed by canonical
    //    alphabetical team-pair.
    let mut series_order: Vec<(String, String)> = Vec::new();
    let mut series_data: HashMap<(String, String), SeriesAgg> = HashMap::new();
    for g in results {
        let (a, b) = canonical_pair(&g.home_team, &g.away_team);
        let key = (a.clone(), b.clone());
        let agg = series_data.entry(key.clone()).or_insert_with(|| {
            series_order.push(key.clone());
            SeriesAgg::new(a.clone(), b.clone(), g.game_date.clone())
        });
        agg.add_game(g);
    }

    // 2. Build list of series, sorted by first-game date.
    let mut series_list: Vec<SeriesAgg> =
        series_order.iter().map(|k| series_data[k].clone()).collect();
    series_list.sort_by(|a, b| a.first_date.cmp(&b.first_date));

    // 3. R1: walk series in date order; a series is R1-eligible the
    //    first time AT LEAST ONE of its teams appears in the data. This
    //    distinguishes "initial matchups" (both teams brand-new to the
    //    bracket) from rematches in later rounds (both teams already
    //    seen). Cap at 8 to match the bracket size.
    let mut consumed = vec![false; series_list.len()];
    let mut seen_teams: HashSet<String> = HashSet::new();
    let mut r1_indices: Vec<usize> = Vec::new();
    for i in 0..series_list.len() {
        if r1_indices.len() >= SLOT_COUNTS[0] {
            break;
        }
        let s = &series_list[i];
        let introduces_new_team =
            !seen_teams.contains(&s.team_a) || !seen_teams.contains(&s.team_b);
        if introduces_new_team {
            r1_indices.push(i);
            seen_teams.insert(s.team_a.clone());
            seen_teams.insert(s.team_b.clone());
        }
    }
    for &i in &r1_indices {
        consumed[i] = true;
    }
    let mut rounds: Vec<Vec<SeriesState>> = Vec::with_capacity(4);
    let mut prev_winners = collect_winners(&series_list, &r1_indices);
    rounds.push(materialize_round(&series_list, &r1_indices, SLOT_COUNTS[0]));

    // 4. R2, R3, Cup Final: each round takes the next date-ordered
    //    unclaimed series whose both participants are winners of the
    //    previous round.
    for r in 1..SLOT_COUNTS.len() {
        let mut picks: Vec<usize> = Vec::new();
        for i in 0..series_list.len() {
            if consumed[i] {
                continue;
            }
            let s = &series_list[i];
            if prev_winners.contains(&s.team_a) && prev_winners.contains(&s.team_b) {
                picks.push(i);
                if picks.len() >= SLOT_COUNTS[r] {
                    break;
                }
            }
        }
        for &i in &picks {
            consumed[i] = true;
        }
        prev_winners = collect_winners(&series_list, &picks);
        rounds.push(materialize_round(&series_list, &picks, SLOT_COUNTS[r]));
    }

    BracketState { rounds }
}

// --- internals ---

fn canonical_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[derive(Debug, Clone)]
struct SeriesAgg {
    team_a: String,
    team_b: String,
    wins_a: u32,
    wins_b: u32,
    first_date: String,
}

impl SeriesAgg {
    fn new(team_a: String, team_b: String, first_date: String) -> Self {
        Self {
            team_a,
            team_b,
            wins_a: 0,
            wins_b: 0,
            first_date,
        }
    }

    fn add_game(&mut self, g: &ResultRow) {
        // Determine the winning abbrev from scores + team names, not
        // from home/away. Some historical rows have home/away labels
        // swapped but the abbrevs + scores remain self-consistent.
        let winner_abbrev = if g.home_score > g.away_score {
            &g.home_team
        } else {
            &g.away_team
        };
        if *winner_abbrev == self.team_a {
            self.wins_a += 1;
        } else {
            self.wins_b += 1;
        }
        if g.game_date < self.first_date {
            self.first_date = g.game_date.clone();
        }
    }

    fn to_state(&self) -> SeriesState {
        if self.wins_a >= 4 {
            SeriesState::Completed {
                winner: self.team_a.clone(),
                loser: self.team_b.clone(),
                total_games: self.wins_a + self.wins_b,
            }
        } else if self.wins_b >= 4 {
            SeriesState::Completed {
                winner: self.team_b.clone(),
                loser: self.team_a.clone(),
                total_games: self.wins_a + self.wins_b,
            }
        } else {
            SeriesState::InProgress {
                top_team: self.team_a.clone(),
                top_wins: self.wins_a,
                bottom_team: self.team_b.clone(),
                bottom_wins: self.wins_b,
            }
        }
    }

    fn winner(&self) -> Option<&str> {
        if self.wins_a >= 4 {
            Some(&self.team_a)
        } else if self.wins_b >= 4 {
            Some(&self.team_b)
        } else {
            None
        }
    }
}

fn collect_winners(series: &[SeriesAgg], indices: &[usize]) -> HashSet<String> {
    indices
        .iter()
        .filter_map(|&i| series[i].winner().map(|s| s.to_string()))
        .collect()
}

fn materialize_round(
    series: &[SeriesAgg],
    indices: &[usize],
    slot_count: usize,
) -> Vec<SeriesState> {
    let mut out: Vec<SeriesState> = indices.iter().map(|&i| series[i].to_state()).collect();
    while out.len() < slot_count {
        out.push(SeriesState::Future);
    }
    out.truncate(slot_count);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str, home: &str, away: &str, hs: i32, as_: i32) -> ResultRow {
        ResultRow {
            game_date: date.into(),
            home_team: home.into(),
            away_team: away.into(),
            home_score: hs,
            away_score: as_,
            round: None,
        }
    }

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
        assert!(log_loss(&preds) < 1e-5);
    }

    #[test]
    fn calibration_curve_groups_into_deciles() {
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
        assert!((mae(&pairs) - 1.0).abs() < 1e-5);
        assert!((rmse(&pairs) - ((5.0_f32) / 3.0).sqrt()).abs() < 1e-5);
    }

    #[test]
    fn interval_coverage_reports_fraction_inside() {
        let samples = vec![
            (0.0f32, 10.0, 5.0),
            (0.0, 10.0, 12.0),
            (0.0, 10.0, -1.0),
            (0.0, 10.0, 0.0),
        ];
        assert!((interval_coverage(&samples) - 0.5).abs() < 1e-6);
    }

    // ------- bracket reconstruction -------

    #[test]
    fn reconstruct_single_completed_series_as_completed() {
        let rows = vec![
            row("2023-04-17", "BOS", "BUF", 4, 2),
            row("2023-04-19", "BUF", "BOS", 3, 2),
            row("2023-04-21", "BOS", "BUF", 5, 4),
            row("2023-04-23", "BUF", "BOS", 1, 3),
            row("2023-04-25", "BOS", "BUF", 4, 1),
        ];
        let bracket = reconstruct_bracket_from_results(&rows);
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
            row("2023-04-17", "BOS", "BUF", 4, 2),
            row("2023-04-19", "BUF", "BOS", 3, 2),
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
        let rows = vec![row("2023-04-17", "BOS", "BUF", 4, 1)];
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

    #[test]
    fn reconstruct_assigns_rounds_topologically_even_when_column_lies() {
        // Miniature 4-team bracket: A vs B and C vs D in R1; R2 pairs
        // the winners. All ResultRows have round=None (simulating the
        // bad-data case) and round=3 for the actual R2 series to prove
        // the input column is ignored.
        let mut rows = Vec::new();
        // R1: A sweeps B.
        for d in ["2023-04-17", "2023-04-19", "2023-04-21", "2023-04-23"] {
            rows.push(row(d, "A", "B", 4, 2));
        }
        // R1: C sweeps D.
        for d in ["2023-04-18", "2023-04-20", "2023-04-22", "2023-04-24"] {
            rows.push(row(d, "C", "D", 5, 1));
        }
        // R2 (A vs C, A sweeps) — tagged round=99 deliberately to prove
        // the column isn't consulted.
        for d in ["2023-05-01", "2023-05-03", "2023-05-05", "2023-05-07"] {
            let mut r = row(d, "A", "C", 3, 1);
            r.round = Some(99);
            rows.push(r);
        }
        let bracket = reconstruct_bracket_from_results(&rows);
        // R1 slot 0 = A beat B (first game date April 17).
        match &bracket.rounds[0][0] {
            SeriesState::Completed { winner, .. } => assert_eq!(winner, "A"),
            other => panic!("expected A-B Completed, got {:?}", other),
        }
        // R1 slot 1 = C beat D (first game April 18).
        match &bracket.rounds[0][1] {
            SeriesState::Completed { winner, .. } => assert_eq!(winner, "C"),
            other => panic!("expected C-D Completed, got {:?}", other),
        }
        // R2 slot 0 = A beat C despite round=99 on the rows.
        match &bracket.rounds[1][0] {
            SeriesState::Completed { winner, loser, .. } => {
                assert_eq!(winner, "A");
                assert_eq!(loser, "C");
            }
            other => panic!("expected A-C Completed in R2, got {:?}", other),
        }
    }

    #[test]
    fn reconstruct_promotes_cup_final_from_r3_winners() {
        // Full 2023-style bracket: 8 R1 series, 4 R2, 2 R3, 1 Final.
        // Exercises that Cup Final gets inferred even when every row's
        // round column is None.
        let mut rows = Vec::new();
        // R1 — 8 series: 1-16, 2-15, 3-14, 4-13, 5-12, 6-11, 7-10, 8-9.
        // Use simple alphabetical abbrevs to keep pairings clear.
        let r1_pairs = [
            ("AA", "AB"),
            ("AC", "AD"),
            ("AE", "AF"),
            ("AG", "AH"),
            ("BA", "BB"),
            ("BC", "BD"),
            ("BE", "BF"),
            ("BG", "BH"),
        ];
        for (i, (w, l)) in r1_pairs.iter().enumerate() {
            for g in 0..4 {
                rows.push(row(
                    &format!("2023-04-{:02}", 15 + i * 2 + g),
                    w,
                    l,
                    4,
                    0,
                ));
            }
        }
        // R2 — 4 series: AA beats AC, AE beats AG, BA beats BC, BE beats BG.
        let r2_pairs = [("AA", "AC"), ("AE", "AG"), ("BA", "BC"), ("BE", "BG")];
        for (i, (w, l)) in r2_pairs.iter().enumerate() {
            for g in 0..4 {
                rows.push(row(
                    &format!("2023-05-{:02}", 1 + i * 2 + g),
                    w,
                    l,
                    4,
                    0,
                ));
            }
        }
        // R3 — 2 series: AA beats AE, BA beats BE.
        for (i, (w, l)) in [("AA", "AE"), ("BA", "BE")].iter().enumerate() {
            for g in 0..4 {
                rows.push(row(
                    &format!("2023-05-{:02}", 15 + i * 2 + g),
                    w,
                    l,
                    4,
                    0,
                ));
            }
        }
        // Cup Final — AA beats BA.
        for g in 0..4 {
            rows.push(row(&format!("2023-06-{:02}", 5 + g), "AA", "BA", 4, 0));
        }
        let bracket = reconstruct_bracket_from_results(&rows);
        match &bracket.rounds[3][0] {
            SeriesState::Completed { winner, loser, .. } => {
                assert_eq!(winner, "AA");
                assert_eq!(loser, "BA");
            }
            other => panic!("expected AA-BA Cup Final Completed, got {:?}", other),
        }
    }
}
