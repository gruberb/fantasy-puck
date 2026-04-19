# Prediction Service — extraction plan

Concept doc. Not a commitment. Written 2026-04-18 as a follow-up to the
v1.8.0 race-odds rework, after a code refactor isolated the pure
prediction engine into `backend/src/domain/prediction/`.

## TL;DR

The Monte Carlo engine that powers race-odds today is a self-contained
piece of math. It takes a bracket state, team ratings, and per-player
projections, and returns win probabilities + expected games + point
distributions. The fantasy-hockey app currently imports it as a Rust
module; the same logic could back a prediction-market product, a
second sport, or a backtesting service. This doc lays out how to
extract it cleanly when that time comes.

## 1. What exists today

`backend/src/domain/prediction/` — pure, no DB, no HTTP:
- `race_sim.rs` — the Monte Carlo itself. Walks a `BracketState`
  (every slot tagged `Future | InProgress | Completed`), resolves each
  slot per trial, aggregates win probabilities and player point
  distributions. Knobs: `DEFAULT_TRIALS`, `k_factor`, `home_ice_bonus`,
  `NB_DISPERSION`.
- `team_ratings.rs` — regular-season blended strength
  (`0.7·season_pts + 0.3·L10·82`). Used pre-playoffs.
- `playoff_elo.rs` — dynamic playoff Elo: seed from standings + replay
  completed games with per-game home-ice and blowout scaling.
- `player_projection.rs` — Bayesian blend
  `(α·rs_ppg + po_gp·blended + β·hist_ppg) / (α + po_gp + β)`
  with recency weighting and availability multiplier.
- `series_projection.rs` — `SeriesStateCode` enum + classify helper
  (mostly UI leverage; optional for a market service).
- `backtest.rs` — Brier, log-loss, calibration curve, MAE, RMSE,
  interval coverage, bracket reconstruction.

`backend/src/infra/prediction.rs` — the DB-backed wrappers. Thin:
- `compute_current_elo(db, standings, season)` — queries
  `playoff_game_results`, folds rows through `apply_game`.
- `project_players(db, season, players, team_games)` — queries
  `playoff_skater_game_stats` and `historical_playoff_skater_totals`,
  folds rows through `project_one`.

**This split is the whole point**: `domain/prediction/` has zero
framework deps (`sqlx`, `axum`, `reqwest` aren't in its tree). Anyone
can lift that subdirectory into another binary, feed it data, and get
predictions back.

## 2. Why extract it

Three reasons, in order of likelihood to matter:

1. **Reuse across products.** A prediction-market frontend could use
   the same engine to price NHL-series markets. The bracket and
   per-player math is sport-general in form; swap in football / NBA
   data and you get a different set of markets.
2. **Independent deploy + scale.** The engine is CPU-bound (tens of
   ms × MC trials). Running it as its own service lets you scale
   sim-heavy workloads independently from the CRUD-heavy fantasy
   backend. Could even move to a different region, a bigger machine
   class, or autoscale off queue depth.
3. **Experiment isolation.** Calibration passes (tune `k_factor`,
   `α`, `NB_DISPERSION`, home-ice) are friendlier when the
   "calibrator" is its own artifact. Version the service; run
   A/B against old vs new without touching the fantasy app.

**Non-goal**: the extraction shouldn't fracture the fantasy app's
dev loop. Local dev must stay a single `make run`.

## 3. Architecture options

### Option A — Workspace crate (recommended first step)

```
fantasy-puck/
  Cargo.toml                    (workspace root)
  crates/
    prediction/                 (new crate, extracted from domain/prediction/)
      Cargo.toml                (no sqlx, no axum, no reqwest)
      src/
        lib.rs                  (re-exports race_sim, playoff_elo, …)
        race_sim.rs
        playoff_elo.rs
        player_projection.rs
        team_ratings.rs
        backtest.rs
  backend/                      (existing app)
    Cargo.toml                  → prediction = { path = "../crates/prediction" }
    src/
      domain/                   (shrinks — prediction lives in its own crate)
      infra/prediction.rs       (DB wrappers, now calls into the crate)
      api/handlers/...
```

**Pros**: single compile, single CI, single repo. Just a workspace
move. Extract takes ~1 afternoon.

**Cons**: still deploys as one binary. No independent scaling.

**When to pick**: if #2 in the motivation list ever becomes real
without waiting for it to become a problem.

### Option B — Standalone HTTP service

```
prediction-service/             (new repo or workspace crate with own bin)
  src/
    main.rs                     (axum, one handler per endpoint)
    lib.rs                      (re-export engine)
    engine/                     (copy of the pure modules)
    dto.rs                      (request/response shapes)
  Cargo.toml
  fly.toml                      (own Fly app, own region mapping)
```

Endpoints:
- `POST /simulate` — body: `RaceSimInput` JSON. Response: `RaceSimOutput`.
- `POST /backtest` — body: predictions + realized outcomes. Response: metrics.
- `GET  /version` — semver of engine + `MODEL_VERSION` constant.

**Pros**: independent deploy, scale, region. Natural surface for a
prediction-market frontend to consume. Easy to language-bridge (a
Python market-maker can just POST JSON).

**Cons**: network hop adds latency (negligible for 5000-trial sim;
noticeable for chatty use). Two repos to keep in sync. Data still has
to come from somewhere — either the caller ships a complete
`RaceSimInput` (simple) or the service reaches back into the fantasy
DB (tight coupling, defeats the point).

**When to pick**: when a second consumer exists that isn't the fantasy app.

### Option C — gRPC service

Same shape as B but `tonic` + protobuf instead of JSON. Schema-first,
generated clients. Overkill for a two-endpoint surface; revisit if
the engine grows.

## 4. Data contract — `/simulate`

The engine already has a well-defined input/output shape. For an
HTTP version, serialize both with `serde`:

### Request

```json
{
  "bracket": {
    "rounds": [
      [
        { "type": "completed", "winner": "BOS", "loser": "BUF", "total_games": 5 },
        { "type": "in_progress", "top_team": "TBL", "top_wins": 2, "bottom_team": "MTL", "bottom_wins": 1 },
        { "type": "future" },
        ...
      ],
      [ ... ], [ ... ], [ ... ]
    ]
  },
  "ratings": { "BOS": 1612.4, "BUF": 1478.9, ... },
  "k_factor": 0.00576,
  "home_ice_bonus": 0.2,
  "fantasy_teams": [
    {
      "team_id": 1,
      "team_name": "Sticky Pucks",
      "players": [
        { "nhl_id": 8478402, "name": "Connor McDavid", "nhl_team": "EDM",
          "position": "C", "playoff_points_so_far": 0, "ppg": 1.2 },
        ...
      ]
    }
  ],
  "trials": 5000
}
```

### Response

```json
{
  "trials": 5000,
  "teams": [
    {
      "team_id": 1, "team_name": "Sticky Pucks",
      "current_points": 0, "projected_final_mean": 84.2,
      "projected_final_median": 83.0, "p10": 62.0, "p90": 108.5,
      "win_prob": 0.14, "top3_prob": 0.38,
      "head_to_head": { "2": 0.61, "3": 0.72, ... }
    }
  ],
  "players": [ ... ],
  "nhl_teams": [
    {
      "abbrev": "BOS",
      "advance_round1_prob": 1.0,
      "conference_finals_prob": 0.58,
      "cup_finals_prob": 0.31,
      "cup_win_prob": 0.17,
      "expected_games": 14.3
    }
  ]
}
```

Both types already exist as `serde` structs in `domain/prediction/race_sim.rs`.
An HTTP wrapper is ~30 lines of axum per endpoint.

## 5. Migration path

### Phase 0 — done (this PR)

- `domain/prediction/` contains the pure engine. Zero framework deps.
- `infra/prediction.rs` holds the DB wrappers.
- Fantasy app still imports from Rust modules directly.

### Phase 1 — workspace crate

- Create `crates/prediction/` in the repo root.
- Move the contents of `backend/src/domain/prediction/` into it.
- Update `backend/Cargo.toml` to depend on `prediction = { path = "../crates/prediction" }`.
- Rename imports: `crate::domain::prediction::race_sim::...` →
  `prediction::race_sim::...`.
- Tests move with the code; nothing else changes.

**Exit criteria**: `cargo test --workspace` green; fantasy app deploys
identically.

### Phase 2 — standalone binary (optional)

- Add a `[[bin]]` entry to the `prediction` crate that starts an
  axum server with `/simulate` and `/backtest`.
- Define the DTO layer (`prediction_service::dto`).
- Add a second `fly.toml` alongside `backend/`'s.
- Keep the library mode intact — the fantasy app still uses it in-process.

**Exit criteria**: `curl localhost:PORT/simulate -d @sample.json`
returns the same output the in-process call produces.

### Phase 3 — client / service split (optional, only if reuse materializes)

- Add a `prediction_client` crate: thin `reqwest` wrapper over the
  HTTP API.
- Switch the fantasy backend's `infra/prediction.rs` to either:
  - **Option A**: keep calling the library in-process (fast path).
  - **Option B**: call the HTTP service (decoupled, cacheable via
    ETag or Redis at the client boundary).
- Pick per-environment: local dev uses Option A, prod uses B.

## 6. Deployment + runtime

- **Binary**: add `[[bin]] name = "prediction-service"` with its own
  entry point. Compile once, ship two binaries from the same crate.
- **Hosting**: own Fly app; one machine is enough to start. Scale by
  concurrency request count (each sim is ~10-50ms, mostly CPU).
- **Observability**: emit per-request trial count, sim duration,
  input size. Histogram of `projected_final_mean` to spot regressions.
- **Versioning**: expose `MODEL_VERSION` constant; bump on any change
  to the math. Clients cache by `(MODEL_VERSION, bracket_hash)`.
- **Determinism**: engine supports `simulate_with_seed`. Expose a
  `seed` query param for reproducible results — useful for
  debugging and contract tests.
- **No state**: the service is pure-function in its HTTP form. No DB,
  no cache, no auth beyond a shared-secret header. Makes horizontal
  scaling trivial.

## 7. Open questions

1. **Where does the data live?** If the prediction service is
   stateless, the *caller* must assemble a `RaceSimInput` (ratings,
   player projections, bracket). That pushes complexity to the
   client. Alternative: the service reads from a shared DB — but then
   it's coupled to the fantasy app's schema. Fantasy-market
   consumers would maintain their own data pipeline.
2. **What about ratings/projection updaters?** Today those are
   DB-backed functions in `infra/prediction.rs` that fantasy app
   invokes at race-odds-request time. They're convenient but not
   part of the pure engine. If extracted:
   - Keep updaters in the fantasy app, feed pre-computed ratings into
     the service. (Cleanest, closest to "pure function" ethos.)
   - Or expose `/update-ratings` + `/update-projections` endpoints
     too. (Muddies the contract.)
3. **Calibration workflow.** Backtest harness is pure and already
   lives in the engine. An extracted service could expose
   `/calibrate` that takes historical data and returns tuned
   hyperparameters. Requires history to be fed in — no stored state.
4. **Auth.** A public prediction-markets consumer needs
   authentication; the fantasy app's in-process use doesn't. Shared
   secret header for service-to-service; JWT for end-user callers.
5. **Rate limiting.** At 5000 trials × tens of ms, 20-50 rps per
   machine before CPU saturation. Beyond that, cache by
   request-hash or split trials across machines.

## 8. Rough effort estimate

- Phase 1 (workspace crate): **~2-4 hours**. Mechanical move +
  import rewrite + tests pass.
- Phase 2 (standalone HTTP): **~1-2 days**. Axum scaffolding, DTO
  layer, fly.toml, initial deploy. Most of the work is plumbing.
- Phase 3 (client/service split): **~2-3 days**, mostly because of
  the Option A/B branching and coordinated rollout.

None of this is a must-do today. The Phase 0 boundary is sufficient
to take this work on at any point without retrofitting.
