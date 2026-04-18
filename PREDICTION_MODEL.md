# Prediction Model Reference

Technical reference for the Fantasy Puck race-odds / playoff-odds Monte Carlo system. Covers the full pipeline from NHL API ingest through bracket simulation to the `GET /api/race-odds` response, plus the admin endpoints, data model, tunable constants, calibration state, and known gaps.

Current versions at the time of writing: **backend 1.16.0, frontend 1.10.2** (2026-04-18).

---

## 1. Overall architecture

The prediction system runs one Monte Carlo sweep of the NHL playoff bracket and returns per-fantasy-team win probabilities, per-player projected totals, and per-NHL-team round-advancement odds. Two surfaces consume the output:

- **Pulse** (`/pulse`) — personal / league race. Reads the `teams` and `players` fields of `RaceSimOutput` to render win-probability bars, the League Race Table, the rivalry card, and the My Stakes section.
- **Insights** (`/insights`) — NHL-generic view. Reads the `nhl_teams` field (per-team `advance_round1_prob`, `conference_finals_prob`, `cup_finals_prob`, `cup_win_prob`, `expected_games`) to render the Stanley Cup Odds table and the Playoff Bracket Tree strength labels.

### Pipeline

```
┌──────────────────────┐
│ NHL API              │  standings, carousel, series games, boxscores
│ api-web.nhle.com     │  (undocumented; see nhl_constants.rs)
└──────────┬───────────┘
           │ (reqwest via NhlClient — rate-limited, cached)
           ▼
┌──────────────────────┐    nightly ingest     ┌──────────────────────┐
│ utils/playoff_ingest │──────────────────────►│ PostgreSQL (Supabase)│
│ utils/historical_seed│  startup backfill     │                      │
│ admin backfill       │  ad-hoc             ─►│ playoff_game_results │
└──────────────────────┘                       │ playoff_skater_game_ │
                                               │   stats              │
                                               │ historical_playoff_  │
                                               │   skater_totals      │
                                               │ response_cache       │
                                               └──────────┬───────────┘
                                                          │
           ┌──────────────────────────────────────────────┘
           │  (SQL reads; sqlx)
           ▼
┌──────────────────────┐      pure-domain     ┌──────────────────────┐
│ infra/prediction.rs  │─────────────────────►│ domain/prediction/   │
│  compute_current_elo │                      │  playoff_elo         │
│  project_players     │                      │  player_projection   │
└──────────┬───────────┘                      │  team_ratings        │
           │                                  │  race_sim (5000 MC)  │
           ▼                                  │  backtest (metrics)  │
┌──────────────────────┐                      └──────────────────────┘
│ api/handlers/        │                                 ▲
│   race_odds.rs       │─────────────────────────────────┘
│   insights.rs        │  builds RaceSimInput, runs simulate()
│   pulse.rs           │  on spawn_blocking, caches in response_cache
└──────────┬───────────┘
           │
           ▼
       HTTP JSON → React Query → Pulse + Insights UI
```

Pre-warming runs at 10:00 UTC (see `utils/scheduler.rs`) for the Insights and race-odds endpoints so the first user visit of the day hits cache.

### Layering rule

Followed throughout: **`domain/` is pure, `infra/` is the DB wrapper**. No framework deps under `domain/prediction/` (no `sqlx`, no `axum`, no `reqwest`, no `tracing` outside debug). The engine is a candidate for extraction into a standalone crate or HTTP service — see `PREDICTION_SERVICE.md` and the Bulletproof Rust Web reference in §13.

---

## 2. Code layout

### `backend/src/domain/prediction/` (pure)

All files declared in `backend/src/domain/prediction/mod.rs`.

| File | Purpose |
|---|---|
| `playoff_elo.rs` | Dynamic playoff Elo: seed from RS standings, replay every playoff game. Public: `BASE_ELO`, `POINTS_SCALE`, `HOME_ICE_ADV`, `K_FACTOR` constants; `home_bonus_from_standings` (line 49), `seed_from_standings` (line 86), `GameResult` struct (line 115), `apply_game` (line 126). |
| `player_projection.rs` | Bayesian blend of rs_ppg + po_ppg + recent_ppg + historical_ppg. Public: `ALPHA`, `BETA`, `PO_TO_RECENT_WEIGHT`, `RECENT_HALF_LIFE_GAMES`, `RECENT_WINDOW`, `ABSENT_MULTIPLIER`, `MIN_APPEARANCE_TEAM_GAMES` constants; `PlayerInput` / `Projection` structs; `project_one` (line 80), `recency_weighted_ppg` (line 138). |
| `race_sim.rs` | Core Monte Carlo. Public: `SeriesState` enum (line 56), `BracketState` (line 92), `TeamRating` (line 155), `SimPlayer` (line 171), `SimFantasyTeam` (line 186), `RaceSimInput` (line 193), `TeamOdds`, `PlayerOdds`, `NhlTeamOdds`, `RaceSimOutput`. Engine: `simulate` (line 331), `simulate_with_seed` (line 336), internal `run` (line 341), `simulate_series` (line 733). Constants: `HOME_ICE_ELO = 35.0` (line 219), `DEFAULT_TRIALS = 5000` (line 223), `DEFAULT_K_FACTOR = 0.010` (line 235), `DEFAULT_PPG = 0.45` (line 238), `NB_DISPERSION = 4.0` (line 250), `HOME_ICE_SCHEDULE` (line 714). |
| `team_ratings.rs` | Pre-playoff blended RS-points rating: `0.7·season_points + 0.3·(L10_ppg·82)`. `from_standings` (line 28). Used on the pre-playoff path and for Insights bracket-tree labels when `game_type != 3`. |
| `series_projection.rs` | Series-level helpers used by Insights bracket enrichment (probability a series goes 4/5/6/7, expected games). |
| `backtest.rs` | Calibration metrics + bracket reconstruction. `brier_score` (line 24), `log_loss` (line 42), `CalibrationBucket` + `calibration_curve` (line 79), `mae` (line 117), `rmse` (line 126), `interval_coverage` (line 137), `ResultRow` (line 155), `reconstruct_bracket_from_results` (line 191). |

### `backend/src/infra/prediction.rs` (DB wrapper)

Only place SQL touches the engine.

- `compute_current_elo(db, standings, season)` (line 27) — seeds Elo from `standings` JSON, replays every row in `playoff_game_results` for that season in chronological order, returns `HashMap<abbrev, elo>`.
- `project_players(db, season, players, team_games_played)` (line 68) — two batched queries (`playoff_skater_game_stats` + `historical_playoff_skater_totals`), then one call to `player_projection::project_one` per player. Returns `HashMap<nhl_id, Projection>`.

### `backend/src/infra/calibrate.rs` (DB wrapper for backtest)

- DTOs: `TeamOutcome` (line 48), `PerTeamCalibration` (line 58), `CalibrationReport` (line 69).
- `calibrate_season(db, nhl, season)` (line 89) — loads the season's completed games, reconstructs realized outcomes via `backtest::reconstruct_bracket_from_results`, rebuilds the day-1 `BracketState`, seeds Elo + per-team home-ice bonus from historical standings, runs the sim with production hyperparameters, and scores predicted vs realized with Brier + log-loss per round.
- Internal constant `ELO_K_FACTOR = LN_10 / 400.0 ≈ 0.00576` (line 40).
- `fetch_historical_standings` (line 317) — post-v1.12.3 helper: walks back day-by-day from the season's earliest playoff game date (up to 10 days) looking for a non-empty `/v1/standings/{date}` response. Falls back to live standings with a warn log if every attempt is empty (the NHL feed serves an empty array during the RS-to-playoffs gap).
- `build_ratings` (line 292) — turns a standings JSON into `(HashMap<String, TeamRating>, k_factor, home_ice_bonus)` for the calibration sim.

### `backend/src/utils/playoff_ingest.rs` (NHL → DB)

- `ingest_playoff_games_for_date(db, nhl, date)` (line 27) — nightly job. Pulls the schedule for one date, filters to completed `game_type == 3`, upserts team-level rows into `playoff_game_results` and per-skater rows into `playoff_skater_game_stats`.
- `ingest_playoff_games_for_range(db, nhl, start, end)` (line 81) — loops the single-day ingest across a date range. Used by the startup backfill (if `playoff_skater_game_stats` is empty) and by the admin `backfill-historical` endpoint.
- `is_playoff_skater_game_stats_empty(db)` (line 116) — boot-time check.
- `rebackfill_playoff_season_via_carousel(db, nhl, season)` (line 138) — the reliable historical path. Walks the carousel for `season` (8-digit string), fetches every series' games list, and upserts team-level rows. **Team-level only** — skater-level stats are not populated here. `season` must be 8-digit (e.g. `20222023`); the 4-digit year returns 404.

### `backend/src/api/handlers/race_odds.rs`

Thin HTTP handler. `get_race_odds` extracts query params, dispatches to either league mode (`run_league_mode`) or champion mode (`run_champion_mode`), builds a `RaceSimInput`, runs `simulate` on `spawn_blocking`, caches under `race_odds:v2:{...}`, returns JSON.

Important internals:
- `bracket_from_carousel` — walks the NHL `/playoff-series/carousel` response and builds a `BracketState`, padding missing rounds / slots with `SeriesState::Future`.
- `build_fantasy_teams_playoff` — batches player-projection queries and assembles `SimFantasyTeam`s for the league roster.
- `build_champion_input` — same pattern for the global top-40 skater leaderboard used by Insights when no league is selected.
- Cache key versioned `race_odds:v2:...` (see line 91) so model changes invalidate stale responses on deploy.
- Logistic scale on the Elo path: `ELO_K_FACTOR = LN_10 / 400.0` (line 49) — a reuse-compatible duplicate of the constant in `infra/calibrate.rs`.

---

## 3. Data model

The sim reads four tables. All live in the default `public` schema. Migrations are under `backend/supabase/migrations/` and are embedded in the binary via `sqlx::migrate!` at startup (since v1.8.1), so Fly deploys self-apply.

### `playoff_game_results`

One row per playoff game. Migration: `20260418100002_playoff_game_results.sql`.

| Column | Type | Notes |
|---|---|---|
| `season` | `INTEGER` | 8-digit (e.g. `20252026`). |
| `game_type` | `SMALLINT` | Always `3` for playoff rows. |
| `game_id` | `BIGINT` | **Primary key**. NHL game ID. |
| `game_date` | `DATE` | Replay ordering anchor. |
| `home_team`, `away_team` | `TEXT` | Abbreviations. |
| `home_score`, `away_score` | `INTEGER` | |
| `winner` | `TEXT` | Winning team's abbreviation (redundant but materialized). |
| `round` | `SMALLINT` | 1–4. May be NULL for historical rows. **Ignored by `reconstruct_bracket_from_results`** — rounds are re-inferred topologically because the NHL schedule endpoint returns this inconsistently for past seasons. |

Indexes: `(game_date, game_id)` for chronological replay; `(home_team, game_date)` and `(away_team, game_date)` for per-team queries.

**Read by**: `infra::prediction::compute_current_elo` (chronological replay), `infra::calibrate::load_result_rows`, Insights bracket-tree enrichment.
**Written by**: `ingest_single_game` (nightly), `rebackfill_playoff_season_via_carousel` (admin).

### `playoff_skater_game_stats`

One row per `(game_id, player_id)`. Migration: `20260418100000_playoff_skater_game_stats.sql`.

| Column | Type | Notes |
|---|---|---|
| `season` | `INTEGER` | |
| `game_type` | `SMALLINT` | `3` only. |
| `game_id` | `BIGINT` | |
| `game_date` | `DATE` | |
| `player_id` | `BIGINT` | NHL player ID. |
| `team_abbrev`, `opponent` | `TEXT` | |
| `home` | `BOOLEAN` | |
| `goals`, `assists`, `points` | `INTEGER` | |
| `shots`, `pp_points`, `toi_seconds` | nullable | Partial signals; `pp_points` stores PP goals when full PP-points is unavailable. |

Primary key: `(game_id, player_id)`.

Indexes: `(player_id, game_date DESC)` — primary read pattern for the recency-weighted projection term; `(team_abbrev, game_id)`; `(game_date)`.

**Read by**: `infra::prediction::project_players` (via `game_date DESC` slice).
**Written by**: `ingest_single_game`. Goalies are skipped at the ingest boundary (skater-only fantasy format). Not populated by `rebackfill_playoff_season_via_carousel` (team-level only).

### `historical_playoff_skater_totals`

5-year rollup, one row per player. Migration: `20260418100001_historical_playoff_skater_totals.sql`.

| Column | Type | Notes |
|---|---|---|
| `player_name` | `TEXT` | Natural-key component. |
| `born` | `INTEGER` | Year, disambiguates the two Sebastian Ahos. |
| `team`, `position` | `TEXT` | `team` is most recent or "TOT". |
| `gp`, `g`, `a`, `p` | `INTEGER` | 5-year totals. |
| `shots`, `toi_seconds` | nullable | |

Primary key: `(player_name, born)`. Index on `player_name` only — the projection module resolves by name first, falls back to the full key on collision.

**Read by**: `infra::prediction::project_players` (shrinkage prior).
**Written by**: `utils::historical_seed` at startup from an embedded CSV bundled via `include_str!` (output of `backend/scripts/parse_historical_playoff_skaters.py`, 582 rows after goalies are filtered).

### `response_cache`

Standard key/value response cache with a `hockey_date` column. The race-odds handler writes under `race_odds:v2:{league_id?}:{season}:{game_type}:{date}`. The `v2` version prefix forces a miss on deploys that bump the model contract; bump it again when adding or removing fields on `RaceSimOutput` or changing the Monte Carlo in a way that should not serve old cached JSON.

**Read by**: every handler that caches (race-odds, insights, pulse, games).
**Written by**: same.

---

## 4. Sim algorithm

Everything below is in `domain/prediction/race_sim.rs`.

### Inputs

`RaceSimInput` (line 193):
- `bracket: BracketState` — every round, every slot, tagged with its lifecycle state.
- `ratings: HashMap<String, TeamRating>` — `abbrev → { base, home_bonus }` where `base` is the team-strength scalar (Elo on the playoff path, RS points on the pre-playoff path) and `home_bonus` is that team's per-team home-ice advantage in the same units as `base` (0.0 means "use the league-wide `home_ice_bonus` fallback").
- `k_factor: f32` — logistic scale. `DEFAULT_K_FACTOR = 0.010` for the RS-points scale, `ELO_K_FACTOR = ln(10)/400 ≈ 0.00576` for the Elo scale.
- `home_ice_bonus: f32` — pre-sigmoid home-ice nudge, added to the gap when the home-ice team hosts game `i`, subtracted when visiting. `0.0` models neutral-site. Expressed in the same units the caller is passing to `k_factor` (i.e. already scaled by `k_factor` if `ratings` are on the Elo scale).
- `fantasy_teams: Vec<SimFantasyTeam>`.

### Bracket state

`SeriesState` enum (line 56):
- **`Future`** — participants unknown. Filled in per-trial from the winners of this slot's two feeder slots in the previous round.
- **`InProgress { top_team, top_wins, bottom_team, bottom_wins }`** — both teams known, may be 0-0 (about to start) or live. `top_team` holds home-ice under the carousel convention.
- **`Completed { winner, loser, total_games }`** — series is done. Propagates `winner` unchanged, adds zero remaining games.

`BracketState` (line 92) is `rounds: Vec<Vec<SeriesState>>`, positional:
- `rounds[0]` — first round (up to 8 series)
- `rounds[1]` — conference semifinals (up to 4)
- `rounds[2]` — conference finals (up to 2)
- `rounds[3]` — Stanley Cup Final (up to 1)

Winner of `rounds[r][2i]` meets winner of `rounds[r][2i+1]` in `rounds[r+1][i]`. Matches the NHL's static bracket (2013-present); teams do not re-seed between rounds.

### Per-trial walk

One trial of `run` (line 341):

1. Clear scratch (`remaining_games`, `team_totals`).
2. For each round `r` in order, for each slot `i`:
   - If `Completed`: emit the known winner, add 0 games.
   - If `InProgress`: call `simulate_series(top_wins, bottom_wins, gap, ice_bonus, top_has_home_ice=true, rng)` where `gap = k · ((top.base − bot.base) + (top.goalie_bonus − bot.goalie_bonus))`. `remaining_games` counts *only* games played from now on (`outcome.total_games() - (top_wins + bottom_wins)`), so already-played games are not double-counted.
   - If `Future`: look up the two feeder winners in `winners[r-1]`. Home-ice goes to whichever has the higher `base` rating (proxy for the eventual RS-standings-based home-ice assignment). Since v1.15, the base-plus-goalie gap is multiplied by `round_depth_shrinkage(r)` — 1.00 at round 0, 0.85 at semis, 0.70 at conference finals, 0.55 at Cup Final — so compounded uncertainty about distant matchups doesn't compound fake confidence. If a feeder slot is missing (structurally incomplete bracket), the slot degrades gracefully — the winner is the empty string and downstream rounds skip it.
3. Record `nhl_round_advance[r][team_idx] += 1` for each slot winner. Winning slot `(r, i)` means the team advanced out of round `r`, i.e. reached round `r+1` (or the Cup when `r == 3`).
4. After the walk, for each fantasy player: draw `sim_pts = sample_negbin(ppg * remaining_games, NB_DISPERSION, rng)` when their team has remaining games, else 0. Add to `playoff_points_so_far`. Accumulate into each fantasy team's total.
5. Sort fantasy teams by total; update `team_wins` and `team_top3` with **fractional tie-splitting** (see below).
6. Update pairwise `h2h_wins[i][j]` with strict `>` — ties contribute to neither side.

### `simulate_series` (line 733)

NHL 2-2-1-1-1 home-ice schedule:
```rust
const HOME_ICE_SCHEDULE: [bool; 7] = [true, true, false, false, true, false, true];
// Games 1, 2, 5, 7 are home for the home-ice-owning team.
```

Per-game probability:
```
game_idx   = top_wins + bot_wins           // 0-indexed position in the series
hi_is_home = HOME_ICE_SCHEDULE[game_idx]   // does home-ice team host this game?
top_is_home = (top_has_home_ice == hi_is_home)
effective_gap = gap_times_k ± ice_bonus    // + when top_is_home, − otherwise
p_top_wins_game = sigmoid(effective_gap).clamp(0.05, 0.95)
```

The `[0.05, 0.95]` clamp prevents extreme rating gaps from producing sweeps every trial — real upsets happen. Loop until one side reaches 4 wins; return `SeriesOutcome { top_wins, bot_wins }`.

### Per-player point draws: Negative Binomial

`sample_negbin(mean, r, rng)` draws via the Gamma-Poisson mixture:
1. `lambda = Gamma(shape=r, scale=mean/r)` — so `E[lambda] = mean`, `Var[lambda] = mean²/r`.
2. Return `Poisson(lambda)`.

Resulting distribution: `E[X] = mean`, `Var[X] = mean + mean²/r`. With `r = 4`, variance ≈ 1.25·λ at λ=1 and ≈ 2.5·λ at λ=5 — captures the overdispersion of real skater scoring (blank nights alternating with multi-point games) better than a plain Poisson.

Fallback to plain Poisson when `mean * dispersion` would overflow (see the sampling function's defensive branch).

### Aggregation

From `run` (line 341) into `RaceSimOutput`:

- **`win_prob`**: `team_wins[i] / trials`. Fractional tie-splitting — if N teams tie for first with strict equality (within 1e-6), each gets `1/N`. Previously the sort-first team took full credit, so small leagues drifted off ∑=1.0.
- **`top3_prob`**: same fractional split at the rank-3 boundary. Teams tied at the 3rd distinct score share the remaining `3 - earned_so_far` credit.
- **`head_to_head`**: `h2h_wins[i][j] / trials`. Strict inequality, ties contribute to neither side (matches the "P(you finish ahead of rival)" semantics the Rivalry card renders).
- **`projected_final_mean` / `p10` / `p90` / `projected_final_median`**: summary stats on the per-trial team-total samples (`summarise` function).
- **`nhl_teams[i].advance_round1_prob / conference_finals_prob / cup_finals_prob / cup_win_prob`**: `nhl_round_advance[r][i] / trials` for `r = 0, 1, 2, 3`.
- **`expected_games`**: `already_played + nhl_games_total[i] / trials` — total games across the whole run, not remaining. Frontends (Stanley Cup Odds, My Stakes) read it as a total, so the semantic is preserved by adding back games already played.

---

## 5. Inputs to the sim (priority order)

### 5.1 Team ratings — `playoff_elo.rs` (playoff path)

Seeded once per request from the NHL standings feed; updated by replaying every row in `playoff_game_results` for the current season.

- **Seed**: `elo_0 = BASE_ELO + POINTS_SCALE · (season_points − league_avg_points)` (line 108).
- **Replay** (per game, chronological): standard logistic-Elo update with home-ice added to the home team's rating and a `ln(|goal_diff|+1)` Silver-style blowout multiplier:
  ```
  gap     = elo_home − elo_away + HOME_ICE_ADV
  p_home  = 1 / (1 + 10^(−gap / 400))
  delta   = K_FACTOR · ln(|goal_diff| + 1) · (result − p_home)
  elo_home += delta;  elo_away -= delta
  ```
- **Current constants** (`playoff_elo.rs`):
  - `BASE_ELO = 1500.0` (line 26)
  - `POINTS_SCALE = 6.0` (line 30) — 25 RS-point spread → ~150-Elo window around base.
  - `HOME_ICE_ADV = 35.0` (line 33) — league-wide constant used inside the replay. Applied in addition to each team's per-team home-ice bonus on the forward sim path.
  - `K_FACTOR = 6.0` (line 36) — base update rate. `ln(goal_diff + 1)` multiplies it so a 1-goal win moves ratings by ~K, a 5-goal win by ~K·ln(6) ≈ 1.8·K.

### 5.2 Per-team home-ice bonus — `home_bonus_from_standings` (`playoff_elo.rs:49`)

Derived from each team's RS home-vs-road points-pct split:
```
home_pct = (homeWins·2 + homeOtLosses) / (home_gp · 2)
road_pct = (roadWins·2 + roadOtLosses) / (road_gp · 2)
raw      = (home_pct − road_pct) · 400
bonus    = raw.clamp(10.0, 80.0)
```

Teams with `home_gp + road_gp < 5` are dropped (small-sample noise). The scale factor 400 comes from matching Elo's 35-point `HOME_ICE_ADV ≈ 0.55 win prob` against an 0.08-point-pct gap, giving ~437; rounded to 400. Clamp floor 10 keeps it strictly positive; ceiling 80 smooths freak home/road splits.

The forward sim (`race_sim::simulate_series`) reads `TeamRating.home_bonus` and prefers it when non-zero, falling back to the league-wide `input.home_ice_bonus` otherwise. Pre-playoff path passes `home_bonus: 0.0`.

### 5.3 Player projection — `player_projection.rs`

Since v1.14.0, the projection consumes a per-game `GameStats { goals, assists, shots, pp_points, toi_seconds }` struct rather than bare point totals. Two signals on top of the Bayesian blend:

1. **Shot-volume stabilisation** of the observed playoff goal rate.
2. **TOI-ratio multiplier** for lineup-role changes (demotions / promotions).

```
rs_ppg         = rs_points / 82
po_gp          = len(game_log)

# Stabilised playoff rate (replaces raw points/gp)
obs_goals_rate = Σ goals_i / po_gp
shots_rate     = Σ shots_i / po_gp                       # when shot data is available
expected_goals = shots_rate · LEAGUE_SH_PCT
stable_goals   = (1 − w_s)·obs_goals_rate + w_s·expected_goals
po_rate        = stable_goals + Σ assists_i / po_gp      # stabilised goals + raw assists

recent_rate    = Σ 2^(−i/H) · points_i / Σ 2^(−i/H)      over last N (most-recent-first)
blended_po     = W · po_rate + (1 − W) · recent_rate
hist_ppg       = hist_points / hist_gp

projected_ppg  = (ALPHA·rs_ppg + po_gp·blended_po + BETA·hist_ppg)
                 / (ALPHA + po_gp + BETA)

# TOI-role multiplier (applied after the blend)
recent_toi_avg = mean(toi_seconds over most-recent TOI_RATIO_RECENT_WINDOW games)
older_toi_avg  = mean(toi_seconds over earlier games)
toi_mult       = (recent_toi_avg / older_toi_avg).clamp(
                   TOI_RATIO_DERATE_FLOOR, TOI_RATIO_BOOST_CAP)
                 # 1.0 when either window lacks enough non-null TOI data

if team_games_played >= MIN_APPEARANCE_TEAM_GAMES and po_gp == 0:
    availability = ABSENT_MULTIPLIER
else:
    availability = 1.0

final_ppg      = projected_ppg · toi_mult · availability
```

Constants (`player_projection.rs`):
- `ALPHA = 10.0` (RS-prior games-equivalent strength)
- `BETA = 4.0` (historical prior games-equivalent strength)
- `PO_TO_RECENT_WEIGHT = 0.65` (`W`)
- `RECENT_HALF_LIFE_GAMES = 4.0` (`H`)
- `RECENT_WINDOW = 10` (`N`)
- `ABSENT_MULTIPLIER = 0.3`
- `MIN_APPEARANCE_TEAM_GAMES = 3`
- `LEAGUE_SH_PCT = 0.095` — NHL league-average shooting percentage, anchor for goal-rate regression.
- `SHOT_STABILIZATION_WEIGHT = 0.40` (`w_s`)
- `TOI_RATIO_RECENT_WINDOW = 3` — games defining "recent TOI".
- `TOI_RATIO_BASELINE_MIN = 3` — minimum non-null older-TOI games before the ratio activates.
- `TOI_RATIO_DERATE_FLOOR = 0.70`, `TOI_RATIO_BOOST_CAP = 1.10` — asymmetric clamp, demotions matter more than promotions.

`rs_points / 82` is still used because the `StatsLeaders` leaderboard the crate consumes does not carry GP per player; this slightly under-projects players who missed RS games. See §10.

### 5.3b Goalie strength — `goalie_rating.rs` (v1.15+)

Per-team Elo delta derived from the regular-season starter's SV%, sitting alongside `base` and `home_bonus` on `TeamRating`.

- **Identify starter**: for each team, pick the goalie with the most wins from the RS leaderboard (minimum `MIN_WINS_FOR_STARTER = 3`). Tandems (secondary starter within 3 wins of the primary) average both bonuses.
- **Map SV% to Elo**: `bonus = ((sv_pct - LEAGUE_AVG_SVP) × GOALIE_BONUS_SCALE).clamp(±GOALIE_BONUS_CLAMP)`.
- **Apply in sim**: `simulate_series` gets a gap that already includes `k · (top.goalie_bonus − bot.goalie_bonus)`. For `Future` slots the contribution is shrunk by `round_depth_shrinkage` at the same rate as `base` — the assumed starter may be rested, injured, or replaced by the deep rounds.

Constants (`goalie_rating.rs`):
- `LEAGUE_AVG_SVP = 0.905`
- `GOALIE_BONUS_SCALE = 800.0` (0.020 SV% delta → 16 Elo)
- `GOALIE_BONUS_CLAMP = 30.0` (caps out at roughly ±.9425 SV%)
- `MIN_WINS_FOR_STARTER = 3.0`

Data source: `NhlClient::get_goalie_stats(season, 2)` → `/v1/goalie-stats-leaders/{season}/2`. Hardcoded to regular-season game-type because playoff SV% is a small-sample leak of what the model is predicting.

### 5.4 k_factor

- **Elo path** (current playoffs): `ELO_K_FACTOR = ln(10) / 400.0 ≈ 0.00576`. Defined in both `api/handlers/race_odds.rs:49` and `infra/calibrate.rs:40` (duplicated constants). Reproduces the standard Elo identity `sigmoid(x · ln10 / 400) = 1 / (1 + 10^(−x/400))`.
- **Pre-playoff path**: `DEFAULT_K_FACTOR = 0.010` (`race_sim.rs:235`). Tuned against HockeyStats.com's published round-1 series probabilities for the 2026 playoffs; the earlier value (0.03) over-concentrated Cup probability on the top standings seed.

### 5.5 NB_DISPERSION

`NB_DISPERSION = 4.0` (`race_sim.rs:250`). Used by `sample_negbin` in the player-points draw.

---

## 6. Constants and current values

The full tunable-hyperparameter table. This is the grid-search target.

| Constant | Value | Location | Meaning |
|---|---|---|---|
| `BASE_ELO` | 1500.0 | `playoff_elo.rs:26` | League-average team Elo seed. |
| `POINTS_SCALE` | 6.0 | `playoff_elo.rs` | Elo points per RS-point of separation before shrinkage. Effective scale (post-shrinkage) is `POINTS_SCALE · PRODUCTION_SHRINKAGE = 4.2`. |
| `PRODUCTION_SHRINKAGE` | 0.7 | `playoff_elo.rs` | Bayesian shrinkage applied to `(season_points − avg)` in the production `seed_from_standings` path. Picked by the v1.13.0 sweep over 4 backfilled seasons (see §9). |
| `HOME_ICE_ADV` | 35.0 | `playoff_elo.rs:33` | League-wide home-ice Elo used inside the replay step. |
| `K_FACTOR` (Elo update) | 6.0 | `playoff_elo.rs:36` | Base Elo update rate per game, multiplied by `ln(goal_diff + 1)`. |
| `HOME_BONUS_DELTA_CLAMP` | 15.0 | `playoff_elo.rs` | Per-team home-ice Elo *delta* clamp around `HOME_ICE_ADV`. Output range for the absolute per-team bonus is `[HOME_ICE_ADV ± 15] = [20, 50]`. |
| Home-bonus scale factor | 400 | `playoff_elo.rs` | Maps `(home_pct − road_pct)` to Elo units. League-average pct-gap × 400 ≈ `HOME_ICE_ADV`, which is why the delta is centered by subtracting `HOME_ICE_ADV`. |
| `ALPHA` | 10.0 | `player_projection.rs:39` | RS-prior games-equivalent. |
| `BETA` | 4.0 | `player_projection.rs:41` | Historical prior games-equivalent. |
| `PO_TO_RECENT_WEIGHT` | 0.65 | `player_projection.rs:43` | Raw playoff PPG weight in `blended_po_ppg`. |
| `RECENT_HALF_LIFE_GAMES` | 4.0 | `player_projection.rs:45` | Recency decay half-life. |
| `RECENT_WINDOW` | 10 | `player_projection.rs:47` | Games included in the recency window. |
| `ABSENT_MULTIPLIER` | 0.3 | `player_projection.rs:50` | Availability mute for players absent from the playoffs. |
| `MIN_APPEARANCE_TEAM_GAMES` | 3 | `player_projection.rs:53` | Games played by team before applying the absent mute. |
| `HOME_ICE_ELO` | 35.0 | `race_sim.rs:219` | Raw home-ice Elo used on the forward sim (duplicate of `playoff_elo::HOME_ICE_ADV`). |
| `DEFAULT_TRIALS` | 5000 | `race_sim.rs:223` | Monte Carlo trial count. |
| `DEFAULT_K_FACTOR` | 0.010 | `race_sim.rs:235` | Logistic scale on the RS-points scale (pre-playoff path). |
| `DEFAULT_PPG` | 0.45 | `race_sim.rs:238` | Fallback PPG when RS rate is unknown. |
| `NB_DISPERSION` | 4.0 | `race_sim.rs:250` | `r` in the Gamma-Poisson point draw. Variance = `λ + λ²/r`. |
| `ELO_K_FACTOR` | `ln(10)/400 ≈ 0.00576` | `race_odds.rs:49`, `calibrate.rs:40` | Logistic scale when ratings are on the Elo scale. |
| `HOME_ICE_SCHEDULE` | `[T,T,F,F,T,F,T]` | `race_sim.rs:714` | NHL 2-2-1-1-1 home-ice pattern across games 1-7. |
| Per-game win prob clamp | `[0.05, 0.95]` | `race_sim.rs:756` | Prevents extreme rating gaps from sweeping every trial. |
| `SEASON_WEIGHT` | 0.7 | `team_ratings.rs:21` | Pre-playoff RS-points blend weight. |
| `RECENT_WEIGHT` | 0.3 | `team_ratings.rs:23` | Pre-playoff L10 blend weight. |

---

## 7. Admin endpoints

All routes registered in `backend/src/api/routes.rs` (lines 229-248). All require a JWT with `is_admin = true`. Frontend stores the token as JSON in `localStorage` under key `auth_session`.

### `GET /api/admin/cache/invalidate?scope=today|all|{date}`

- `scope=today`: invalidates rows with today's `hockey_date`.
- `scope=all`: nukes the whole `response_cache`.
- `scope={YYYY-MM-DD}`: invalidates that specific hockey-date.

Forces the next request to regenerate (race-odds, insights, pulse, etc.).

### `GET /api/admin/backfill-historical?start=YYYY-MM-DD&end=YYYY-MM-DD`

Calls `ingest_playoff_games_for_range(start, end)`. Iterates the schedule endpoint date-by-date, upserting completed playoff games into both `playoff_game_results` and `playoff_skater_game_stats`. **Flaky for historical seasons**: the `/schedule/{date}` response frequently drops the `series_status.round` field and occasionally drops Cup Finals / conference-finals rows entirely.

Kept because it populates skater-level stats (which the carousel path does not).

### `GET /api/admin/rebackfill-carousel?season=YYYYYYYY`

Calls `rebackfill_playoff_season_via_carousel(season)`. **This is the correct historical path for team-level data.** Walks `/v1/playoff-series/carousel/{season}` for the round structure and then `/v1/schedule/playoff-series/{season}/{letter}` (letter lowercased, season 8-digit) for every game in every series. Upserts into `playoff_game_results` only — skater-level stats are not touched.

Typical round-trip for a full season is ~15 series × 1 call = 15 carousel requests. Idempotent. Error-surfacing (since v1.12.1): a single series failure propagates as a 500 with the NHL-side message rather than being silently swallowed.

### `GET /api/admin/calibrate?season=YYYYYYYY`

Calls `infra::calibrate::calibrate_season(season)`. Measures predicted vs realized for a completed historical season. Returns `CalibrationReport` with aggregate `brier_r1 / brier_r2 / brier_r3 / brier_cup`, matching `log_loss_*`, and per-team detail (`predicted_advance_r1/_r2/_r3/_cup_win` alongside each team's realized outcome flags).

Requires `playoff_game_results` for that season to be populated (via `rebackfill-carousel`).

As of v1.12.3, Elo seeds come from `/v1/standings/{date}` for the day before the season's first playoff game (retrying up to 10 days back to work around the empty-array RS-to-playoffs gap), replacing the earlier live-standings fallback that contaminated historical runs with current-roster bias.

As of v1.13.0, the calibration run is deterministic (`simulate_with_seed` with a fixed RNG seed). Two runs with identical inputs produce identical Brier/log-loss — so the sweep endpoint can attribute every grid-cell delta to a knob change, not to Monte Carlo jitter.

### `GET /api/admin/calibrate-sweep?season=YYYYYYYY&points_scale=3,4,5&shrinkage=0.5,0.7,1.0&k_factor=...&home_ice_elo=...&trials=...`

Calls `infra::calibrate::calibrate_sweep(season, grid)` over the Cartesian product of the comma-separated lists. Each axis accepts 1–many values; omit or leave empty to pin that axis to the production default. Hard-capped at 200 cells.

Returns `SweepReport { season, grid_size, best: SweepRun, runs: Vec<SweepRun> }` with runs sorted by `brier_aggregate` ascending (sum of per-round Brier). Each run records the knobs plus `brier_r1/r2/r3/cup`, `log_loss_r1/cup`, and the aggregate.

**One-off tool.** You run it a handful of times, pick the winning knobs, hard-code them as production constants in `playoff_elo.rs` + `race_sim.rs`, and ship. Not meant to be called from the request path: a 200-cell sweep can spend minutes of CPU and is not cached.

The winning knobs are not yet baked into production — `CalibrationKnobs::default()` still reproduces v1.12.3 behavior (`POINTS_SCALE = 6.0`, `shrinkage = 1.0`). First-pass sweep results should populate §9 of this doc once run.

### `GET /api/admin/process-rankings/{date}`

Daily-ranking processing for the given date. Unrelated to the Monte Carlo directly but shares the same cache surface (`daily_rankings` feeds the Pulse sparkline which has a `playoff_start()` floor to avoid showing RS points on day 1 of the playoffs).

---

## 8. NHL API endpoints used

Base URL: `https://api-web.nhle.com` (undocumented; see `backend/src/nhl_api/nhl_constants.rs`).

| Endpoint | Purpose | Notes |
|---|---|---|
| `/v1/standings/now` | Live standings (teams, points, home/road splits). | Used by the race-odds handler for current-season Elo seeds and home-ice bonuses. |
| `/v1/standings/{date}` | Historical standings snapshot. | `date` is `YYYY-MM-DD`. Returns an empty `standings` array for dates in the gap between regular-season finale and playoff game 1 — `infra::calibrate::fetch_historical_standings` walks back up to 10 days to work around this. |
| `/v1/playoff-series/carousel/{season}` | Round structure + `seriesLetter` per matchup for a given season. | `season` is 8-digit (`YYYYYYYY`). Primary source for building `BracketState` on live requests. |
| `/v1/schedule/playoff-series/{season}/{letter}` | All games in one playoff series. | `season` is 8-digit, `letter` is lowercased (carousel returns uppercase). The 4-digit year returns 404 — v1.12.2 fix. This is the reliable historical path. |
| `/v1/gamecenter/{game_id}/boxscore` | Per-game skater stats. | Read by `ingest_single_game` to populate `playoff_skater_game_stats`. |
| `/v1/schedule/{date}` | Date-indexed schedule. | Flaky for historical dates — `series_status.round` is often missing for past seasons, and some Cup Finals / conference-finals games are dropped entirely. Used for the live daily ingest where these issues don't apply. |
| `/v1/skater-stats-leaders/{season}/{game_type}` | Skater leaderboard. | Used to source RS points for the Bayesian blend. |
| `/v1/goalie-stats-leaders/{season}/{game_type}` | Goalie leaderboard. | **Not currently consumed.** Available but unused — this is the goalie-data gap noted in §10. |

Client lives in `backend/src/nhl_api/nhl.rs` (`NhlClient`). Built-in rate limiting (5-concurrent semaphore at the HTTP boundary) and per-endpoint caching so duplicate fetches within a request lifecycle reuse the same response.

---

## 9. Calibration results so far

From runs of `GET /api/admin/calibrate?season=YYYYYYYY` against each backfilled season, **using the live-standings seed** (pre-v1.12.3 Elo seeding):

| Season | brierR1 | brierR2 | brierR3 | brierCup | logLossR1 | logLossCup |
|---|---|---|---|---|---|---|
| 2021-22 | 0.361 | 0.197 | 0.062 | 0.025 | 1.02 | 0.092 |
| 2022-23 | 0.351 | 0.223 | 0.173 | 0.080 | 1.13 | 0.374 |
| 2023-24 | 0.302 | 0.285 | 0.176 | 0.083 | 0.85 | 0.567 |
| 2024-25 | 0.396 | 0.247 | 0.147 | 0.074 | 1.02 | 0.490 |

Baselines (always-0.5 and base-rate references):
- R1: 0.25
- R2: 0.19
- R3: 0.11
- Cup: 0.059

**R1 Brier is worse than coinflip across every backfilled season.** R2 is roughly at baseline, R3 and Cup are better than base-rate (largely because the base rates themselves are very low for late-round outcomes).

These numbers are **biased by current-season Elo seeds**. v1.12.3 (just shipped) fixes historical-standings seeding via `/v1/standings/{date}` with a 10-day walk-back. Re-running after deploy should produce more honest numbers; the old table is kept here as the pre-fix baseline for comparison.

---

## 10. Known gaps / tuning targets

Ordered roughly by expected impact on the R1 / Cup-finals Brier.

### `POINTS_SCALE = 6` is likely too aggressive

Public Elo trackers use scales that produce a roughly ~200-point total spread across the NHL. The current `POINTS_SCALE = 6` produces a ~400-point spread for a typical 70-point RS range, which compounds through the sim into per-game probabilities that are too concentrated on the higher-rated team.

Symptom: on 2026-04-17 the model showed COL at 43% to win the Cup while MoneyPuck had them at ~13%. Back-of-envelope: COL's per-game win probability against LAK in R1 was ~0.745 (186-Elo gap through `ELO_K_FACTOR = 0.00576`) vs an industry-consensus ~0.59. That gap compounds to ~30 percentage points by the Cup.

### No Bayesian shrinkage on ratings

`seed_from_standings` uses the raw `(season_points − avg)` without regression to the mean. A team that ran hot for an 82-game sample gets full credit as if their true talent equals their observed results. Most public models apply a shrinkage factor in the 0.5–0.8 range.

### No goalie quality signal

MoneyPuck weights goaltending at ~29% of team strength. The current model weights it at 0%. The NHL API exposes `/v1/goalie-stats-leaders` for season-level data (SV%, GAA, etc.) but it is not currently consumed. A goalie signal would plug directly into `TeamRating` as a third component alongside `base` and `home_bonus`, scaled to a bounded Elo range.

### L10 blend in `team_ratings.rs` is near-dead during playoffs

`team_ratings::from_standings` reads `l10Wins / l10Losses / l10OtLosses` from the standings feed. During the playoffs the standings endpoint freezes L10 at its final regular-season value, so the blend collapses toward plain RS points. Not a bug (double-counting playoff form here would over-weight a small sample), but worth noting that the blend is effectively a pure-RS-points rating for the duration of the playoffs.

### `k_factor` is the same across all rounds

A higher `k_factor` might fit early rounds (where strength gaps are larger) and a lower one late rounds (where the field has been filtered to comparable teams). Currently a single scalar.

### No injury / scratch awareness

Aside from the availability multiplier in `player_projection` (0.3x PPG for players absent from all playoff games after ≥3 team games played), the model has no visibility into game-day scratches or mid-series injuries. Team ratings are not adjusted when a top skater is ruled out.

### Poisson → NegBin improvement done but dispersion not empirically tuned

`NB_DISPERSION = 4.0` was chosen to produce variance ≈ 1.25·λ at λ=1 and ≈ 2.5·λ at λ=5, roughly matching empirical per-game point spread for top skaters. A proper fit against historical point-per-game variance has not been run.

### Historical standings lookup can return empty during RS-to-playoffs gap

As of v1.12.3, `fetch_historical_standings` walks back up to 10 days from the season's first playoff game looking for a non-empty response. If every attempt fails the fallback is live standings (with a warn log), which reintroduces the current-roster bias for that season's calibration.

### The sim seeds once and updates only from completed playoff games

Mid-round external signals (injury news, goalie starts, line changes) do not feed back into `TeamRating.base`. The only update channel is chronological replay of `playoff_game_results`, which by definition lags the information environment.

---

## 11. Concrete experiments to try

Ordered by expected bang/buck.

1. **Drop `POINTS_SCALE` from 6 → 3**. Tightens the Elo spread to roughly ±75 at a 25-point RS separation, bringing it in line with public Elo trackers. Likely the biggest single fix for R1 Brier.
2. **Add Bayesian shrinkage on standings**. Multiply `(season_points − avg)` by a shrinkage factor `s ∈ [0.5, 0.8]` before scaling. Combine with the `POINTS_SCALE` change rather than tuning in isolation.
3. **Add goalie bonus to `TeamRating`**. New `goalie_bonus: f32` field on `TeamRating` (Elo units). Derive from the starting goalie's `SV% − league_avg_SVP` via `/v1/goalie-stats-leaders`; scale to a ±30 Elo range. Sum into the pre-sigmoid gap alongside `base` and `home_bonus`.
4. **Grid-search over**:
   - `POINTS_SCALE ∈ {3, 4, 5, 6}`
   - `ELO_K_FACTOR ∈ default × {0.5, 1.0, 2.0}`
   - Standings shrinkage ∈ {0.5, 0.7, 1.0}
   - `NB_DISPERSION ∈ {2, 4, 8}`

   Score against aggregate Brier + log-loss across all backfilled seasons post-v1.12.3. Target: R1 Brier ≤ 0.22.
5. **Injury detection via player game logs**. Already partly implemented in `player_projection` (availability multiplier). A team-level extension could derate `TeamRating.base` when a top-line skater is missing from the last 2 team games — threshold and magnitude to be tuned.

---

## 12. Recent release history

Condensed arc from v1.7.0 through v1.12.3 (see `CHANGELOG.md` for full entries):

- **v1.7.0** — Monte Carlo engine (`race_sim`) introduced. 5000 trials, per-game logistic on RS-points rating with `k = 0.010`. `/api/race-odds` endpoint. Pulse/Insights split (Pulse = personal/league, Insights = NHL-generic). League Race Table, Rivalry card, My Stakes, NHL Cup Odds.
- **v1.7.1** — Collapsed rostered-by chips on Insights.
- **v1.7.2** — You-only ownership pills on Insights.
- **v1.7.3** — Full-league dashboard, mobile Teams link, season-badge dedupe.
- **v1.7.4** — Pulse reordering, Tonight section, "Last day" wording for `points_today`.
- **v1.8.0** — Race-odds rework. `SeriesState`/`BracketState` for full-bracket correctness; `playoff_game_results` + `playoff_skater_game_stats` + `historical_playoff_skater_totals` tables; dynamic playoff Elo with `POINTS_SCALE = 6`, `K = 6`, `HOME_ICE_ADV = 35`; Bayesian player projection; Negative-Binomial sampling; fractional tie-splitting; `race_odds:v2` cache key; forward home-ice via 2-2-1-1-1 schedule. Test suite 11 → 53.
- **v1.8.1** — Auto-apply migrations at startup via `sqlx::migrate!`; Tonight player-row layout fix.
- **v1.9.0** — Prediction engine isolated under `backend/src/domain/prediction/` as pure domain; DB wrappers moved to `backend/src/infra/prediction.rs`. `PREDICTION_SERVICE.md` plan doc added. Games-page latency fixes (parallel box-score pre-load); iOS team-name clipping fix.
- **v1.11.1** — `/api/admin/calibrate` endpoint (P4.2 MVP); per-team home-ice advantage (`home_bonus_from_standings`); `/api/admin/backfill-historical` endpoint; Insights narrative self-heal on empty schedule; dashboard quick-link buttons; `reconstruct_bracket_from_results` stops trusting the `round` column (topology-only).
- **v1.12.0 / v1.12.1** — `/api/admin/rebackfill-carousel` endpoint (carousel-driven, reliable for historical); Pulse "No games scheduled today" self-heal; rebackfill error-surfacing (no more silent no-ops).
- **v1.12.3** — Historical standings seeding in calibration (`/v1/standings/{date}` with 10-day walk-back); rebackfill URL format fix (8-digit season required, 4-digit returns 404).

Test coverage: 54+ passing in the backend lib suite (post-v1.11.1). Core coverage under `domain/prediction/`:
- 6 playoff-Elo (seeding, upset payoff, blowout scaling, zero-sum, home-ice).
- 6 player-projection (cold start, heavy sample override, historical anchor, recency, absent multiplier, empty input).
- 9 backtest helpers (Brier, log-loss, calibration, MAE/RMSE, interval coverage, bracket reconstruction).
- 3 bracket-state correctness.
- 3 carousel-to-BracketState classification.
- 2 distribution tests (Gamma mean/variance, NegBin variance exceeds Poisson).
- 1 tie-splitting; 1 home-ice advancement.

---

## 13. Reference — Bulletproof Rust Web

The codebase aligns with the layered shape from [bulletproof-rust-web](https://github.com/gruberb/bulletproof-rust-web):

- **`domain/`** — pure business logic, no framework dependencies.
- **`infra/`** — adapters (DB, HTTP client, external services).
- **`api/`** — HTTP surface (handlers, DTOs, routes).

`domain/prediction/` has zero framework dependencies today (no `sqlx`, no `axum`, no `reqwest`, `tracing` only in debug paths). It is a candidate for extraction into a standalone crate or HTTP service — see `PREDICTION_SERVICE.md` at the repo root for a phased migration plan covering a workspace-crate option, a standalone HTTP service (`/simulate` JSON contract), and a gRPC variant.
