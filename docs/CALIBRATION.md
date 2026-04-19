# Calibrating the Prediction Engine

How to measure whether the race-odds / Cup-odds model is miscalibrated, how to find better knobs, and how to promote them to production. This doc is the operator's guide — for the *architecture* of the engine, read `PREDICTION_MODEL.md`.

---

## 1. Why calibrate

The engine is a Monte Carlo bracket simulator with a small number of tunable hyperparameters (Elo `POINTS_SCALE`, standings `shrinkage`, logistic `k_factor`, league-wide `HOME_ICE_ELO`, trial count). Those numbers were seeded at sensible defaults in v1.7 and iterated on twice since. They are not proven optimal — they are proven *workable*.

The fact that you can get a sane bracket out does not mean the win probabilities are sharp. A model that says `COL 43%` to win the Cup when public consensus is `~13%` is not "confident" — it is miscalibrated, and downstream everything that uses it (Stanley Cup Odds table, Pulse My Stakes, rivalry card, insights narratives) inherits the bias.

The symptom that actually matters is the **Brier score** — mean squared error between predicted probability and realised outcome. Low is good. Baselines:

| Round | Always-0.5 Brier | Base-rate Brier | v1.12.3 Brier (measured) |
|---|---|---|---|
| R1 | 0.25 | 0.25 | 0.30 – 0.40 |
| R2 | 0.19 | 0.19 | 0.20 – 0.29 |
| R3 | 0.11 | 0.11 | 0.06 – 0.18 |
| Cup | 0.059 | 0.059 | 0.03 – 0.08 |

R1 is worse than a coin flip across every backfilled season. That is the headline problem. Later rounds look better because base rates are tiny (only 8 of 16 teams advance past R1, only 1 of 16 wins the Cup) — a model that confidently says "0%" for most teams automatically looks good on Cup prediction. This is why you want Brier *and* log-loss, and why the sweep scores across all four rounds.

Calibration is the loop: measure Brier → find knobs that lower Brier → promote them → ship → measure again.

---

## 2. Prerequisites

Before the endpoints work you need:

**1. Admin JWT.** Log into the Fantasy Puck frontend as an admin user. Open devtools → Application → Local Storage → `auth_session`. Copy the `token` field. Export it:

```bash
export FP_TOKEN="eyJhbGci..."
export FP_BASE="https://your-fly-app.fly.dev"   # or http://localhost:3000
```

**2. Historical data backfilled.** The sweep measures predicted vs realised outcomes for past seasons, so those seasons' games must be in `playoff_game_results`. Check:

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/calibrate?season=20212022"
```

If you get `"No playoff_game_results rows for season..."`, backfill it:

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/rebackfill-carousel?season=20212022"
```

Repeat for `20222023`, `20232024`, `20242025`. Idempotent — safe to re-run.

**3. Enough CPU headroom.** The sweep is sequential (each cell runs `DEFAULT_TRIALS = 5000` sims + the setup cost). A 16-cell sweep across 4 seasons takes 2-5 minutes depending on the Fly machine size. A 200-cell sweep is a multi-minute lockup — don't run it during league peak hours.

---

## 3. What you're tuning

Each knob and the range that actually makes sense to sweep:

### `points_scale`

Elo points per RS-standings-point of separation from league average. Production default is `6.0` — a 70-point RS spread produces a ~420-Elo window, which is wide. Public NHL Elo trackers use scales that produce a ~200-point total spread. Lower values flatten the field (more upsets); higher values concentrate probability on top seeds (more chalk).

**Sweep candidates:** `{2, 3, 4, 5, 6}`. The v1.12.3 diagnostic note specifically flags this as likely too aggressive; expect the winner to land at 3 or 4.

### `shrinkage`

Multiplicative regression toward the mean applied to `(season_points − league_avg)` *before* `points_scale` scales it. `1.0` = no shrinkage (legacy behaviour). `0.7` = "treat the standings as 70% signal, 30% noise" — the Bayesian prior for an 82-game NHL sample. `0.0` flattens every team to `BASE_ELO` (good sanity check — the model should then produce near-uniform win probabilities).

**Sweep candidates:** `{0.5, 0.7, 1.0}`. Combining `points_scale = 3` with `shrinkage = 0.7` is mathematically similar to `points_scale = 2.1` with `shrinkage = 1.0`, but the decomposition matters for clarity in downstream UI explanations.

### `k_factor`

Logistic scale in the per-game probability draw. Production is `ELO_K_FACTOR = ln(10) / 400 ≈ 0.00576` (the Elo identity). Doubling it makes the model more responsive to rating gaps; halving it flattens win probabilities toward 50/50.

**Sweep candidates:** `{0.00288, 0.00576, 0.01152}` — default × {0.5, 1.0, 2.0}.

### `home_ice_elo`

League-wide home-ice bonus in raw Elo. Production is `35.0` (matches the ~54/46 league-average home/road split). Unlikely to move materially — this is a well-measured parameter in hockey analytics. Don't sweep this unless you're debugging.

**Sweep candidates:** `{25, 35, 45}` only if you're genuinely curious. Most sweeps should omit this axis.

### `trials`

Monte Carlo trial count. `5000` is the production default. `DEFAULT_TRIALS` produces ±~1pp 95% CI on win probabilities. Higher counts narrow confidence but cost linear time; lower counts speed up the sweep but let Monte Carlo noise leak into your knob comparisons.

**Sweep candidates:** don't sweep. The calibration path uses `simulate_with_seed` with a fixed RNG seed, so every cell uses the *same* random draws — comparisons are deterministic regardless of trial count. Leave `trials` unset (defaults to 5000).

---

## 4. The workflow

### Step 1 — baseline

Measure today's model against one season with default knobs:

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/calibrate?season=20232024" | jq '.data | {brier_r1, brier_r2, brier_r3, brier_cup, log_loss_r1, log_loss_cup}'
```

Write down these numbers. They are what you must beat.

### Step 2 — sanity sweep

Start with a tiny 2×2×2 grid (8 cells, ~1 min) to confirm the endpoint works and to eyeball whether small perturbations move the needle:

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/calibrate-sweep?season=20232024&points_scale=3,6&shrinkage=0.7,1.0&k_factor=0.00576,0.01152" \
  | jq '.data | {grid_size, best: .best}'
```

The response shape is:

```json
{
  "gridSize": 8,
  "best": {
    "knobs": { "pointsScale": 3.0, "shrinkage": 0.7, "kFactor": 0.00576, "homeIceElo": 35.0, "trials": 5000 },
    "brierR1": 0.22,
    "brierR2": 0.18,
    "brierR3": 0.11,
    "brierCup": 0.05,
    "logLossR1": 0.72,
    "logLossCup": 0.27,
    "brierAggregate": 0.56
  },
  "runs": [ { "knobs": ..., "brierAggregate": ... }, ... ]
}
```

`runs` is sorted ascending by `brierAggregate` — top entry equals `best`. The gap between the best and worst cells tells you how much the knobs actually matter. If the spread is small (< 0.02 on aggregate), your knobs are over-tuned already and a larger sweep is wasted work.

### Step 3 — full sweep

Run the real grid once you're confident the small sweep is informative. A 45-cell grid (`5 × 3 × 3`):

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/calibrate-sweep?season=20232024&points_scale=2,3,4,5,6&shrinkage=0.5,0.7,1.0&k_factor=0.00288,0.00576,0.01152" \
  | jq '.data.runs[0:5]'
```

45 cells × 4-5 seconds each ≈ 3-4 minutes. The top 5 runs should look similar — if rank 1 and rank 5 are miles apart on one knob and identical on the others, that's your signal for which knob matters.

### Step 4 — cross-season check (the one people skip)

**This is where single-season tuning dies.** The winning knobs on 2023-24 may be terrible on 2021-22. Run the same grid against every backfilled season:

```bash
for s in 20212022 20222023 20232024 20242025; do
  echo "=== Season $s ==="
  curl -s -H "Authorization: Bearer $FP_TOKEN" \
    "$FP_BASE/api/admin/calibrate-sweep?season=$s&points_scale=2,3,4,5,6&shrinkage=0.5,0.7,1.0&k_factor=0.00288,0.00576,0.01152" \
    | jq '.data | {best: .best.knobs, brierAggregate: .best.brierAggregate}'
done
```

You want knobs that land in the top-5 on every season. A knob that wins season A and finishes 20th on season B is overfitting — pick something worse-but-consistent. In practice: find the subset of cells that are in the top quartile for all four seasons, take the one with the lowest *average* `brierAggregate` across the four.

### Step 5 — promote to production

Once you have a winner, bake it. The knobs live in two files:

- **`backend/src/domain/prediction/playoff_elo.rs`** — `POINTS_SCALE` (constant).
- **`backend/src/api/handlers/race_odds.rs`** — `ELO_K_FACTOR` (constant).
- **Shrinkage** is not yet a production constant. If the sweep picks `shrinkage != 1.0`, thread it: in `resolve_ratings`, swap `compute_current_elo(db, standings, season_val)` — which internally seeds via `seed_from_standings` — for a new variant that calls `seed_from_standings_tuned(standings, POINTS_SCALE, PRODUCTION_SHRINKAGE)`. Add `PRODUCTION_SHRINKAGE` as a new `pub const` in `playoff_elo.rs`.

Edit, then:

1. Bump `backend/Cargo.toml` version (e.g. `1.15.0 → 1.16.0`).
2. Add a changelog entry describing the new knobs and the measured Brier improvement.
3. Bump the response-cache key from `race_odds:v2` → `race_odds:v3` in `api/handlers/race_odds.rs` so cached pre-change payloads don't leak through.
4. Tag: `git tag -a v1.16.0 -m "calibrated knobs"`.
5. Push. Fly auto-deploys.

### Step 6 — verify

Re-run the single-season calibrate (step 1) against the production model. The numbers should match the sweep's winning cell within Monte Carlo noise (~0.5pp on Brier). If they don't, something in the promotion step was wrong — probably the cache wasn't invalidated or a knob wasn't actually threaded through.

---

## 5. How to read a sweep response

### `brierAggregate`

Sum of per-round Brier. This is the primary ranking metric. Minimising it means the model is well-calibrated *across all rounds*, not just the one your knob accidentally tuned for. A configuration that wins R1 Brier by 0.05 and loses Cup Brier by 0.02 still has a better aggregate than one that tied R1 and lost Cup by 0.05.

### Per-round Brier

Look for **R1 specifically** — that's the worst round in the baseline and the round where most knob changes show up first (fewer simulation steps, so less compounding). If R1 Brier doesn't drop meaningfully, your sweep isn't finding signal.

### Log-loss

Same ordering as Brier, but log-loss penalises confident-but-wrong predictions harder. A model that says `0.99` and is wrong contributes log(1/0.99) ≈ ∞ to log-loss, while Brier contributes only 1.0. Useful sanity check: if sweep winners according to Brier and log-loss disagree, you probably have a miscalibrated-but-not-over-confident model (Brier lower, log-loss worse) or vice-versa.

### `teamsEvaluated`

Should be 16 for every season. If not, your historical bracket reconstruction is missing teams and the Brier numbers are suspect — rebackfill the season via `/api/admin/rebackfill-carousel`.

---

## 6. Pitfalls

**Don't sweep a live season.** The calibrate endpoints require `playoff_game_results` rows, and for an in-progress season those only cover completed games. The bracket reconstruction will produce partial realised outcomes. Results are technically meaningful but noisy — stick to completed past seasons for tuning.

**Don't sweep more than ~100 cells on a single Fly machine.** The handler runs sequentially to avoid stampeding the NHL API. 200 cells × 5 seconds = 17 minutes of one CPU pinned at 100%. The 200-cell cap in the code is a guardrail, not a recommendation.

**Don't cherry-pick single-season winners.** See Step 4. A sweep that only evaluates 2023-24 is optimising for 2023-24, not for "next year's playoffs". Cross-season consistency is the real signal.

**Don't forget to invalidate cache after promotion.** `race_odds:v2` entries from before the deploy will persist until their TTL expires. Either bump the cache key (preferred) or call:

```bash
curl -H "Authorization: Bearer $FP_TOKEN" \
  "$FP_BASE/api/admin/cache/invalidate?scope=all"
```

**Don't skip log-loss.** If a sweep shows Brier improving but log-loss worsening, the model got better at predicting roughly-right probabilities but worse at the confident predictions — that's usually not a win for user-facing "Cup Odds" where the headline numbers are the extremes.

---

## 7. When to recalibrate

- **After new historical data is backfilled** — e.g. when the next completed season (2025-26) gets added to the training set. Re-run the sweep with that season included in the cross-season check.
- **After a model structural change** — if you add a new signal to `TeamRating` (like the v1.15 goalie component), the optimal `points_scale` and `k_factor` likely shift. Sweep before and after to quantify the lift.
- **Not on every deploy.** The knobs are stable between schema changes — there is no value in running a sweep after a frontend-only release or a bug fix that doesn't touch the sim.
- **Never on startup.** The training data is static between deploys; a sweep at boot would compute the same answer every time and delay Fly readiness checks by minutes.

---

## 8. Current state (v1.15.0)

- `POINTS_SCALE = 6.0` — almost certainly too aggressive, first sweep target.
- `shrinkage = 1.0` (implicit; no production constant yet) — expect the sweep to want ~0.7.
- `ELO_K_FACTOR ≈ 0.00576` — may be fine, may want doubling. Unclear without data.
- `HOME_ICE_ELO = 35.0` — leave alone unless you're debugging.
- Goalie component: active since v1.15.0, contributes up to ±30 Elo per team, shrinks with round depth.
- Round-depth mean reversion: active since v1.15.0, `1.00 / 0.85 / 0.70 / 0.55` from round 0 to Cup Final.

The v1.13.0 sweep harness exists but the v1.15.0 constants have not been swept yet. That's the next piece of work — and is exactly what this document is for.
