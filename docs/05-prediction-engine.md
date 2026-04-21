# Prediction engine

The backend ships a full forecasting stack that turns "who has which NHL player on their fantasy roster" into "what are the odds team X wins my league's playoff race, and who wins the Stanley Cup". This document explains every piece, with formulas and motivating intuition for readers who have not worked with Elo ratings or Monte Carlo simulation before.

Code layout under [`backend/src/domain/prediction/`](../backend/src/domain/prediction/):

```
team_ratings.rs        standings-based team strength
playoff_elo.rs         dynamic Elo that updates after every playoff game
goalie_rating.rs       starter-quality Elo bonus
player_projection.rs   Bayesian per-skater PPG blend
race_sim.rs            Monte Carlo simulator that ties it all together
series_projection.rs   empirical lookup (used for UI, not sim)
backtest.rs            replay helpers
```

Database-backed adapters live under [`backend/src/infra/prediction/`](../backend/src/infra/prediction/) (Elo replay loop, Claude narrator). The calibration harness lives at [`backend/src/infra/calibrate.rs`](../backend/src/infra/calibrate.rs).

## The forecast problem

Fantasy teams draft NHL skaters. Fantasy points = goals + assists scored by those skaters (see [`06-business-logic.md`](./06-business-logic.md)). In a playoff league, the question a user cares about is:

> What is the probability my team finishes first across the whole playoff run?

The naive answer is "rank teams by current points, declare the leader the winner". That is wrong because different rosters have different amounts of playoff hockey remaining:

- A team that rostered the 1-seed Oilers has up to 28 games of hockey ahead of them if the team reaches the Cup final.
- A team that rostered the 8-seed Jets has up to 28 too, but Jets probably exit in round 1.
- A team full of wildcard-round Senators skaters may have 0 remaining games if Ottawa gets swept.

So the forecast is really: for every roster, sample how many games each of their NHL skaters will still play, then sample how many points each skater scores in those games, then sum per team, then rank, then count how often each fantasy team finishes first across many trials.

That is a Monte Carlo. To run it you need three inputs:

1. **Team strength** - used to simulate playoff series, which tells you how many games each NHL team plays.
2. **Player projections** - expected fantasy points per game for each rostered skater.
3. **The current bracket state** - which series are in progress, which are complete, which are future.

The simulator consumes those three inputs and returns probabilities.

```
 ┌──────────────────────┐   ┌──────────────────────┐   ┌──────────────────────┐
 │  team ratings        │   │  player projections  │   │  bracket state       │
 │  (team_ratings.rs +  │   │  (player_projection  │   │  (nhl_playoff_bracket│
 │   playoff_elo.rs +   │   │   .rs, Bayesian      │   │   JSONB from meta    │
 │   goalie_rating.rs)  │   │   blend)             │   │   poller)            │
 └──────────┬───────────┘   └──────────┬───────────┘   └──────────┬───────────┘
            │                          │                          │
            ▼                          ▼                          ▼
                          ┌─────────────────────────────┐
                          │    race_sim.rs              │
                          │    Monte Carlo, 5000 trials │
                          └─────────────┬───────────────┘
                                        ▼
                          ┌─────────────────────────────┐
                          │   Team odds, player odds,   │
                          │   NHL team advance probs    │
                          │   → /api/race-odds          │
                          │   → /api/pulse              │
                          │   → /api/insights           │
                          └─────────────────────────────┘
```

## 1. Team ratings - blended standings

File: [`backend/src/domain/prediction/team_ratings.rs`](../backend/src/domain/prediction/team_ratings.rs).

The simplest team-strength signal is how many points a team earned in the regular season. The file adds one more signal - recent form - and mixes them:

```
rating = 0.7 × season_points  +  0.3 × (L10_points_per_game × 82)
```

Where `L10_points_per_game` is computed from the last-ten window in the standings feed: `(l10_wins × 2 + l10_ot_losses) / (l10_games × 2)`. The multiplication by 82 rescales the 10-game rate into the same units as `season_points` so the weighted blend is comparable.

Constants ([`team_ratings.rs:21-23`](../backend/src/domain/prediction/team_ratings.rs)):

| Constant | Value | Meaning |
| --- | --- | --- |
| `SEASON_WEIGHT` | 0.7 | How much to trust the 82-game record |
| `RECENT_WEIGHT` | 0.3 | How much to trust the 10-game rolling window |

Why 70/30 and not 50/50? An 82-game sample dominates a 10-game sample in Bayesian terms. The 30 % weight on L10 is a small nudge that catches hot-and-cold late-season teams without letting a good week overpower a mediocre season.

Behaviour during the playoffs: once the regular season ends the standings feed freezes L10 at its final value. The blend does not refresh - but the playoff Elo (next section) takes over as the real-time strength signal during the playoffs. The two models coexist; the team-ratings blend is shown in UI and used as a seeding prior, while the playoff Elo drives the Monte Carlo.

## 2. Playoff Elo - dynamic team strength

File: [`backend/src/domain/prediction/playoff_elo.rs`](../backend/src/domain/prediction/playoff_elo.rs).

### What Elo is

Elo is a rating system where every team has a numerical score. A 100-point rating gap implies the higher-rated team wins about 64% of the time. After each game, ratings shift: the winner gains points and the loser loses them, scaled by how surprising the outcome was. If the rating gap is already 200 points and the favorite wins, almost no points change hands because that was expected. If the underdog wins, the underdog gains a lot and the favorite loses a lot.

The canonical Elo win-probability formula:

```
p_home = 1 / (1 + 10^( -(elo_home - elo_away + home_ice) / 400 ))
```

The 400 in the denominator is the historical chess-Elo convention: a 400-point gap means a ten-to-one favorite. We keep it for compatibility with public NHL Elo trackers.

### Seeding

Before round 1, every team's rating comes from regular-season standings ([`playoff_elo.rs:140-169`](../backend/src/domain/prediction/playoff_elo.rs)):

```
avg_points     = mean of season_points across all teams
elo_0(team)    = 1500 + points_scale × shrinkage × (season_points - avg_points)
```

Production values:

| Constant | Value | Line |
| --- | --- | --- |
| `BASE_ELO` | 1500.0 | `playoff_elo.rs:26` |
| `POINTS_SCALE` | 6.0 | `playoff_elo.rs:30` |
| `HOME_ICE_ADV` | 35.0 | `playoff_elo.rs:33` |
| `K_FACTOR` | 6.0 | `playoff_elo.rs:36` |
| `PRODUCTION_SHRINKAGE` | 0.7 | `playoff_elo.rs:50` |
| `HOME_BONUS_DELTA_CLAMP` | 15.0 | `playoff_elo.rs:58` |

Worked example. Three teams: A at 100 points, B at 110, C at 90.

```
avg = 100
effective_scale = 6.0 × 0.7 = 4.2 Elo points per RS point
elo_A = 1500 + 4.2 × (100 - 100) = 1500
elo_B = 1500 + 4.2 × (110 - 100) = 1542
elo_C = 1500 + 4.2 × (90  - 100) = 1458
```

Why the 0.7 shrinkage? A raw 82-game sample contains noise. Treating each team's points differential as 70 % signal and 30 % noise is a standard Bayesian prior - it pulls every team toward 1500 slightly, which empirically improves calibration across backfilled playoff seasons (see Calibration, section 8).

### Per-game update

For each completed playoff game, feed it through `apply_game` ([`playoff_elo.rs:185-199`](../backend/src/domain/prediction/playoff_elo.rs)):

```
diff       = elo_home - elo_away + HOME_ICE_ADV
p_home     = 1 / (1 + 10^(-diff / 400))
home_won   = 1.0 if home_score > away_score else 0.0
goal_diff  = |home_score - away_score|
margin     = ln(goal_diff + 1)
delta      = K_FACTOR × margin × (home_won - p_home)
elo_home  += delta
elo_away  -= delta
```

Three pieces worth calling out:

- `+ HOME_ICE_ADV` in the `diff` - home teams are expected to win more often at equal ratings, so they must perform above that elevated baseline to gain rating points.
- `ln(goal_diff + 1)` - a 6-1 blowout moves ratings more than a 2-1 squeaker, with diminishing returns. A 1-goal win gives `ln(2) ≈ 0.69`; a 5-goal win gives `ln(6) ≈ 1.79`.
- The update is zero-sum within a game: whatever the home team gains, the away team loses.

### Per-team home-ice bonus

The league-wide 35-Elo home-ice bonus is an average. Some teams have noticeably stronger home records than others (altitude in Denver, barn atmosphere in Winnipeg) - [`home_bonus_from_standings`](../backend/src/domain/prediction/playoff_elo.rs#L78) reads each team's home and road points percentages from the regular season standings and produces a per-team bonus ([`playoff_elo.rs:78-113`](../backend/src/domain/prediction/playoff_elo.rs)):

```
home_pts_pct  = (home_wins × 2 + home_ot_losses) / (home_games × 2)
road_pts_pct  = same shape for road
raw_elo       = (home_pts_pct - road_pts_pct) × 400
delta         = raw_elo - HOME_ICE_ADV
delta_clamped = delta.clamp(-15, +15)
final_bonus   = HOME_ICE_ADV + delta_clamped    // in [20, 50]
```

The linear 400 coefficient is chosen so that a league-average home/road split produces `raw_elo ≈ HOME_ICE_ADV` (neutral delta). Teams much stronger or weaker at home clamp into the `[20, 50]` band.

### How the DB loads into the model

The handler path ([`infra/prediction/elo.rs`](../backend/src/infra/prediction/)) reads `playoff_game_results` in chronological order and folds each game through `apply_game`, starting from `seed_from_standings`. The resulting `HashMap<team_abbrev, elo>` is handed to the race simulator.

## 3. Goalie rating bonus

File: [`backend/src/domain/prediction/goalie_rating.rs`](../backend/src/domain/prediction/goalie_rating.rs).

Standings data does not capture starter quality. A team with an elite starter and an average roster can punch well above its rating; MoneyPuck weighs goaltending at about 29% of team strength. This module produces an Elo bonus per team based on the primary goalie's season save percentage.

Constants ([`goalie_rating.rs:29-47`](../backend/src/domain/prediction/goalie_rating.rs)):

| Constant | Value | Purpose |
| --- | --- | --- |
| `LEAGUE_AVG_SVP` | 0.905 | League-average save percentage |
| `GOALIE_BONUS_SCALE` | 800.0 | Elo delta per percentage point of SV% above/below average |
| `GOALIE_BONUS_CLAMP` | 30.0 | Max ±30 Elo swing from the goalie component |
| `MIN_WINS_FOR_STARTER` | 3.0 | Minimum wins for a goalie to count as a starter |

Formula ([`goalie_rating.rs:125-128`](../backend/src/domain/prediction/goalie_rating.rs)):

```
bonus_elo = ((sv_pct - 0.905) × 800).clamp(-30, 30)
```

Examples:

- `.925` SV% → `(0.020 × 800) = +16` Elo.
- `.940` SV% → `(0.035 × 800) = +28` Elo.
- `.895` SV% → `(-0.010 × 800) = -8` Elo.

### Tandem logic

Teams with two goalies close in playing time split the credit. `compute_bonuses` ([`goalie_rating.rs:75-121`](../backend/src/domain/prediction/goalie_rating.rs)):

1. Drop goalies with fewer than 3 wins or no `save_pct` (backups, callups).
2. Per team, sort by wins descending (tiebreak: higher SV% first).
3. Primary is the top entry. Any secondary with `primary_wins - wins ≤ 3` is in the tandem.
4. The team's bonus is the average of each tandem member's individual bonus.
5. Teams without a credible starter get no entry; callers default to zero bonus.

## 4. Player projection - Bayesian blend

File: [`backend/src/domain/prediction/player_projection.rs`](../backend/src/domain/prediction/player_projection.rs).

For each rostered skater, the Monte Carlo needs one number: expected fantasy points per remaining game. That number is a weighted blend of four signals:

| Signal | What it represents | Weight |
| --- | --- | --- |
| Regular-season PPG | Stable talent floor | `ALPHA = 10.0` games-equivalent |
| This-playoff rate | Current-run form, shot-stabilised | `po_gp` (number of playoff games played) |
| Recency-weighted rate | Last few games count more | Mixed with this-playoff rate at 65/35 |
| 5-year historical playoff PPG | Regression-to-mean anchor | `BETA = 4.0` games-equivalent |

The word "games-equivalent" captures the Bayesian structure: `ALPHA = 10` means the prior carries the weight of 10 observed games. If a player has 2 playoff games this year, the prior still dominates. After 20 playoff games, the current-run signal starts to dominate.

### Constants

All in [`player_projection.rs:44-87`](../backend/src/domain/prediction/player_projection.rs):

```
ALPHA                    = 10.0    // RS-prior strength
BETA                     = 4.0     // 5-year-history prior strength
PO_TO_RECENT_WEIGHT      = 0.65    // Mix raw playoff PPG vs recency-weighted
RECENT_HALF_LIFE_GAMES   = 4.0     // Exponential decay half-life
RECENT_WINDOW            = 10      // Look at most recent 10 games
ABSENT_MULTIPLIER        = 0.3     // Player absent from all playoff games
MIN_APPEARANCE_TEAM_GAMES = 3      // Threshold for "likely scratch" flag
LEAGUE_SH_PCT            = 0.095   // NHL-average shooting percentage
SHOT_STABILIZATION_WEIGHT = 0.40   // How much to regress toward shot-implied rate
TOI_RATIO_RECENT_WINDOW  = 3       // Recent TOI sample
TOI_RATIO_BASELINE_MIN   = 3       // Older TOI sample
TOI_RATIO_DERATE_FLOOR   = 0.70    // Max derate from lineup demotion
TOI_RATIO_BOOST_CAP      = 1.10    // Max boost from lineup promotion
```

### Formula

`project_one` ([`player_projection.rs:139-187`](../backend/src/domain/prediction/player_projection.rs)):

```
rs_ppg         = rs_points / 82.0

po_gp          = game_log.len()
po_rate        = stabilized_po_rate(game_log)       // goals stabilized against shot volume
recent_rate    = recency_weighted_rate(game_log)    // last 10 games, half-life 4

blended_po_rate = 0.65 × po_rate + 0.35 × recent_rate

hist_gp, hist_points from historical table (optional)
historical_ppg  = hist_points / hist_gp if hist_gp > 0 else 0
beta_weight     = BETA if hist_gp > 0 else 0

numerator   = ALPHA × rs_ppg + po_gp × blended_po_rate + beta_weight × historical_ppg
denominator = ALPHA + po_gp + beta_weight
base_ppg    = numerator / denominator

toi_mult    = toi_ratio_multiplier(game_log)
active_prob = ABSENT_MULTIPLIER if (team_games >= 3 and po_gp == 0) else 1.0

final_ppg   = base_ppg × toi_mult × active_prob
```

### Goal-rate stabilisation

`stabilized_po_rate` ([`player_projection.rs:201-229`](../backend/src/domain/prediction/player_projection.rs)): a player generating 4 shots per game with zero goals after 1 playoff game should not project at 0 goals per game. The stabilisation regresses observed goals toward what the player's shot volume would predict at league-average shooting percentage:

```
obs_goals_rate = Σ goals / gp
assists_rate   = Σ assists / gp

// If any game in the log has a non-null shot count:
shots_rate            = Σ shots (where non-null) / (games with non-null shots)
expected_goals_rate   = shots_rate × 0.095
stable_goals_rate     = 0.60 × obs_goals_rate + 0.40 × expected_goals_rate

po_rate = stable_goals_rate + assists_rate
```

Assists stay raw because most goals have an assist credited, so observed assist rate is close to true rate even over small samples.

### Recency weighting

`recency_weighted_rate` ([`player_projection.rs:235-252`](../backend/src/domain/prediction/player_projection.rs)). Game log is ordered newest-first. Weights follow a half-life of four games:

```
for i in 0..min(n, 10):
    w_i = 2^(-i / 4)
    num += w_i × points_i
    den += w_i

recent_rate = num / den
```

Game i=0 gets weight 1.0; game i=4 (half-life) gets 0.5; game i=10 gets about 0.18. A player who has been hot in the last three games but quiet before them gets a bump; a player who was hot early but has cooled off gets penalised.

### TOI ratio multiplier

`toi_ratio_multiplier` ([`player_projection.rs:262-282`](../backend/src/domain/prediction/player_projection.rs)): detects lineup demotions and promotions by comparing recent ice time to earlier playoff ice time:

```
if recent has ≥ 3 games with TOI data AND older has ≥ 3 games with TOI data:
    recent_avg = mean of toi[0..3]
    older_avg  = mean of toi[3..]
    mult       = (recent_avg / older_avg).clamp(0.70, 1.10)
else:
    mult = 1.0
```

A player demoted from first-line minutes to fourth-line minutes can drop to about 0.7 multiplier. A promotion caps at 1.1 because a single high-TOI overtime game can fake-inflate the signal.

### Worked example

Ryan McDonagh, regular season 40 points, 5 playoff games played with game log `[2p, 1p, 0p, 1p, 0p]` most-recent-first. Team has played 5 games. Historical 5-year playoff: 100 gp, 60 points. No shot data in the log.

```
rs_ppg          = 40 / 82 = 0.488
po_gp           = 5
obs_goals_rate  = (say 2 goals total) / 5 = 0.4
obs_assists     = (say 2 assists total) / 5 = 0.4
po_rate         = 0.4 + 0.4 = 0.8            (no shot data → raw)
recent_rate     = weighted avg of [2, 1, 0, 1, 0] with weights [1.0, 0.841, 0.707, 0.595, 0.5]
                ≈ (2 + 0.841 + 0 + 0.595 + 0) / (1.0 + 0.841 + 0.707 + 0.595 + 0.5)
                ≈ 3.436 / 3.643 ≈ 0.943
blended_po_rate = 0.65 × 0.8 + 0.35 × 0.943 ≈ 0.850

historical_ppg  = 60 / 100 = 0.6
beta_weight     = 4.0

numerator   = 10 × 0.488 + 5 × 0.850 + 4 × 0.6 = 4.88 + 4.25 + 2.4 = 11.53
denominator = 10 + 5 + 4 = 19
base_ppg    = 11.53 / 19 ≈ 0.607

toi_mult    = 1.0  (no lineup change detected)
active_prob = 1.0  (player is active)
final_ppg   ≈ 0.607
```

That is the number the Monte Carlo sampler uses.

## 5. Race simulator - the Monte Carlo

File: [`backend/src/domain/prediction/race_sim.rs`](../backend/src/domain/prediction/race_sim.rs).

This is the engine that puts everything together. It takes:

- A `BracketState` - the playoff tree, with each series tagged `Completed`, `InProgress`, or `Future`.
- A `HashMap<team_abbrev, TeamRating>` - base Elo + home-ice bonus + goalie bonus per team.
- A `k_factor: f32` - the slope of the logistic.
- A `home_ice_bonus: f32` - the pre-sigmoid league-average home advantage.
- A `Vec<SimFantasyTeam>` - rosters with per-player PPG projections and points already scored.

And returns for each fantasy team: projected final total (mean, median, p10, p90), win probability, top-3 probability, head-to-head probabilities. Plus per-NHL-team: round-advance probabilities and expected games played.

### Constants

From [`race_sim.rs:246-277`](../backend/src/domain/prediction/race_sim.rs):

| Constant | Value | Purpose |
| --- | --- | --- |
| `HOME_ICE_ELO` | 35.0 | Same 35 as `playoff_elo::HOME_ICE_ADV` |
| `DEFAULT_TRIALS` | 5000 | Number of Monte Carlo iterations |
| `DEFAULT_K_FACTOR` | 0.010 | Pre-sigmoid logistic slope; calibrated against HockeyStats round-1 probabilities |
| `DEFAULT_PPG` | 0.45 | Fallback projection when a skater's rate is unknown |
| `NB_DISPERSION` | 4.0 | Negative-Binomial `r` parameter for per-player point draws |

### Algorithm

For each of N trials (default 5000):

1. **Walk the bracket in round order.** For each series, resolve it based on its state:
   - `Completed`: propagate the known winner unchanged; contributes zero remaining games.
   - `InProgress { top_team, top_wins, bottom_team, bottom_wins }`: continue from the current `(top_wins, bottom_wins)`. Simulate game-by-game until one side reaches four wins.
   - `Future`: pull participants from the two feeder slots in the previous round's `winners[r-1]`. Simulate from 0-0.

2. **Per-game probability** for a single playoff game:
   ```
   rating_gap = (elo_top - elo_bottom) + (goalie_top - goalie_bottom)
   gap        = k × rating_gap
   // For Future slots, shrink the gap by a round-dependent factor
   gap       *= round_depth_shrinkage(r)
   // Home ice applies to whichever side hosts
   p_top_wins_game = sigmoid(gap + ice_bonus)     // when top hosts
                   = sigmoid(gap - ice_bonus)     // when bottom hosts
   ```
   The logistic is the same shape as the Elo formula but on a per-game scale (not chess-Elo's 400-point scale).

3. **Simulate the series** by repeating step 2 until one side reaches four wins. Track how many games were actually played.

4. **Accumulate remaining-games per NHL team.** Every series adds `games_played_in_this_trial` to both participants' "remaining games this trial" counter.

5. **Sample fantasy points.** For each rostered skater:
   ```
   rg = remaining_games[player.nhl_team]  // same number for teammates
   sim_pts = sample_negbin(player.ppg × rg, NB_DISPERSION, rng)  if rg > 0
           = 0                                                    otherwise
   total = player.playoff_points_so_far + sim_pts
   ```
   Add `total` to the player's sample array and the fantasy team's running total.

6. **Rank teams** by simulated total. The top team earns a win; ties split the win fractionally. Track `head_to_head[i][j]` += 1 for every trial where team `i` strictly beat team `j`.

After all trials:
- `win_prob[i] = team_wins[i] / trials`
- `mean[i]`, `median[i]`, `p10[i]`, `p90[i]` from the sample array
- `head_to_head[i][j] / trials` per opponent pair
- Per NHL team: `P(advance past round r) = round_advance[r][team] / trials`

### Why Negative-Binomial, not Poisson?

A Poisson variable has `variance = mean`. Real NHL skater point totals per game are overdispersed: a top scorer alternates blank nights with 3-point games. We model scoring as a Gamma-Poisson mixture ([`race_sim.rs:267-277`](../backend/src/domain/prediction/race_sim.rs)):

```
variance(λ) = λ + λ² / r    where r = NB_DISPERSION = 4.0
```

At `λ = 1` that's `variance ≈ 1.25`. At `λ = 5`, `variance ≈ 11.25`. This widens the tails of player projections so p10 and p90 reflect real playoff variance, not a Poisson straitjacket.

### Why correlated teammates fall out for free

Two skaters on the Oilers share the same `remaining_games[EDM]` inside one trial. If that trial sends Edmonton to the Cup final (20 games played), both Oilers get to sample points from a 20-game window. If Edmonton gets swept in round 1 (4 games), both get a 4-game window. The correlation is structural - no covariance matrix needed.

Cross-roster correlation (two different fantasy teams both rostering an Oiler) also drops out: in any trial where Edmonton goes deep, both fantasy teams benefit. Head-to-head probabilities therefore reflect real rostering overlaps.

### Mean-reversion for future rounds

`round_depth_shrinkage(r)` ([`race_sim.rs:514`](../backend/src/domain/prediction/race_sim.rs)) shrinks rating gaps for Future slots as we predict further into the bracket. A team that looks 200 Elo stronger today is less likely to *still appear* 200 Elo stronger in the Cup final - the field has been filtered to survivors of similar quality. `InProgress` and `Completed` slots skip this shrinkage because the teams are known.

## 6. Series projection - not the sim

File: [`backend/src/domain/prediction/series_projection.rs`](../backend/src/domain/prediction/series_projection.rs).

Used purely for UI colour coding on the series cards, not by the Monte Carlo. Three functions:

- `classify(wins, opp_wins) → SeriesStateCode` - buckets a best-of-7 into `Advanced`, `AboutToAdvance`, `Leading`, `Tied`, `Trailing`, `FacingElim`, `Eliminated` for red-to-green axis rendering.
- `probability_to_advance(wins, opp_wins) → f32` - an empirical lookup table based on NHL series outcomes: `~0.05` at 0-3 down, `~0.50` tied, `~0.95` at 3-0 up ([`series_projection.rs:64-87`](../backend/src/domain/prediction/series_projection.rs)). Not a simulation.
- `games_remaining(wins, opp_wins) → u32` - max games left in the best-of-7.

The race simulator does not use the lookup table; it re-derives series probabilities from team strength per trial. The lookup is the right tool for UI because it does not need rating inputs and gives stable numbers a user can recognise ("3-0 up, 95% chance to advance").

## 6a. Player grading

File: [`backend/src/domain/prediction/grade.rs`](../backend/src/domain/prediction/grade.rs).

Used by `GET /api/fantasy/teams/{id}` to convert a player's projection and their actual playoff output into a letter grade (A–F) and a descriptive status bucket. Pure domain; no IO.

### Grade formula

```
expected   = ppg × games_played
variance   = max(expected × (1 + ppg / NB_DISPERSION), 0.5)
z          = (actual − expected) / sqrt(variance)
```

`NB_DISPERSION = 4.0` is the same Negative-Binomial parameter the Monte Carlo uses for per-player point draws, so the variance model the grader compares against is the one the simulator actually samples from. The `0.5` floor keeps `z` finite for fringe skaters whose expected output is near zero.

Cutoffs:

| Letter | Z-score band |
| --- | --- |
| A | `z ≥ 1.0` |
| B | `0.3 ≤ z < 1.0` |
| C | `−0.3 ≤ z < 0.3` |
| D | `−1.0 ≤ z < −0.3` |
| F | `z < −1.0` |

Gated to `NotEnoughData` when `games_played < MIN_GAMES_FOR_GRADE` (=2) or `ppg ≤ 0` — a single goose-egg doesn't brand a player as cold, and a skater whose projection itself rounds to zero can't meaningfully over- or under-perform.

### Bucket classifier

`classify_bucket(grade_report, projection, series_state)` returns one of seven labels. Priority order — first match wins:

1. `series_state == Eliminated` → `TeamEliminated`
2. `projection.active_prob < 1.0` → `ProblemAsset` (likely-scratch signal from `player_projection::project_one`)
3. `projection.toi_multiplier < 0.80` → `ProblemAsset` (demoted off the depth chart)
4. Grade A or B → `Outperforming`
5. Grade D or F + series in `FacingElim`/`Trailing` + `toi_multiplier ≥ 0.9` → `KeepFaith` (role intact, finishing cold)
6. Grade F → `NeedMiracle`
7. Grade D → `FineButFragile`
8. Grade C or `NotEnoughData` → `OnPace`

**Descriptive, not prescriptive.** The roster is locked for the playoffs; the bucket labels describe the player's situation, not a roster action. The frontend's `BucketPill` component maps them to reader-facing strings like `AHEAD`, `DUE`, `FADING`, `NOT IN LINEUP` — never "start/sit/drop" language.

### Remaining impact

`remaining_impact(ppg, expected_games_total, team_games_already_played, nhl_team_eliminated)` projects the rest-of-run contribution:

```
remaining_games = max(0, expected_games_total - team_games_already_played)
remaining_points = ppg × remaining_games
```

`expected_games_total` is pulled per NHL team from the cached `race_odds:v4:*` payload via [`infra::prediction::race_odds_cache::load_nhl_team_odds`](../backend/src/infra/prediction/race_odds_cache.rs) — never re-run the Monte Carlo on the request path. When the cache is cold or the NHL team is out, both fields zero out; the page renders `—` rather than a crash.

## 7. Inputs from the database

The `infra::prediction` module (adapters) builds the pure-domain inputs from the mirror:

- Elo ratings: seed from `nhl_standings` (with shrinkage), fold every row of `playoff_game_results` through `apply_game` in chronological order.
- Home-ice bonuses: from the same `nhl_standings` rows.
- Goalie bonuses: from `nhl_goalie_season_stats` (wins, save_pctg).
- Bracket state: from `nhl_playoff_bracket.carousel` (JSONB).
- Player projections: from `playoff_skater_game_stats` (recent game log) + `nhl_skater_season_stats` (RS points) + `historical_playoff_skater_totals` (5-year prior).

All mirror tables are populated by background jobs (see [`04-nhl-integration.md`](./04-nhl-integration.md) and [`07-background-jobs.md`](./07-background-jobs.md)). The prediction path never calls the NHL API directly.

## 8. Calibration

File: [`backend/src/infra/calibrate.rs`](../backend/src/infra/calibrate.rs). Admin endpoints at `GET /api/admin/calibrate` and `GET /api/admin/calibrate-sweep` ([`handlers/admin.rs:291-318`](../backend/src/api/handlers/admin.rs)).

Calibration answers: are the probabilities this model produces actually honest? If the model says "30% chance of winning round 1", then across all 30% predictions, the team should actually win about 30% of the time.

The harness ([`calibrate.rs:1-22`](../backend/src/infra/calibrate.rs)):

1. Realise a completed historical playoff season by folding every game through `backtest::reconstruct_bracket_from_results`. This gives the actual outcomes (who advanced, who won the Cup).
2. Rebuild the day-1 `BracketState` - round 1 at 0-0, every later round `Future`.
3. Run the current engine against that state with production hyperparameters.
4. Compare predicted round-advancement probabilities to realised outcomes using Brier score and log-loss per round.

Brier score is `mean((predicted_prob - actual)²)`. Lower is better; perfect is zero, a random 50/50 guess on a 50/50 outcome is `0.25`.

### The sweep

`CalibrationKnobs` ([`calibrate.rs:51-72`](../backend/src/infra/calibrate.rs)) lets a caller vary:

- `points_scale` - Elo per RS point (default 6.0)
- `shrinkage` - Bayesian shrinkage on RS deviation (default 0.7)
- `k_factor` - logistic slope (default `ln(10)/400 ≈ 0.00576` on the Elo scale; race-sim uses 0.010 pre-sigmoid)
- `home_ice_elo` - league-wide home bonus (default 35)
- `trials` - Monte Carlo iterations per cell

The production `DEFAULT_K_FACTOR = 0.010` and `PRODUCTION_SHRINKAGE = 0.7` come from sweeps against 2021-22 through 2024-25 backfilled seasons. Earlier values concentrated Cup probability too tightly on chalky favourites (Colorado at 39 % on 2025-26 vs HockeyStats reference of about 13 %). The docstring on `PRODUCTION_SHRINKAGE` ([`playoff_elo.rs:43-49`](../backend/src/domain/prediction/playoff_elo.rs)) carries the rationale.

Operators run the sweep off-line. The endpoint is capped at 200 grid cells so a misconfigured invocation cannot peg the server for hours ([`handlers/admin.rs:289-290`](../backend/src/api/handlers/admin.rs)).

## Where the outputs surface

| UI | Source | What it shows |
| --- | --- | --- |
| `/api/race-odds` (Race Odds page, Fantasy Champion board) | `race_sim::simulate` wrapped in `response_cache` | Per-fantasy-team win probability, head-to-head, Stanley Cup odds per NHL team |
| `/api/pulse` (Pulse page) | `series_projection` for series badges; race-sim outputs for fan-wide context | "Your team has X% chance to finish first"; today's stakes |
| `/api/insights` (Insights page) | Player projection + bracket enrichment | Hot / cold players; round previews |
| `/api/pulse` (Pulse "Your Read" + "Your League" blocks) | `project_players` + `grade` + cached race-odds + Claude narrator | Per-player box line + grade + bucket + remaining-points impact; team-level descriptive diagnosis narrative; top-3 projected finishers across the league |

See [`03-api.md`](./03-api.md) for endpoint shapes and cache keys.
