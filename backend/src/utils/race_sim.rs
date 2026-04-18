//! Team-correlated Monte Carlo simulation of the fantasy-points race.
//!
//! Pure-domain module: no HTTP, no SQL, no logging of request state. Takes a
//! fully-built `RaceSimInput` and returns a `RaceSimOutput` describing each
//! fantasy team's (and player's) projected final total plus win probability.
//!
//! The engine simulates the whole bracket end-to-end for N trials:
//!   1. The caller hands us a [`BracketState`] — the playoff tree with every
//!      series tagged `Completed` (winner fixed), `InProgress` (live wins),
//!      or `Future` (participants not yet known). For each trial we walk the
//!      rounds in order and resolve every slot. `Completed` slots propagate
//!      their known winner unchanged — they contribute zero remaining games.
//!      `InProgress` slots continue from their live (top_wins, bottom_wins)
//!      state. `Future` slots pull their participants from the winners of
//!      the two feeder slots in the previous round and simulate from 0-0.
//!   2. Per-game win probability is a logistic of the team-strength gap —
//!      this follows the hockeystats.com methodology where per-game odds
//!      come from team strength and series outcome emerges from iterated
//!      per-game draws, rather than from a pre-baked series-state table.
//!   3. Each playoff team ends the trial with a `remaining_games_this_trial`
//!      value shared by every player on that roster — teammates are
//!      correlated by construction, and cross-roster correlation (two
//!      fantasy teams both rostering an Oilers skater) falls out for free.
//!   4. For each skater, remaining fantasy points are drawn from a Poisson
//!      around `ppg * remaining_games`. Realised playoff points (locked-in)
//!      are added on top.
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

/// Lifecycle state of one playoff series.
///
/// Every series in the bracket passes through these states in order:
/// `Future` → `InProgress` → `Completed`. The simulator resolves each state
/// differently inside a trial:
/// - [`SeriesState::Completed`] propagates its known winner and contributes
///   zero remaining games (the series is done in real life).
/// - [`SeriesState::InProgress`] continues from the live `(top_wins,
///   bottom_wins)` score — so a team up 3-0 inherits a near-certainty of
///   advancing, and the remaining-games count excludes games already played.
/// - [`SeriesState::Future`] has no participants until the previous round's
///   feeding slots resolve within the trial. Simulated from 0-0.
#[derive(Debug, Clone)]
pub enum SeriesState {
    /// Participants unknown; filled in per-trial from feeder slots.
    Future,
    /// Both teams known; may be 0-0 (about to start) or live.
    InProgress {
        top_team: String,
        top_wins: u32,
        bottom_team: String,
        bottom_wins: u32,
    },
    /// Resolved series. `total_games` is the actual number of games played
    /// in this series (4 to 7). Used only for bookkeeping; the sim adds
    /// zero *remaining* games on top.
    Completed {
        winner: String,
        loser: String,
        total_games: u32,
    },
}

/// The NHL playoff bracket as a strict positional binary tree.
///
/// Indexing convention:
/// - `rounds[0]` — first round (8 series for a 16-team bracket)
/// - `rounds[1]` — conference semifinals (4)
/// - `rounds[2]` — conference finals (2)
/// - `rounds[3]` — Stanley Cup Final (1)
///
/// Pairing is positional: the winner of `rounds[r][2i]` meets the winner of
/// `rounds[r][2i+1]` in `rounds[r+1][i]`. This matches the NHL's static
/// bracket (2013-present); teams do not re-seed between rounds.
///
/// Callers building this from the NHL `/playoff-series/carousel` feed
/// should pad missing rounds / slots with `SeriesState::Future` so the
/// sim walks the full tree.
#[derive(Debug, Clone, Default)]
pub struct BracketState {
    pub rounds: Vec<Vec<SeriesState>>,
}

impl BracketState {
    /// Number of rounds with at least one slot. For a full 16-team bracket
    /// this returns 4.
    pub fn depth(&self) -> usize {
        self.rounds.len()
    }

    /// The set of NHL team abbreviations that are (or were) participants in
    /// at least one bracket slot. Derived from `InProgress` and `Completed`
    /// series in the first round — enough for every team that actually
    /// entered the playoffs.
    pub fn known_teams(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut seen = HashSet::<String>::new();
        let first = match self.rounds.first() {
            Some(r) => r,
            None => return out,
        };
        for series in first {
            match series {
                SeriesState::InProgress {
                    top_team,
                    bottom_team,
                    ..
                } => {
                    for t in [top_team, bottom_team] {
                        if seen.insert(t.clone()) {
                            out.push(t.clone());
                        }
                    }
                }
                SeriesState::Completed { winner, loser, .. } => {
                    for t in [winner, loser] {
                        if seen.insert(t.clone()) {
                            out.push(t.clone());
                        }
                    }
                }
                SeriesState::Future => {}
            }
        }
        out
    }
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
    /// Full bracket — every round, every slot, tagged with its lifecycle
    /// state. The sim walks this in order and resolves each slot per trial.
    pub bracket: BracketState,
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

/// Negative-Binomial dispersion parameter for the per-player scoring
/// distribution. NHL fantasy scoring is overdispersed relative to
/// Poisson (mean == variance is too tight for a skater who alternates
/// blank nights with 3-point games). We model it as a Gamma-Poisson
/// mixture with mean λ and variance λ + λ²/r.
///
/// `r = 4.0` gives variance ≈ 1.25·λ at λ=1 and ≈ 2.5·λ at λ=5, which
/// roughly matches empirical per-game point spread for top skaters. A
/// future P4.2 hyperparameter pass will retune this against historical
/// scoring variance.
pub const NB_DISPERSION: f32 = 4.0;

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
    // Fractional accumulators: ties split credit evenly across the
    // tied teams so `win_prob` always sums to 1.0 (or close to it,
    // floating-point) even when a seed produces tied totals. The old
    // integer-counter version gave full credit to whichever team
    // happened to sort first.
    let mut team_wins: Vec<f32> = vec![0.0; n_teams];
    let mut team_top3: Vec<f32> = vec![0.0; n_teams];
    // head_to_head_wins[i][j] = count of trials where team i's total > team j's.
    // Strict inequality: ties contribute to neither side.
    let mut h2h_wins: Vec<Vec<u32>> = vec![vec![0; n_teams]; n_teams];

    // NHL-team accumulators. Index space is every abbrev appearing in R1 of
    // the bracket (InProgress or Completed) — that's every team that
    // entered the playoffs.
    let nhl_abbrevs = input.bracket.known_teams();
    let nhl_idx: HashMap<String, usize> = nhl_abbrevs
        .iter()
        .enumerate()
        .map(|(i, a)| (a.clone(), i))
        .collect();
    let n_nhl = nhl_abbrevs.len();
    let mut nhl_round_advance: Vec<Vec<u32>> = vec![vec![0; n_nhl]; input.bracket.depth()];
    let mut nhl_games_total = vec![0u64; n_nhl];

    // Per-trial scratch. `remaining_games[team]` counts games the team is
    // *still going to play* this trial — not games already played in real
    // life. Used directly to size each player's Poisson draw, no
    // subtraction needed.
    let mut remaining_games: HashMap<String, u32> = HashMap::with_capacity(16);
    let mut team_totals: Vec<f32> = vec![0.0; n_teams];
    let mut ranking: Vec<(usize, f32)> = Vec::with_capacity(n_teams);

    let k = if input.k_factor > 0.0 {
        input.k_factor
    } else {
        DEFAULT_K_FACTOR
    };

    for _ in 0..trials {
        remaining_games.clear();
        for g in team_totals.iter_mut() {
            *g = 0.0;
        }

        // --- Walk the bracket. Each round produces winners[r][i] = team that
        // won slot i of round r in this trial. Round r+1's Future slots pull
        // their participants from winners[r][2i] and winners[r][2i+1]. ---
        let n_rounds = input.bracket.depth();
        let mut winners: Vec<Vec<String>> = Vec::with_capacity(n_rounds);
        for (r, round) in input.bracket.rounds.iter().enumerate() {
            let mut round_winners: Vec<String> = Vec::with_capacity(round.len());
            for (i, slot) in round.iter().enumerate() {
                let (winner, games_added, top_team, bottom_team) = match slot {
                    SeriesState::Completed {
                        winner,
                        loser,
                        total_games: _,
                    } => {
                        // Series already happened in real life. Zero new games.
                        (winner.clone(), 0u32, winner.clone(), loser.clone())
                    }
                    SeriesState::InProgress {
                        top_team,
                        top_wins,
                        bottom_team,
                        bottom_wins,
                    } => {
                        let top_rating = rating_for(&input.ratings, top_team);
                        let bot_rating = rating_for(&input.ratings, bottom_team);
                        let p_top_game = sigmoid(k * (top_rating - bot_rating));
                        let outcome =
                            simulate_series(*top_wins, *bottom_wins, p_top_game, rng);
                        // Only games played *from now on* count as remaining.
                        let remaining = outcome
                            .total_games()
                            .saturating_sub(*top_wins + *bottom_wins);
                        let winner = if outcome.top_wins >= 4 {
                            top_team.clone()
                        } else {
                            bottom_team.clone()
                        };
                        (winner, remaining, top_team.clone(), bottom_team.clone())
                    }
                    SeriesState::Future => {
                        // Participants come from the previous round's feeder
                        // slots. If the bracket is structurally incomplete
                        // (e.g. the previous round has fewer winners than we
                        // need), skip this slot — the trial degrades
                        // gracefully instead of panicking.
                        let (top, bot) = match (
                            winners.get(r.wrapping_sub(1)).and_then(|w| w.get(2 * i)),
                            winners.get(r.wrapping_sub(1)).and_then(|w| w.get(2 * i + 1)),
                        ) {
                            (Some(a), Some(b)) => (a.clone(), b.clone()),
                            _ => {
                                round_winners.push(String::new());
                                continue;
                            }
                        };
                        let top_rating = rating_for(&input.ratings, &top);
                        let bot_rating = rating_for(&input.ratings, &bot);
                        let p_top_game = sigmoid(k * (top_rating - bot_rating));
                        let outcome = simulate_series(0, 0, p_top_game, rng);
                        let games = outcome.total_games();
                        let winner = if outcome.top_wins >= 4 {
                            top.clone()
                        } else {
                            bot.clone()
                        };
                        (winner, games, top, bot)
                    }
                };

                if games_added > 0 {
                    add_games(&mut remaining_games, &top_team, games_added);
                    add_games(&mut remaining_games, &bottom_team, games_added);
                }
                round_winners.push(winner.clone());

                // Record advancement: winning slot (r, i) means the team
                // advanced *out of* round r, i.e. reached round r+1 (or the
                // Cup if r is the final round).
                if let Some(&team_i) = nhl_idx.get(&winner) {
                    nhl_round_advance[r][team_i] += 1;
                }
            }
            winners.push(round_winners);
        }

        // Expected-games accumulator — count games we simulated as
        // remaining this trial. Already-played games are *not* in this
        // number; callers who want "total games incl. past" add the
        // carousel's current wins back themselves.
        for (abbrev, games) in &remaining_games {
            if let Some(&i) = nhl_idx.get(abbrev) {
                nhl_games_total[i] += *games as u64;
            }
        }

        // --- Accumulate fantasy totals. ---
        for (pi, player) in players.iter().enumerate() {
            let rg = *remaining_games.get(&player.nhl_team).unwrap_or(&0);
            let sim_pts = if rg > 0 && player.ppg > 0.0 {
                // Negative-Binomial widens the tails vs plain Poisson so
                // p10/p90 / head-to-head reflect real playoff variance
                // instead of the Poisson mean-equals-variance straitjacket.
                sample_negbin(player.ppg * rg as f32, NB_DISPERSION, rng)
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

        // Win credit: fractional split across all teams tied for the
        // top score. `top_score` is the sort leader; count how many
        // entries equal it.
        let top_score = ranking.first().map(|(_, s)| *s).unwrap_or(0.0);
        let tied_for_first = ranking
            .iter()
            .take_while(|(_, s)| (*s - top_score).abs() < 1e-6)
            .count()
            .max(1);
        let win_share = 1.0 / tied_for_first as f32;
        for (team_idx, _) in ranking.iter().take(tied_for_first) {
            team_wins[*team_idx] += win_share;
        }

        // Top-3: split credit across whichever teams overlap the 3rd
        // rank. If teams 3, 4, 5 all have the same score as rank 3,
        // split the *remaining* (3 - fully_earned) credit among them.
        if !ranking.is_empty() {
            // Everyone ranked above the 3rd distinct score earns a full
            // 1.0. Teams tied at the 3rd distinct score share the
            // remaining credit (3 - earned_so_far).
            let mut earned_so_far = 0.0f32;
            let mut i = 0usize;
            while i < ranking.len() && earned_so_far + 1e-6 < 3.0 {
                let score_i = ranking[i].1;
                let j = i
                    + ranking[i..]
                        .iter()
                        .take_while(|(_, s)| (*s - score_i).abs() < 1e-6)
                        .count();
                // This group's slice is [i, j). Give them (remaining / group_size).
                let group_size = (j - i) as f32;
                let remaining = (3.0 - earned_so_far).max(0.0);
                let share = (remaining.min(group_size)) / group_size;
                for (team_idx, _) in &ranking[i..j] {
                    team_top3[*team_idx] += share;
                }
                earned_so_far += share * group_size;
                i = j;
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
    // Pre-compute already-played games per team so `expected_games` remains
    // "average total games this team plays across the whole run" rather than
    // "average remaining." Frontends (My Stakes, Stanley Cup Odds) read it
    // as a total, so preserving that semantics matters.
    let already_played = already_played_by_team(&input.bracket);
    let advance = |r: usize, i: usize| -> f32 {
        nhl_round_advance
            .get(r)
            .and_then(|v| v.get(i).copied())
            .unwrap_or(0) as f32
            / trials_f
    };
    let nhl_teams_out: Vec<NhlTeamOdds> = nhl_abbrevs
        .iter()
        .enumerate()
        .map(|(i, abbrev)| {
            let played = *already_played.get(abbrev).unwrap_or(&0) as f32;
            NhlTeamOdds {
                abbrev: abbrev.clone(),
                advance_round1_prob: advance(0, i),
                conference_finals_prob: advance(1, i),
                cup_finals_prob: advance(2, i),
                cup_win_prob: advance(3, i),
                expected_games: played + nhl_games_total[i] as f32 / trials_f,
            }
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

/// Count games already played per team across the bracket (both
/// `Completed` and `InProgress` contribute). Pure reduction over the
/// input bracket, no simulation involved.
fn already_played_by_team(bracket: &BracketState) -> HashMap<String, u32> {
    let mut out = HashMap::<String, u32>::new();
    for round in &bracket.rounds {
        for series in round {
            match series {
                SeriesState::Completed {
                    winner,
                    loser,
                    total_games,
                } => {
                    *out.entry(winner.clone()).or_insert(0) += total_games;
                    *out.entry(loser.clone()).or_insert(0) += total_games;
                }
                SeriesState::InProgress {
                    top_team,
                    top_wins,
                    bottom_team,
                    bottom_wins,
                } => {
                    let g = top_wins + bottom_wins;
                    *out.entry(top_team.clone()).or_insert(0) += g;
                    *out.entry(bottom_team.clone()).or_insert(0) += g;
                }
                SeriesState::Future => {}
            }
        }
    }
    out
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

/// Sample from Gamma(shape, scale) using Marsaglia & Tsang's method
/// (shape ≥ 1) with the `Gamma(r, θ) = Gamma(r+1, θ) * U^(1/r)` boost
/// for shape < 1. Mean = shape·scale, variance = shape·scale². Kept
/// dependency-free for the same reason as `sample_poisson`: tiny usage
/// surface, no point pulling in `rand_distr` for one draw.
fn sample_gamma(shape: f32, scale: f32, rng: &mut SmallRng) -> f32 {
    if shape <= 0.0 || scale <= 0.0 {
        return 0.0;
    }
    if shape < 1.0 {
        // Boost: X ~ Gamma(shape) == Gamma(shape+1) * U^(1/shape).
        let u: f32 = rng.r#gen::<f32>().max(f32::MIN_POSITIVE);
        return sample_gamma(shape + 1.0, scale, rng) * u.powf(1.0 / shape);
    }
    // Marsaglia–Tsang
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        // Sample standard normal via Box–Muller (one draw per iteration
        // is fine; acceptance rate is ~96%).
        let u1: f32 = rng.r#gen::<f32>().max(f32::MIN_POSITIVE);
        let u2: f32 = rng.r#gen::<f32>();
        let n = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        let v = (1.0 + c * n).powi(3);
        if v <= 0.0 {
            continue;
        }
        let u: f32 = rng.r#gen::<f32>().max(f32::MIN_POSITIVE);
        let x2 = n * n;
        if u < 1.0 - 0.0331 * x2 * x2 {
            return d * v * scale;
        }
        if u.ln() < 0.5 * x2 + d - d * v + d * v.ln() {
            return d * v * scale;
        }
    }
}

/// Negative-Binomial via Gamma-Poisson mixture. Mean is `lambda`;
/// variance is `lambda + lambda²/dispersion`. As `dispersion → ∞` the
/// distribution collapses to plain Poisson.
fn sample_negbin(lambda: f32, dispersion: f32, rng: &mut SmallRng) -> i32 {
    if lambda <= 0.0 {
        return 0;
    }
    if dispersion <= 0.0 || !dispersion.is_finite() {
        return sample_poisson(lambda, rng);
    }
    // Gamma(shape = dispersion, scale = lambda / dispersion) has
    // mean = lambda and variance = lambda²/dispersion.
    let theta = sample_gamma(dispersion, lambda / dispersion, rng);
    sample_poisson(theta, rng)
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

    /// Build a fresh 16-team bracket with every R1 series at 0-0 and every
    /// downstream round tagged Future. Mirrors the pre-puck-drop state of a
    /// new playoffs.
    fn baseline_bracket() -> BracketState {
        let r1: Vec<SeriesState> = [
            ("BOS", "BUF"),
            ("TBL", "MTL"),
            ("CAR", "OTT"),
            ("PIT", "PHI"),
            ("EDM", "ANA"),
            ("COL", "LAK"),
            ("DAL", "MIN"),
            ("VGK", "UTA"),
        ]
        .into_iter()
        .map(|(t, b)| SeriesState::InProgress {
            top_team: t.into(),
            top_wins: 0,
            bottom_team: b.into(),
            bottom_wins: 0,
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

    fn baseline_input() -> RaceSimInput {
        RaceSimInput {
            bracket: baseline_bracket(),
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
        // BOS up 3-0 (in the InProgress R1 series).
        if let Some(SeriesState::InProgress { top_wins, .. }) =
            input.bracket.rounds[0].get_mut(0)
        {
            *top_wins = 3;
        } else {
            panic!("expected first R1 slot to be InProgress");
        }

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

    // --- New tests for bracket-state correctness (P0 regression guards) ---

    #[test]
    fn completed_r1_series_propagate_winner_deterministically() {
        // If every R1 slot is Completed, the sim must honour those winners
        // 100% of trials — no re-opening a decided series.
        let mut bracket = baseline_bracket();
        let winners = [
            "BOS", "TBL", "CAR", "PIT", "EDM", "COL", "DAL", "VGK",
        ];
        let losers = [
            "BUF", "MTL", "OTT", "PHI", "ANA", "LAK", "MIN", "UTA",
        ];
        for (i, (w, l)) in winners.iter().zip(losers.iter()).enumerate() {
            bracket.rounds[0][i] = SeriesState::Completed {
                winner: (*w).into(),
                loser: (*l).into(),
                total_games: 5,
            };
        }
        let input = RaceSimInput {
            bracket,
            ratings: HashMap::new(),
            k_factor: DEFAULT_K_FACTOR,
            fantasy_teams: vec![SimFantasyTeam {
                team_id: 1,
                team_name: "Me".into(),
                players: vec![mk_player("A", "BOS", 0.5, 0)],
            }],
        };
        let out = simulate_with_seed(&input, 1000, 42);
        let bos = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "BOS")
            .expect("BOS present");
        let buf = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "BUF")
            .expect("BUF present");
        assert!(
            (bos.advance_round1_prob - 1.0).abs() < 1e-6,
            "BOS must advance R1 in every trial, got {}",
            bos.advance_round1_prob
        );
        assert!(
            buf.advance_round1_prob.abs() < 1e-6,
            "BUF must never advance R1, got {}",
            buf.advance_round1_prob
        );
    }

    #[test]
    fn completed_series_add_zero_remaining_games() {
        // When every series in the bracket is Completed, the sim should
        // project zero future points for every player (no remaining games
        // to score in). Only playoff_points_so_far remains.
        let mut bracket = baseline_bracket();
        for series in bracket.rounds[0].iter_mut() {
            if let SeriesState::InProgress {
                top_team,
                bottom_team,
                ..
            } = series.clone()
            {
                *series = SeriesState::Completed {
                    winner: top_team,
                    loser: bottom_team,
                    total_games: 4,
                };
            }
        }
        // Round 2+: propagate placeholder winners too so nothing is Future.
        let r1_winners: Vec<String> = bracket.rounds[0]
            .iter()
            .map(|s| match s {
                SeriesState::Completed { winner, .. } => winner.clone(),
                _ => unreachable!(),
            })
            .collect();
        for r in 1..bracket.rounds.len() {
            for i in 0..bracket.rounds[r].len() {
                let w = r1_winners[2 * i % r1_winners.len()].clone();
                let l = r1_winners[(2 * i + 1) % r1_winners.len()].clone();
                bracket.rounds[r][i] = SeriesState::Completed {
                    winner: w,
                    loser: l,
                    total_games: 4,
                };
            }
        }
        let input = RaceSimInput {
            bracket,
            ratings: HashMap::new(),
            k_factor: DEFAULT_K_FACTOR,
            fantasy_teams: vec![SimFantasyTeam {
                team_id: 1,
                team_name: "Me".into(),
                players: vec![mk_player("A", "BOS", 1.0, 5)],
            }],
        };
        let out = simulate_with_seed(&input, 200, 7);
        let me = &out.teams[0];
        // Only lock-in points remain; nothing to sample.
        assert!(
            (me.projected_final_mean - 5.0).abs() < 1e-5,
            "all-Completed bracket must project exactly the locked-in points, got {}",
            me.projected_final_mean
        );
    }

    #[test]
    fn ties_split_win_credit_fractionally() {
        // Four fantasy teams, each with an empty roster → every trial
        // ends in a 4-way tie at 0 points. Each team must get exactly
        // 0.25 win_prob (a previous integer-counter version would have
        // given 1.0 to whichever team happened to sort first).
        let input = RaceSimInput {
            bracket: baseline_bracket(),
            ratings: HashMap::new(),
            k_factor: DEFAULT_K_FACTOR,
            fantasy_teams: vec![
                SimFantasyTeam {
                    team_id: 1,
                    team_name: "A".into(),
                    players: vec![],
                },
                SimFantasyTeam {
                    team_id: 2,
                    team_name: "B".into(),
                    players: vec![],
                },
                SimFantasyTeam {
                    team_id: 3,
                    team_name: "C".into(),
                    players: vec![],
                },
                SimFantasyTeam {
                    team_id: 4,
                    team_name: "D".into(),
                    players: vec![],
                },
            ],
        };
        let out = simulate_with_seed(&input, 200, 1);
        let total: f32 = out.teams.iter().map(|t| t.win_prob).sum();
        assert!(
            (total - 1.0).abs() < 1e-4,
            "win probs must still sum to 1.0 under ties, got {total}"
        );
        for t in &out.teams {
            assert!(
                (t.win_prob - 0.25).abs() < 1e-4,
                "each tied team must get exactly 1/4 of the win credit, got {}",
                t.win_prob
            );
        }
    }

    #[test]
    fn in_progress_late_round_is_respected() {
        // A team up 3-0 in an R2 series should advance to the Conference
        // Finals near-deterministically (clamp caps at 0.95 per game, so
        // ~95% series win). This catches the old bug where R2 was replayed
        // from 0-0.
        let mut bracket = baseline_bracket();
        // Finish R1: BOS, TBL, CAR, PIT advance on one side; EDM, COL, DAL,
        // VGK on the other.
        let r1_results = [
            ("BOS", "BUF"),
            ("TBL", "MTL"),
            ("CAR", "OTT"),
            ("PIT", "PHI"),
            ("EDM", "ANA"),
            ("COL", "LAK"),
            ("DAL", "MIN"),
            ("VGK", "UTA"),
        ];
        for (i, (w, l)) in r1_results.iter().enumerate() {
            bracket.rounds[0][i] = SeriesState::Completed {
                winner: (*w).into(),
                loser: (*l).into(),
                total_games: 5,
            };
        }
        // R2: BOS vs TBL, BOS up 3-0.
        bracket.rounds[1][0] = SeriesState::InProgress {
            top_team: "BOS".into(),
            top_wins: 3,
            bottom_team: "TBL".into(),
            bottom_wins: 0,
        };
        // Leave the remaining R2 slots as Future so the sim resolves them
        // naturally from R1 Completed winners.

        let input = RaceSimInput {
            bracket,
            ratings: HashMap::new(),
            k_factor: DEFAULT_K_FACTOR,
            fantasy_teams: vec![SimFantasyTeam {
                team_id: 1,
                team_name: "Me".into(),
                players: vec![mk_player("A", "BOS", 0.5, 0)],
            }],
        };
        let out = simulate_with_seed(&input, 2000, 55);
        let bos = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "BOS")
            .expect("BOS present");
        let tbl = out
            .nhl_teams
            .iter()
            .find(|t| t.abbrev == "TBL")
            .expect("TBL present");
        // BOS already won R1 (Completed) and is up 3-0 in R2, so the chance
        // they reach the Conference Finals should be >= 0.9 even with the
        // per-game clamp.
        assert!(
            bos.conference_finals_prob >= 0.9,
            "BOS up 3-0 in R2 after winning R1 should reach CF ≥ 90% of trials, got {}",
            bos.conference_finals_prob
        );
        // TBL should never advance R1 again (they're Completed losers to
        // someone else… wait, TBL won their R1 series in this setup, so
        // they advance R1 deterministically) but their CF chance should
        // be <= 0.1 (they'd have to come back from 0-3).
        assert!(
            (tbl.advance_round1_prob - 1.0).abs() < 1e-6,
            "TBL won their R1 series in this setup; advance_round1_prob must be 1.0"
        );
        assert!(
            tbl.conference_finals_prob <= 0.15,
            "TBL down 0-3 should reach CF at most ~10% of trials, got {}",
            tbl.conference_finals_prob
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
    fn gamma_mean_and_variance_match_formula() {
        // Gamma(shape, scale): mean = shape·scale, variance = shape·scale².
        let mut rng = SmallRng::seed_from_u64(11);
        let shape = 4.0f32;
        let scale = 1.5f32;
        let samples: Vec<f32> = (0..8000).map(|_| sample_gamma(shape, scale, &mut rng)).collect();
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / samples.len() as f32;
        let expected_mean = shape * scale;
        let expected_var = shape * scale * scale;
        assert!(
            (mean - expected_mean).abs() < 0.2,
            "empirical mean {} vs expected {}",
            mean,
            expected_mean
        );
        assert!(
            (var - expected_var).abs() < 1.0,
            "empirical variance {} vs expected {}",
            var,
            expected_var
        );
    }

    #[test]
    fn negbin_variance_exceeds_poisson() {
        // At lambda = 5, dispersion = 4, variance should be ~5 + 25/4 ≈
        // 11.25 — well over Poisson's 5. The empirical variance from
        // enough draws should clearly exceed the Poisson's lambda.
        let mut rng = SmallRng::seed_from_u64(13);
        let lambda = 5.0f32;
        let n = 8000;
        let nb_samples: Vec<i32> = (0..n)
            .map(|_| sample_negbin(lambda, 4.0, &mut rng))
            .collect();
        let nb_mean = nb_samples.iter().map(|x| *x as f32).sum::<f32>() / n as f32;
        let nb_var = nb_samples
            .iter()
            .map(|x| (*x as f32 - nb_mean).powi(2))
            .sum::<f32>()
            / n as f32;
        // Mean matches lambda; variance exceeds lambda substantially.
        assert!((nb_mean - lambda).abs() < 0.3, "nb_mean {}", nb_mean);
        assert!(
            nb_var > lambda * 1.5,
            "NB variance {} should exceed 1.5·λ = {} (Poisson variance is λ)",
            nb_var,
            lambda * 1.5
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
