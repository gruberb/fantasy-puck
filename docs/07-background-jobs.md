# Background jobs

Every scheduled and one-shot task the backend runs outside the request path. Three buckets:

1. **Scheduled crons** in [`infra/jobs/scheduler.rs`](../backend/src/infra/jobs/scheduler.rs) - fire on wall-clock time via `tokio_cron_scheduler`.
2. **Continuous pollers** in [`infra/jobs/meta_poller.rs`](../backend/src/infra/jobs/meta_poller.rs) and [`infra/jobs/live_poller.rs`](../backend/src/infra/jobs/live_poller.rs) - spawned at startup, tick forever until SIGTERM.
3. **One-shot startup jobs** - run once during `main.rs` bootstrap if guard conditions say so.

Every admin endpoint that manually triggers one of these jobs is noted in its row.

```
 ┌──────────────────────────────────────────────────────────────────┐
 │                      wall clock (UTC)                             │
 │                                                                  │
 │  09:00 ── morning rankings    (crons)                            │
 │  09:30 ── edge refresher      (crons)                            │
 │  10:00 ── daily prewarm       (crons)                            │
 │  15:00 ── afternoon rankings  (crons)                            │
 └──────────────────────────────────────────────────────────────────┘

 ┌──────────────────────────────────────────────────────────────────┐
 │                    continuous pollers                             │
 │  meta_poller:  5-min tick, schedule + aggregates + rosters       │
 │  live_poller:  60-s tick, in-progress boxscores + state          │
 └──────────────────────────────────────────────────────────────────┘

 ┌──────────────────────────────────────────────────────────────────┐
 │                    startup one-shots                              │
 │  historical_seed                (if table empty)                  │
 │  populate_historical_rankings   (if rankings empty, in-playoffs)  │
 │  ingest_playoff_games_for_range (if playoff stats empty)          │
 │  rehydrate (auto-seed after 45s) (if player-stats empty)          │
 └──────────────────────────────────────────────────────────────────┘
```

## Scheduled crons

All four crons register in `init_rankings_scheduler` ([`scheduler.rs:221-346`](../backend/src/infra/jobs/scheduler.rs)). Cron expressions come from [`tuning::scheduler`](../backend/src/tuning.rs) and use the six-field form (`sec min hour dom mon dow`) that `tokio_cron_scheduler` expects.

| Job | Cron | UTC | ET | Function | Admin trigger |
| --- | --- | --- | --- | --- | --- |
| Morning rankings | `0 0 9 * * *` | 09:00 | 05:00 | `process_daily_rankings_all_leagues(yesterday)` + prune `response_cache` rows older than seven days | `GET /api/admin/process-rankings/{date}` |
| Afternoon rankings | `0 0 15 * * *` | 15:00 | 11:00 | Same as morning, without the prune step - safety net for late-published boxscores | Same as above |
| Daily prewarm | `0 0 10 * * *` | 10:00 | 06:00 | `ingest_yesterdays_playoff_games` → `prewarm_derived_payloads` (insights + race-odds per league, plus global) | `GET /api/admin/prewarm` |
| Edge refresh | `0 30 9 * * *` | 09:30 | 05:30 | `edge_refresher::run(force=false)` | Triggered opportunistically by `/api/admin/prewarm` with the same freshness gate |

### Morning rankings (09:00 UTC)

Defined at [`scheduler.rs:239-270`](../backend/src/infra/jobs/scheduler.rs). For each league:

1. Compute yesterday's date (UTC-based).
2. Call `process_daily_rankings(db, nhl, yesterday, league_id)` ([`scheduler.rs:19-99`](../backend/src/infra/jobs/scheduler.rs)):
   - If any game on yesterday is still `LIVE` / `CRIT` / `PRE`, skip (the daily total is still moving).
   - Read `v_daily_fantasy_totals` filtered to that league + date, ordered by `points DESC`.
   - Upsert into `daily_rankings` with 1-based rank.

After iterating all leagues, delete `response_cache` rows where `date` is older than `tuning::scheduler::CACHE_RETENTION` (seven days).

### Afternoon rankings (15:00 UTC)

Same pipeline as morning but without the cache prune. Exists because the NHL sometimes re-publishes boxscores hours after the game ends; the first morning run can snapshot a partial row. Re-running upserts the correct values over the same `(team_id, date, league_id)` key.

### Daily prewarm (10:00 UTC)

Defined at [`scheduler.rs:293-302`](../backend/src/infra/jobs/scheduler.rs). Two phases:

1. **`ingest_yesterdays_playoff_games`** ([`scheduler.rs:127-148`](../backend/src/infra/jobs/scheduler.rs)) - `playoff_ingest::ingest_playoff_games_for_date(yesterday)`. Upserts per-skater playoff stats into `playoff_skater_game_stats` and team results into `playoff_game_results`.
2. **`prewarm_derived_payloads`** ([`scheduler.rs:154-218`](../backend/src/infra/jobs/scheduler.rs)):
   - Rebuild an `AppState` inside the job so it can call handler functions directly.
   - If `game_type == 3`, refresh `playoff_roster_cache` (one JSONB blob with all 16 playoff rosters, fetched sequentially with roster pacing; if NHL rejects the refresh but a cache row already exists, keep the existing row).
   - Call `generate_and_cache_insights(state, "")` and `generate_and_cache_race_odds(state, "", None)` for the global (no-league) variant.
   - For every league, call the same two functions with that league's id.

Order matters: playoff ingest goes first so the projection model inside `race_odds` reads fresh player facts. Edge refresh is a separate cron 30 min earlier for the same reason (fresh telemetry feeds the insights pre-warm).

### Edge refresh (09:30 UTC)

Defined at [`scheduler.rs:307-315`](../backend/src/infra/jobs/scheduler.rs). Calls `edge_refresher::run(db, nhl, force=false)`. The freshness gate inside the refresher skips the run if `nhl_skater_edge` was updated within the last 18 hours - which means either the 09:30 cron or an admin prewarm becomes a no-op when the other already refreshed Edge recently. See [`04-nhl-integration.md`](./04-nhl-integration.md) for the refresh mechanics.

## Continuous pollers

### Meta poller

File: [`backend/src/infra/jobs/meta_poller.rs`](../backend/src/infra/jobs/meta_poller.rs). Full details in [`04-nhl-integration.md`](./04-nhl-integration.md).

| Property | Value |
| --- | --- |
| Interval | 5 min |
| Startup delay | 15 s |
| Leader election | Postgres advisory lock `884_471_193_001` |
| Writes every tick | `nhl_games` for today; `nhl_game_landing` for FUT games |
| Writes every 6 ticks (≈30 min) | Tomorrow's schedule, standings, skater leaderboard, goalie leaderboard, playoff carousel |
| Writes every 288 ticks (≈24 h) | All 32 team rosters (sequential with 250 ms pacing) |

### Live poller

File: [`backend/src/infra/jobs/live_poller.rs`](../backend/src/infra/jobs/live_poller.rs). Details in [`04-nhl-integration.md`](./04-nhl-integration.md).

| Property | Value |
| --- | --- |
| Interval | 60 s |
| Startup delay | 45 s |
| Leader election | Postgres advisory lock `884_471_193_002` |
| Work | Via `nhl_mirror::list_games_needing_poll(today)`: every `LIVE` / `CRIT` row regardless of date, plus `PRE` rows on today. For each one: upsert boxscore, update state/score/period, invalidate `pulse_narrative:{league}:*` on `LIVE|CRIT → OFF|FINAL` transition. The any-date sweep is the self-heal pass — a process restart or rate-limit blip can leave a row stuck on `LIVE` after the real game finalised, and a today-only query would never re-check it. |

## Startup one-shots

These run in `main.rs` before (and during) the pollers starting. All are idempotent - reboot the process and they short-circuit when their guard condition says the work is already done.

### Historical-skater CSV seed

[`main.rs:100-107`](../backend/src/main.rs) → [`infra/jobs/historical_seed.rs`](../backend/src/infra/jobs/historical_seed.rs).

- **Guard:** `SELECT COUNT(*) FROM historical_playoff_skater_totals > 0` → skip.
- **Work:** parse an `include_str!`-embedded CSV (600 rows, ~36 KB, five-year playoff aggregate) and bulk-insert.
- **Consumer:** the Bayesian shrinkage prior in `domain::prediction::player_projection`.

The CSV is regenerated offline by `backend/scripts/parse_historical_playoff_skaters.py` from a hockey-reference export. Updating it requires truncating the table or otherwise bumping a seed version; the current seed function does not diff.

### Historical rankings backfill

[`main.rs:110-124`](../backend/src/main.rs) → [`scheduler::populate_historical_rankings`](../backend/src/infra/jobs/scheduler.rs).

- **Guard:** `today >= playoff_start` AND `scheduler::is_rankings_table_empty(db)` → run. Otherwise skip.
- **Work:** iterate dates from `playoff_start` through `min(today, season_end)`; for each league × each date, call `process_daily_rankings`.
- **Consumer:** the Rankings page, which reads `daily_rankings`.

### Playoff skater-stats backfill

[`main.rs:130-151`](../backend/src/main.rs) → [`playoff_ingest::ingest_playoff_games_for_range`](../backend/src/infra/jobs/playoff_ingest.rs).

- **Guard:** `is_playoff_skater_game_stats_empty(db)` → run. Otherwise skip.
- **Work:** iterate dates from `playoff_start` through today; for each, ingest completed playoff games into `playoff_skater_game_stats` and `playoff_game_results`.
- **Consumer:** player projection model (recency-weighted rate), playoff Elo loop (chronological replay).

### Auto-seed rehydrate

[`main.rs:200-233`](../backend/src/main.rs) → [`infra/jobs/rehydrate.rs`](../backend/src/infra/jobs/rehydrate.rs).

- **Timer:** `tokio::time::sleep(45 s)` so the meta poller's first tick populates `nhl_games`.
- **Guard:** `SELECT COUNT(*) FROM nhl_player_game_stats > 0` → skip.
- **Work:** call `rehydrate::run` - for each known game row, fetch the boxscore and upsert per-player stats.
- **Motivation:** the live poller never re-fetches boxscores for games that finalized before it first saw them. After a deploy mid-day, every already-final game would otherwise read as zeros in rankings and fantasy totals.

## Admin-triggered work

Admin handlers are at [`backend/src/api/handlers/admin.rs`](../backend/src/api/handlers/admin.rs). Every handler checks `auth.is_admin` and returns 403 if unset.

| Endpoint | Handler line | Purpose |
| --- | --- | --- |
| `GET /api/admin/process-rankings/{date}` | `admin.rs:23-42` | Re-run `process_daily_rankings` for every league on the given date. Use when the morning cron missed a date or snapshotted zeros against a not-yet-populated mirror. |
| `GET /api/admin/cache/invalidate?scope=(all\|today\|{date})` | `admin.rs:49-87` | Delete `response_cache` rows matching scope; optionally also clear the NHL client's in-memory URL cache. |
| `GET /api/admin/backfill-historical?from=&to=` | `admin.rs:105-125` | Re-run `playoff_ingest::ingest_playoff_games_for_range` over a date range. Use after filling a known gap in `playoff_skater_game_stats`. |
| `GET /api/admin/rebackfill-carousel?season=` | `admin.rs:142-158` | Rebuild `playoff_game_results` for a past season via the playoff-carousel + series-games endpoints. |
| `GET /api/admin/calibrate?...` | `admin.rs` | Single-run calibration report. See [`05-prediction-engine.md`](./05-prediction-engine.md). |
| `GET /api/admin/calibrate-sweep?...` | `admin.rs:291-318` | Grid-search calibration. Capped at 200 cells. |
| `GET /api/admin/prewarm` | `admin.rs:258-281` | Fire `edge_refresher::run(force=false)`, then `scheduler::prewarm_derived_payloads`. Runs in `tokio::spawn` so the HTTP response returns immediately; progress in server logs. |
| `GET /api/admin/rehydrate` | `admin.rs:327-337` | Synchronous run of every mirror-poller step plus a boxscore backfill for every game in `nhl_games`. Returns a JSON summary of row counts. |

## Retention and pruning

- `response_cache` - rows with `date` older than seven days are deleted by the 09:00 UTC cron ([`scheduler.rs:256-267`](../backend/src/infra/jobs/scheduler.rs)).
- `daily_rankings` - not pruned.
- `nhl_player_game_stats` - not pruned.
- NHL client in-memory URL cache - per-entry TTL is honoured by `start_cache_cleanup` (see [`04-nhl-integration.md`](./04-nhl-integration.md)); no disk state.

## Notes on multi-replica deployment

`tokio_cron_scheduler` runs every cron on every replica. The pollers coordinate via Postgres advisory locks - only one replica actually does the work per tick - but the crons do not. The scheduler module docstring ([`tuning.rs:162-166`](../backend/src/tuning.rs)) calls this out: before scaling past one Fly machine, wrap the four cron jobs in a leader-election primitive so they do not fire N times per tick.
