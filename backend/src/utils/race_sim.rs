//! Team-correlated Monte Carlo simulation of the fantasy-points race.
//!
//! Pure-domain module: no HTTP, no SQL, no logging of request state. Takes a
//! fully-built `RaceSimInput` and returns a `RaceSimOutput` describing each
//! fantasy team's (and player's) projected final total plus win probability.
//!
//! The engine simulates the whole bracket end-to-end for N trials:
//!   1. Every in-progress first-round series is resolved into a winner and a
//!      game count by simulating individual games. Per-game win probability
//!      is a logistic of the team-strength gap, biased by current series
//!      state — this follows the hockeystats.com methodology where per-game
//!      odds come from team strength and series outcome emerges from
//!      iterated per-game draws, rather than from a pre-baked series-state
//!      table.
//!   2. Round winners pair up the bracket by series-letter convention
//!      (A+B, C+D, E+F, G+H, then those pairs) through the Cup Final. Later
//!      rounds start 0-0 and inherit team strengths.
//!   3. Each playoff team ends the trial with a `games_played_this_run`
//!      shared by every player on that roster — teammates are correlated by
//!      construction, and cross-roster correlation (two fantasy teams both
//!      rostering an Oilers skater) falls out automatically.
//!   4. For each skater, remaining fantasy points are drawn from a Poisson
//!      around `ppg * games_remaining`. Realised playoff points are added
//!      on top.
//!
//! Callers are expected to run [`simulate`] inside `spawn_blocking`: for the
//! default 5000 trials across a full bracket and ~100 players we're in the
//! tens-of-milliseconds range, which is too long to sit on the async
//! runtime on a hot request path.

use std::collections::{HashMap, HashSet};

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// State of one currently-active first-round series.
///
/// `series_letter` (A..H) determines round-2 bracket pairing — the NHL
/// convention is A↔B, C↔D, E↔F, G↔H into round 2, then those winners pair
/// into round 3, and the conference winners meet in the final.
#[derive(Debug, Clone)]
pub struct CurrentSeries {
    pub series_letter: String,
    pub top_team: String,
    pub top_wins: u32,
    pub bottom_team: String,
    pub bottom_wins: u32,
}

/// Team strength scalar — regular-season standings points or equivalent.
///
/// Only the *relative* value across teams matters; the engine feeds the gap
/// through a logistic, so absolute scale is absorbed by [`RaceSimInput::k_factor`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TeamRating(pub f32);

#[derive(Debug, Clone)]
pub struct SimPlayer {
    pub nhl_id: i64,
    pub name: String,
    pub nhl_team: String,
    pub position: String,
    /// Points already accrued in these playoffs (realised, locked in).
    pub playoff_points_so_far: i32,
    /// Fantasy-points expectation per game. Callers should supply regular-
    /// season PPG where known and a position-neutral prior otherwise.
    pub ppg: f32,
    /// Optional headshot URL used only by the Fantasy Champion view.
    pub image_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SimFantasyTeam {
    pub team_id: i64,
    pub team_name: String,
    pub players: Vec<SimPlayer>,
}

#[derive(Debug, Clone)]
pub struct RaceSimInput {
    /// All in-progress first-round series. Later rounds are simulated from
    /// 0-0 states because no NHL series ever exists past round 1 before
    /// round 1 completes.
    pub round1: Vec<CurrentSeries>,
    /// Games already played per team across the whole playoff run so far.
    /// For round 1 this is `top_wins + bottom_wins` of the team's current
    /// series.
    pub games_played_so_far: HashMap<String, u32>,
    /// Team-strength scalar keyed by NHL abbrev. Teams absent from this map
    /// are treated as league-average.
    pub ratings: HashMap<String, TeamRating>,
    /// Logistic scale factor: `p_game = sigmoid(k * (rating_a - rating_b))`.
    /// For standings-points ratings with ~40-point spreads, `k ≈ 0.03` gives
    /// a sensible 5–15% advantage for a top team over a wildcard. Callers
    /// can tune or recalibrate.
    pub k_factor: f32,
    pub fantasy_teams: Vec<SimFantasyTeam>,
}

/// Default Monte Carlo trial count. Tuned so win probability resolution is
/// ±~1pp at 95% confidence without burning more than a CPU-tenth per call.
pub const DEFAULT_TRIALS: usize = 5000;

/// Default logistic scale when caller doesn't supply one.
///
/// Calibration: tuned against HockeyStats.com's published round-1 series
/// probabilities for the 2026 playoffs (e.g. COL vs LAK series win ≈ 65%,
/// EDM vs ANA ≈ 70%). At `k = 0.010` our series-win probabilities land
/// within ~3pp of those references across most matchups. The earlier value
/// (`0.03`) over-concentrated Cup probability on the top standings seed —
/// Colorado came out at ~39% to win the Cup where HockeyStats has them at
/// ~13%. Keeping the logistic but lowering its slope preserves the "team
/// strength matters" signal without amplifying favorites into inevitability.
pub const DEFAULT_K_FACTOR: f32 = 0.010;

/// Default fantasy PPG prior when a skater's regular-season rate is unknown.
pub const DEFAULT_PPG: f32 = 0.45;

// ---------------------------------------------------------------------------
// Outputs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamOdds {
    pub team_id: i64,
    pub team_name: String,
    pub current_points: i32,
    pub projected_final_mean: f32,
    pub projected_final_median: f32,
    pub p10: f32,
    pub p90: f32,
    pub win_prob: f32,
    pub top3_prob: f32,
    /// Exact MC pairwise probability: `head_to_head[opponent_team_id]` is
    /// `P(this team finishes strictly ahead of that opponent)`. Populated
    /// from per-trial score comparisons, so it's consistent with the sort
    /// used for `win_prob` and doesn't rely on a normal approximation.
    #[serde(default)]
    pub head_to_head: HashMap<i64, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerOdds {
    pub nhl_id: i64,
    pub name: String,
    pub nhl_team: String,
    pub position: String,
    pub current_points: i32,
    pub projected_final_mean: f32,
    pub projected_final_median: f32,
    pub p10: f32,
    pub p90: f32,
    pub image_url: Option<String>,
}

/// Per-NHL-team playoff projection distilled from the same MC sweep that
/// produces fantasy-team odds. One entry per team active in `round1` of the
/// input; drops out as teams get eliminated (still present, with zero
/// probabilities for rounds they never reach).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NhlTeamOdds {
    pub abbrev: String,
    /// P(team wins its first-round series).
    pub advance_round1_prob: f32,
    /// P(team reaches the Conference Finals).
    pub conference_finals_prob: f32,
    /// P(team reaches the Stanley Cup Final).
    pub cup_finals_prob: f32,
    /// P(team wins the Stanley Cup).
    pub cup_win_prob: f32,
    /// Mean number of games this team plays across the whole playoff run
    /// (including games already played). Useful for fantasy point projections.
    pub expected_games: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceSimOutput {
    pub trials: usize,
    pub teams: Vec<TeamOdds>,
    pub players: Vec<PlayerOdds>,
    /// Per-NHL-team playoff projections. Empty only when `round1` input was
    /// empty (pre-playoffs or data fetch failed).
    #[serde(default)]
    pub nhl_teams: Vec<NhlTeamOdds>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Run the Monte Carlo. Deterministic with a seeded RNG via
/// [`simulate_with_seed`]; production callers should use [`simulate`] which
/// draws from `SmallRng::from_entropy`.
pub fn simulate(input: &RaceSimInput, trials: usize) -> RaceSimOutput {
    let mut rng = SmallRng::from_entropy();
    run(input, trials, &mut rng)
}

pub fn simulate_with_seed(input: &RaceSimInput, trials: usize, seed: u64) -> RaceSimOutput {
    let mut rng = SmallRng::seed_from_u64(seed);
    run(input, trials, &mut rng)
}

fn run(input: &RaceSimInput, trials: usize, rng: &mut SmallRng) -> RaceSimOutput {
    // Flatten roster so per-player accumulators are array-indexed.
    let mut players: Vec<&SimPlayer> = Vec::new();
    let mut player_team_index: Vec<usize> = Vec::new();
    for (team_idx, team) in input.fantasy_teams.iter().enumerate() {
        for p in &team.players {
            players.push(p);
            player_team_index.push(team_idx);
        }
    }

    let n_teams = input.fantasy_teams.len();
    let n_players = players.len();

    let mut team_samples: Vec<Vec<f32>> = vec![Vec::with_capacity(trials); n_teams];
    let mut player_samples: Vec<Vec<f32>> = vec![Vec::with_capacity(trials); n_players];
    let mut team_wins: Vec<u32> = vec![0; n_teams];
    let mut team_top3: Vec<u32> = vec![0; n_teams];
    // head_to_head_wins[i][j] = count of trials where team i's total > team j's.
    // Strict inequality: ties contribute to neither side.
    let mut h2h_wins: Vec<Vec<u32>> = vec![vec![0; n_teams]; n_teams];

    // NHL-team accumulators. Index space is the set of abbrevs appearing in
    // `round1` input — every team entering the playoffs starts with a slot.
    let nhl_abbrevs: Vec<String> = {
        let mut seen = HashSet::<String>::new();
        let mut out = Vec::new();
        for s in &input.round1 {
            for abbrev in [&s.top_team, &s.bottom_team] {
                if seen.insert(abbrev.clone()) {
                    out.push(abbrev.clone());
                }
            }
        }
        out
    };
    let nhl_idx: HashMap<String, usize> = nhl_abbrevs
        .iter()
        .enumerate()
        .map(|(i, a)| (a.clone(), i))
        .collect();
    let n_nhl = nhl_abbrevs.len();
    let mut nhl_round1_wins = vec![0u32; n_nhl];
    let mut nhl_conf_finals = vec![0u32; n_nhl];
    let mut nhl_cup_finals = vec![0u32; n_nhl];
    let mut nhl_cup_wins = vec![0u32; n_nhl];
    let mut nhl_games_total = vec![0u64; n_nhl];

    let mut team_games_this_run: HashMap<String, u32> = HashMap::with_capacity(16);
    let mut team_totals: Vec<f32> = vec![0.0; n_teams];
    let mut ranking: Vec<(usize, f32)> = Vec::with_capacity(n_teams);

    let k = if input.k_factor > 0.0 {
        input.k_factor
    } else {
        DEFAULT_K_FACTOR
    };

    for _ in 0..trials {
        team_games_this_run.clear();
        for g in team_totals.iter_mut() {
            *g = 0.0;
        }
        for (team, games) in &input.games_played_so_far {
            team_games_this_run.insert(team.clone(), *games);
        }

        // --- Round 1: simulate remaining games in each active series. ---
        let mut round_winners: Vec<(String, String)> = Vec::with_capacity(input.round1.len());
        for series in &input.round1 {
            let top_rating = rating_for(&input.ratings, &series.top_team);
            let bot_rating = rating_for(&input.ratings, &series.bottom_team);
            let p_top_game = sigmoid(k * (top_rating - bot_rating));

            let outcome = simulate_series(
                series.top_wins,
                series.bottom_wins,
                p_top_game,
                rng,
            );
            let remaining = outcome
                .total_games()
                .saturating_sub(series.top_wins + series.bottom_wins);
            add_games(&mut team_games_this_run, &series.top_team, remaining);
            add_games(&mut team_games_this_run, &series.bottom_team, remaining);

            let winner = if outcome.top_wins >= 4 {
                series.top_team.clone()
            } else {
                series.bottom_team.clone()
            };
            // Winner advanced past round 1.
            if let Some(&i) = nhl_idx.get(&winner) {
                nhl_round1_wins[i] += 1;
            }
            round_winners.push((series.series_letter.clone(), winner));
        }

        // --- Rounds 2+: pair winners by bracket order, simulate from 0-0. ---
        // Track which round the survivors have most recently reached. After
        // round 1 they've reached "round 2" (conference semis). Each pass of
        // pair_and_simulate advances the survivors one round further.
        let mut round_reached = 2u32;
        while round_winners.len() > 1 {
            round_winners = pair_and_simulate(
                &round_winners,
                &input.ratings,
                k,
                &mut team_games_this_run,
                rng,
            );
            round_reached += 1;
            // round_reached now reflects the round the new `round_winners`
            // have advanced to: 3 = Conference Finals, 4 = Cup Final,
            // 5 = Cup winner.
            for (_, winner) in &round_winners {
                if let Some(&i) = nhl_idx.get(winner) {
                    match round_reached {
                        3 => nhl_conf_finals[i] += 1,
                        4 => nhl_cup_finals[i] += 1,
                        5 => nhl_cup_wins[i] += 1,
                        _ => {} // bracket is 16→1, shouldn't go past 5
                    }
                }
            }
        }

        // Accumulate games-played per NHL team for the expected_games stat.
        for (abbrev, games) in &team_games_this_run {
            if let Some(&i) = nhl_idx.get(abbrev) {
                nhl_games_total[i] += *games as u64;
            }
        }

        // --- Accumulate fantasy totals. ---
        for (pi, player) in players.iter().enumerate() {
            let team_games = *team_games_this_run.get(&player.nhl_team).unwrap_or(&0);
            let already = *input
                .games_played_so_far
                .get(&player.nhl_team)
                .unwrap_or(&0);
            let remaining_games = team_games.saturating_sub(already);
            let sim_pts = if remaining_games > 0 && player.ppg > 0.0 {
                sample_poisson(player.ppg * remaining_games as f32, rng)
            } else {
                0
            };
            let total = player.playoff_points_so_far as f32 + sim_pts as f32;
            player_samples[pi].push(total);
            team_totals[player_team_index[pi]] += total;
        }

        for (i, &t) in team_totals.iter().enumerate() {
            team_samples[i].push(t);
        }

        ranking.clear();
        for (i, &t) in team_totals.iter().enumerate() {
            ranking.push((i, t));
        }
        ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (rank, (team_idx, _)) in ranking.iter().enumerate() {
            if rank == 0 {
                team_wins[*team_idx] += 1;
            }
            if rank < 3 {
                team_top3[*team_idx] += 1;
            }
        }

        // Pairwise comparisons. Strict > so ties contribute to neither side;
        // the Rivalry card renders "P(finish ahead of rival)" which matches
        // the strict-inequality semantics.
        for i in 0..n_teams {
            for j in 0..n_teams {
                if i != j && team_totals[i] > team_totals[j] {
                    h2h_wins[i][j] += 1;
                }
            }
        }
    }

    let teams_out: Vec<TeamOdds> = input
        .fantasy_teams
        .iter()
        .enumerate()
        .map(|(i, team)| {
            let samples = &team_samples[i];
            let current: i32 = team
                .players
                .iter()
                .map(|p| p.playoff_points_so_far)
                .sum();
            let (mean, median, p10, p90) = summarise(samples);

            let mut head_to_head: HashMap<i64, f32> = HashMap::with_capacity(n_teams.saturating_sub(1));
            for (j, opponent) in input.fantasy_teams.iter().enumerate() {
                if i == j {
                    continue;
                }
                head_to_head.insert(opponent.team_id, h2h_wins[i][j] as f32 / trials as f32);
            }

            TeamOdds {
                team_id: team.team_id,
                team_name: team.team_name.clone(),
                current_points: current,
                projected_final_mean: mean,
                projected_final_median: median,
                p10,
                p90,
                win_prob: team_wins[i] as f32 / trials as f32,
                top3_prob: team_top3[i] as f32 / trials as f32,
                head_to_head,
            }
        })
        .collect();

    let players_out: Vec<PlayerOdds> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (mean, median, p10, p90) = summarise(&player_samples[i]);
            PlayerOdds {
                nhl_id: p.nhl_id,
                name: p.name.clone(),
                nhl_team: p.nhl_team.clone(),
                position: p.position.clone(),
                current_points: p.playoff_points_so_far,
                projected_final_mean: mean,
                projected_final_median: median,
                p10,
                p90,
                image_url: p.image_url.clone(),
            }
        })
        .collect();

    let trials_f = trials as f32;
    let nhl_teams_out: Vec<NhlTeamOdds> = nhl_abbrevs
        .iter()
        .enumerate()
        .map(|(i, abbrev)| NhlTeamOdds {
            abbrev: abbrev.clone(),
            advance_round1_prob: nhl_round1_wins[i] as f32 / trials_f,
            conference_finals_prob: nhl_conf_finals[i] as f32 / trials_f,
            cup_finals_prob: nhl_cup_finals[i] as f32 / trials_f,
            cup_win_prob: nhl_cup_wins[i] as f32 / trials_f,
            expected_games: nhl_games_total[i] as f32 / trials_f,
        })
        .collect();

    RaceSimOutput {
        trials,
        teams: teams_out,
        players: players_out,
        nhl_teams: nhl_teams_out,
    }
}

// ---------------------------------------------------------------------------
// Series + bracket simulation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct SeriesOutcome {
    top_wins: u32,
    bot_wins: u32,
}

impl SeriesOutcome {
    fn total_games(&self) -> u32 {
        self.top_wins + self.bot_wins
    }
}

/// Play out a best-of-7 game by game from the given state, with `p_top_game`
/// as the per-game probability the top seed wins.
///
/// We deliberately do *not* fold the hockeystats "second-by-second shot sim"
/// here — that would require xG/RAPM data we don't have. Per-game prob from
/// a team-strength logistic is the biggest cheap improvement over assuming
/// 50/50 regardless of matchup.
fn simulate_series(
    mut top_wins: u32,
    mut bot_wins: u32,
    p_top_game: f32,
    rng: &mut SmallRng,
) -> SeriesOutcome {
    // Clamp to [0.05, 0.95] so an extreme rating gap doesn't produce sweeps
    // every trial — real upsets happen.
    let p = p_top_game.clamp(0.05, 0.95);
    while top_wins < 4 && bot_wins < 4 {
        if rng.r#gen::<f32>() < p {
            top_wins += 1;
        } else {
            bot_wins += 1;
        }
    }
    SeriesOutcome { top_wins, bot_wins }
}

/// Advance winners into the next round. Pairing is positional: neighbouring
/// entries meet in the next round, which matches the NHL bracket order
/// (A,B,C,D,E,F,G,H → AB,CD,EF,GH → ABCD,EFGH → Cup).
fn pair_and_simulate(
    winners: &[(String, String)],
    ratings: &HashMap<String, TeamRating>,
    k: f32,
    team_games: &mut HashMap<String, u32>,
    rng: &mut SmallRng,
) -> Vec<(String, String)> {
    let mut next = Vec::with_capacity(winners.len() / 2);
    let mut i = 0;
    while i + 1 < winners.len() {
        let (lbl_a, team_a) = &winners[i];
        let (lbl_b, team_b) = &winners[i + 1];

        let r_a = rating_for(ratings, team_a);
        let r_b = rating_for(ratings, team_b);
        let p_a_game = sigmoid(k * (r_a - r_b));

        let outcome = simulate_series(0, 0, p_a_game, rng);
        let games = outcome.total_games();
        add_games(team_games, team_a, games);
        add_games(team_games, team_b, games);

        let winner = if outcome.top_wins >= 4 {
            team_a.clone()
        } else {
            team_b.clone()
        };
        next.push((format!("{}{}", lbl_a, lbl_b), winner));
        i += 2;
    }
    next
}

fn rating_for(ratings: &HashMap<String, TeamRating>, team: &str) -> f32 {
    ratings.get(team).copied().unwrap_or(TeamRating(0.0)).0
}

fn add_games(map: &mut HashMap<String, u32>, team: &str, games: u32) {
    *map.entry(team.to_string()).or_insert(0) += games;
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// ---------------------------------------------------------------------------
// Sampling helpers
// ---------------------------------------------------------------------------

/// Knuth's Poisson sampler. Fine for the small lambda values (≤ ~10) we hit
/// here; we don't pull in `rand_distr` just for this one draw.
fn sample_poisson(lambda: f32, rng: &mut SmallRng) -> i32 {
    if lambda <= 0.0 {
        return 0;
    }
    let l = (-lambda).exp();
    let mut k = 0i32;
    let mut p: f32 = 1.0;
    loop {
        k += 1;
        p *= rng.r#gen::<f32>();
        if p <= l {
            return k - 1;
        }
        if k > 50 {
            return k - 1;
        }
    }
}

fn summarise(samples: &[f32]) -> (f32, f32, f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut sorted: Vec<f32> = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = samples.iter().copied().sum::<f32>() / samples.len() as f32;
    (mean, percentile(&sorted, 0.50), percentile(&sorted, 0.10), percentile(&sorted, 0.90))
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f32 - 1.0) * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_player(name: &str, team: &str, ppg: f32, points: i32) -> SimPlayer {
        SimPlayer {
            nhl_id: name.len() as i64,
            name: name.to_string(),
            nhl_team: team.to_string(),
            position: "C".to_string(),
            playoff_points_so_far: points,
            ppg,
            image_url: None,
        }
    }

    fn baseline_input() -> RaceSimInput {
        let r1 = vec![
            ("A", "BOS", "BUF"),
            ("B", "TBL", "MTL"),
            ("C", "CAR", "OTT"),
            ("D", "PIT", "PHI"),
            ("E", "EDM", "ANA"),
            ("F", "COL", "LAK"),
            ("G", "DAL", "MIN"),
            ("H", "VGK", "UTA"),
        ]
        .into_iter()
        .map(|(l, t, b)| CurrentSeries {
            series_letter: l.into(),
            top_team: t.into(),
            top_wins: 0,
            bottom_team: b.into(),
            bottom_wins: 0,
        })
        .collect();

        RaceSimInput {
            round1: r1,
            games_played_so_far: HashMap::new(),
            ratings: HashMap::new(),
            k_factor: DEFAULT_K_FACTOR,
            fantasy_teams: vec![
                SimFantasyTeam {
                    team_id: 1,
                    team_name: "One".into(),
                    players: vec![
                        mk_player("A", "BOS", 0.6, 0),
                        mk_player("B", "COL", 0.6, 0),
                    ],
                },
                SimFantasyTeam {
                    team_id: 2,
                    team_name: "Two".into(),
                    players: vec![
                        mk_player("C", "EDM", 0.6, 0),
                        mk_player("D", "DAL", 0.6, 0),
                    ],
                },
            ],
        }
    }

    #[test]
    fn probabilities_are_valid_and_sum_to_one() {
        let out = simulate_with_seed(&baseline_input(), 2000, 42);
        assert_eq!(out.teams.len(), 2);
        let total_win: f32 = out.teams.iter().map(|t| t.win_prob).sum();
        assert!((total_win - 1.0).abs() < 1e-5, "win probs should sum to 1.0");
        for t in &out.teams {
            assert!(t.win_prob >= 0.0 && t.win_prob <= 1.0);
            assert!(t.projected_final_mean >= 0.0);
            assert!(t.p10 <= t.projected_final_median + 1e-3);
            assert!(t.projected_final_median <= t.p90 + 1e-3);
        }
    }

    #[test]
    fn teammate_correlation_widens_team_spread() {
        // Teammates on the same NHL club share games-played every trial, so
        // their joint variance should be >> 2× a single player's variance.
        let mut input = baseline_input();
        input.fantasy_teams = vec![SimFantasyTeam {
            team_id: 1,
            team_name: "BruinsStack".into(),
            players: vec![
                mk_player("A", "BOS", 1.0, 0),
                mk_player("B", "BOS", 1.0, 0),
            ],
        }];
        let out = simulate_with_seed(&input, 2000, 7);
        let team_spread = out.teams[0].p90 - out.teams[0].p10;
        let player_spread = out.players[0].p90 - out.players[0].p10;
        assert!(
            team_spread >= 1.4 * player_spread,
            "correlated team spread {} should exceed 1.4× player spread {}",
            team_spread,
            player_spread
        );
    }

    #[test]
    fn stronger_team_wins_more_often() {
        let mut input = baseline_input();
        // Make BOS dominant, MTL average; the other teams balance out.
        input.ratings.insert("BOS".into(), TeamRating(80.0));
        input.ratings.insert("BUF".into(), TeamRating(40.0));
        input.fantasy_teams = vec![
            SimFantasyTeam {
                team_id: 1,
                team_name: "Bruins".into(),
                players: vec![mk_player("A", "BOS", 1.0, 0)],
            },
            SimFantasyTeam {
                team_id: 2,
                team_name: "Sabres".into(),
                players: vec![mk_player("B", "BUF", 1.0, 0)],
            },
        ];
        let out = simulate_with_seed(&input, 2000, 11);
        let bos = out.teams.iter().find(|t| t.team_name == "Bruins").unwrap();
        let buf = out.teams.iter().find(|t| t.team_name == "Sabres").unwrap();
        assert!(
            bos.projected_final_mean > buf.projected_final_mean,
            "higher-rated team should project higher"
        );
        assert!(
            bos.win_prob > buf.win_prob,
            "higher-rated team should win the race more often"
        );
    }

    #[test]
    fn advancing_team_gets_more_games() {
        let mut input = baseline_input();
        // BOS up 3-0, already played 3 games. MTL in a separate tied series.
        input.round1[0].top_wins = 3;
        input.games_played_so_far.insert("BOS".into(), 3);
        input.games_played_so_far.insert("BUF".into(), 3);

        input.fantasy_teams = vec![
            SimFantasyTeam {
                team_id: 1,
                team_name: "Bruins".into(),
                players: vec![mk_player("A", "BOS", 1.0, 0)],
            },
            SimFantasyTeam {
                team_id: 2,
                team_name: "Habs".into(),
                players: vec![mk_player("B", "MTL", 1.0, 0)],
            },
        ];

        let out = simulate_with_seed(&input, 2000, 99);
        let bos = out.teams.iter().find(|t| t.team_name == "Bruins").unwrap();
        let mtl = out.teams.iter().find(|t| t.team_name == "Habs").unwrap();
        assert!(
            bos.projected_final_mean > mtl.projected_final_mean,
            "BOS (up 3-0) should project higher than MTL (tied 0-0 in losing series)"
        );
    }

    #[test]
    fn nhl_cup_odds_are_monotonic_and_self_consistent() {
        // Every NHL team in round1 must have non-negative probabilities, and
        // the round-reached bucket chain must be monotonically non-increasing:
        // P(win R1) ≥ P(conf finals) ≥ P(cup finals) ≥ P(win cup). Across all
        // teams the cup-win probabilities must sum to ~1.0.
        let out = simulate_with_seed(&baseline_input(), 2000, 13);
        assert_eq!(out.nhl_teams.len(), 16, "all round1 teams should have odds");
        let mut total_cup = 0.0f32;
        for team in &out.nhl_teams {
            assert!(team.advance_round1_prob >= team.conference_finals_prob - 1e-6);
            assert!(team.conference_finals_prob >= team.cup_finals_prob - 1e-6);
            assert!(team.cup_finals_prob >= team.cup_win_prob - 1e-6);
            assert!(team.expected_games >= 4.0, "every team plays at least 4");
            total_cup += team.cup_win_prob;
        }
        assert!(
            (total_cup - 1.0).abs() < 1e-5,
            "Cup-win probabilities must sum to 1.0, got {}",
            total_cup
        );
    }

    #[test]
    fn stronger_team_has_higher_cup_odds() {
        // Set BOS up as a heavyweight; the rating gap should show up in the
        // Cup-win probability, not just the head-to-head.
        let mut input = baseline_input();
        input.ratings.insert("BOS".into(), TeamRating(120.0));
        input.ratings.insert("BUF".into(), TeamRating(60.0));

        let out = simulate_with_seed(&input, 3000, 21);
        let bos = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "BOS")
            .expect("BOS in output");
        let buf = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "BUF")
            .expect("BUF in output");
        assert!(bos.cup_win_prob > buf.cup_win_prob * 2.0,
            "BOS with 60-point rating advantage should have 2× the Cup odds of BUF (got BOS={}, BUF={})",
            bos.cup_win_prob, buf.cup_win_prob);
    }

    #[test]
    fn poisson_mean_is_close_to_lambda() {
        let mut rng = SmallRng::seed_from_u64(42);
        let samples: Vec<i32> = (0..5000).map(|_| sample_poisson(2.0, &mut rng)).collect();
        let mean = samples.iter().sum::<i32>() as f32 / samples.len() as f32;
        assert!(
            (mean - 2.0).abs() < 0.15,
            "empirical mean {} should be within 0.15 of lambda=2.0",
            mean
        );
    }

    #[test]
    fn realised_points_count_even_when_eliminated() {
        let mut input = baseline_input();
        input.fantasy_teams = vec![SimFantasyTeam {
            team_id: 1,
            team_name: "Eliminated".into(),
            players: vec![mk_player("A", "BUF", 0.0, 12)], // no PPG, already scored 12
        }];
        let out = simulate_with_seed(&input, 500, 1);
        assert_eq!(out.teams[0].current_points, 12);
        assert!(
            out.teams[0].projected_final_mean >= 12.0 - 1e-5,
            "projected mean must not undercut realised points"
        );
    }
}
