# NHL integration

How the backend talks to `api-web.nhle.com`, how it keeps a local mirror of the data in Postgres, and how it avoids hitting rate limits on playoff evenings.

Three pieces:

1. The **NHL HTTP client** ([`infra/nhl/client.rs`](../backend/src/infra/nhl/client.rs)) - a `reqwest` wrapper with a semaphore, retry loop, and in-memory URL cache.
2. The **mirror tables** in Postgres ([`02-database.md`](./02-database.md), section 3) - every handler reads from here, never from the client directly.
3. The **pollers** under [`infra/jobs/`](../backend/src/infra/jobs/) - meta (slow-moving metadata every 5 min), live (game-in-progress every 60 s), edge (nightly). They write the mirror.

```
    ┌─────────────────┐
    │   api-web       │
    │   .nhle.com     │  (undocumented; no public rate quota)
    └────────┬────────┘
             │
             │  reqwest + 10-concurrency semaphore
             │        + 5-retry exponential backoff (500 ms base)
             │        + per-endpoint in-memory URL cache
             ▼
    ┌─────────────────┐
    │   NhlClient     │
    └────────┬────────┘
             │
      ┌──────┴───────┐────────────────┬──────────────┐
      ▼              ▼                ▼              ▼
   meta_poller   live_poller    edge_refresher    handlers that still
   (5 min)       (60 s)         (nightly 09:30)   call NHL directly on
      │              │                │          cache miss (few, wrapped
      ▼              ▼                ▼          in response_cache reads)
  ┌───────────────────────────────────────┐
  │          Postgres mirror tables        │
  │  (nhl_games, nhl_player_game_stats,    │
  │   nhl_skater_season_stats, etc.)       │
  └───────────────────┬───────────────────┘
                      ▼
              user-facing handlers
```

## The NHL HTTP client

File: [`backend/src/infra/nhl/client.rs`](../backend/src/infra/nhl/client.rs). Base URL and URL builders in [`infra/nhl/constants.rs`](../backend/src/infra/nhl/constants.rs).

| Property | Value | Source |
| --- | --- | --- |
| Base URL | `https://api-web.nhle.com` | `constants.rs:7` |
| Max concurrent requests | 10 | `tuning::nhl_client::MAX_CONCURRENT_REQUESTS` |
| Max 429 retries | 5 | `tuning::nhl_client::MAX_RETRIES` |
| Retry backoff | 500 ms, 1 s, 2 s, 4 s, 8 s | `tuning::nhl_client::RETRY_INITIAL_DELAY` doubled per attempt |
| Request timeout | 30 s | `tuning::nhl_client::REQUEST_TIMEOUT` |
| Cache sweep interval | 5 min | `tuning::nhl_client::CACHE_CLEANUP_INTERVAL`, spawned by `start_cache_cleanup()` at boot |

### Rate limit handling

The NHL API enforces per-IP limits without a public quota. The client's `fetch_raw` loop ([`client.rs:107-160`](../backend/src/infra/nhl/client.rs)) treats a 429 as transient: it waits `base << (retries-1)` milliseconds and tries again, up to five retries. At the default 500 ms base, the worst-case total wait is about 15 seconds before the call surfaces an `Error::NhlApi` to the caller.

Every outbound call also has to acquire a permit from a shared `tokio::sync::Semaphore` of size 10. That's the ceiling on how many NHL requests this process can have in flight at once. Raising it speeds up fan-out-heavy pages but pushes more calls into the 429 window; 10 is the value that survived 2026 playoff traffic ([`tuning.rs:67`](../backend/src/tuning.rs)).

### In-memory URL cache

Responses are cached in a `tokio::sync::RwLock<HashMap<String, CacheEntry>>`. Keys are full URLs; entries carry an insertion `Instant` and a `Duration` TTL ([`client.rs:18-29`](../backend/src/infra/nhl/client.rs)). `make_request_cached` checks for a non-expired entry under a read lock, falls back to `fetch_raw` on miss, and writes the body back under a write lock ([`client.rs:163-197`](../backend/src/infra/nhl/client.rs)).

Per-endpoint TTLs, all declared in [`tuning::nhl_client`](../backend/src/tuning.rs):

| Endpoint family | TTL | Why that value |
| --- | --- | --- |
| `SKATER_STATS_TTL`, goalie-stats leaders | 5 min | Leaderboard updates only on completed games |
| `SCHEDULE_TTL` | 2 min | Game state flips are user-visible but not minute-to-minute |
| `GAME_CENTER_TTL` | 2 min | Landing feed; governs the Games-page poll |
| `BOXSCORE_LIVE_TTL` | 60 s | Boxscore for in-progress game; matches live-poller cadence |
| `BOXSCORE_FINAL_TTL` | 24 h | Immutable once the game ends |
| `PLAYOFF_CAROUSEL_TTL` | 15 min | Only changes on series clinch |
| `PLAYER_GAME_LOG_TTL` | 10 min | Heavy Pulse/Insights fan-out amortizes here |
| `PLAYER_DETAILS_TTL` | 30 min | Bio + season totals |
| `STANDINGS_TTL` | 30 min | NHL's own cadence |
| `ROSTER_TTL` | 30 min | Trade/recall events are rare intraday |
| `EDGE_TTL` | 30 min | Season-aggregated telemetry |
| `SCORES_TTL` | 2 min | For the last-game-result sidebar |

`get_game_boxscore` ([`client.rs:520-566`](../backend/src/infra/nhl/client.rs)) is special: the TTL is chosen at call time based on the game state in the response, so live games cache for 60 s and final games cache for a day.

### NHL endpoints the client can reach

Every `get_*` method on `NhlClient` is a thin wrapper around a URL builder in `constants.rs`. The full set:

| Method | Endpoint | TTL source |
| --- | --- | --- |
| `get_skater_stats(season, game_type)` | `/v1/skater-stats-leaders/{season}/{game_type}` | `SKATER_STATS_TTL` |
| `get_goalie_stats(season, game_type)` | `/v1/goalie-stats-leaders/{season}/{game_type}` | `SKATER_STATS_TTL` |
| `get_all_teams()` / `get_standings_raw()` | `/v1/standings/now` | `STANDINGS_TTL` |
| `get_standings_for_date(date)` | `/v1/standings/{date}` | `STANDINGS_TTL` |
| `get_team_roster(team)` | `/v1/roster/{team}/current` | `ROSTER_TTL` |
| `get_today_schedule()` | `/v1/schedule/now` | `SCHEDULE_TTL` |
| `get_schedule_by_date(date)` | `/v1/schedule/{date}` | `SCHEDULE_TTL` |
| `get_player_details(id)` | `/v1/player/{id}/landing` | `PLAYER_DETAILS_TTL` |
| `get_player_game_log(id, season, gt)` | `/v1/player/{id}/game-log/{season}/{gt}` | `PLAYER_GAME_LOG_TTL` |
| `get_game_scores`, `get_period_info`, `get_game_data`, `get_game_landing_raw` | `/v1/gamecenter/{game_id}/landing` | `GAME_CENTER_TTL` |
| `get_game_boxscore(id)` | `/v1/gamecenter/{id}/boxscore` | Live or final TTL chosen dynamically |
| `get_scores_by_date(date)` | `/v1/score/{date}` | `SCORES_TTL` |
| `get_playoff_carousel(season)` | `/v1/playoff-series/carousel/{season}` | `PLAYOFF_CAROUSEL_TTL` |
| `get_playoff_series_games(season, letter)` | `/v1/schedule/playoff-series/{season}/{letter}` | `PLAYOFF_CAROUSEL_TTL` |
| `get_skater_edge_detail(id)` | `/v1/edge/skater-detail/{id}/now` | `EDGE_TTL` |

`invalidate_cache()` clears every entry and is called from the admin endpoint `GET /api/admin/cache/invalidate` when the scope includes the in-memory cache.

## The mirror

The in-memory URL cache protects the NHL API from short-term repeats, but it lives inside one process and dies on restart. Postgres holds the durable cache: the mirror tables. See [`02-database.md`](./02-database.md) for full schemas.

Ownership of each mirror table:

| Table | Written by | Read by (main consumers) |
| --- | --- | --- |
| `nhl_games` | `meta_poller` (schedule), `live_poller` (state/score) | Games, Pulse, Insights, rankings cron |
| `nhl_player_game_stats` | `live_poller` (during game), `rehydrate` (cold start), `admin::rehydrate` | `v_daily_fantasy_totals`, rankings handlers |
| `nhl_skater_season_stats` | `meta_poller` (every 6 ticks) | Stats leaderboard, Edge refresher input |
| `nhl_goalie_season_stats` | `meta_poller` (every 6 ticks) | Pulse matchup block, goalie Elo bonus |
| `nhl_standings` | `meta_poller` (every 6 ticks) | Insights, race-odds rating input |
| `nhl_playoff_bracket` | `meta_poller` (every 6 ticks, playoffs only) | Playoffs page, race-odds bracket input |
| `nhl_team_rosters` | `meta_poller` (every 288 ticks), 10:00 UTC prewarm | Draft pool fallback, NHL rosters page |
| `nhl_game_landing` | `meta_poller` (write-once for FUT games) | Insights game card |
| `nhl_skater_edge` | `edge_refresher` (nightly + admin force) | Insights hot card |
| `playoff_skater_game_stats` | `playoff_ingest` (nightly + backfill) | Player projection model, backtest harness |
| `playoff_game_results` | `playoff_ingest` | Playoff Elo replay |
| `historical_playoff_skater_totals` | `historical_seed` (one-shot CSV import) | Projection model prior |
| `playoff_roster_cache` | 10:00 UTC prewarm | Draft `populate_player_pool` |

## Advisory-lock leader election

Two Postgres advisory locks (integer keys defined in [`infra/db/nhl_mirror.rs:37-41`](../backend/src/infra/db/nhl_mirror.rs)):

```rust
const META_LOCK_KEY: i64 = 884_471_193_001;
const LIVE_LOCK_KEY: i64 = 884_471_193_002;
```

Every poller tick does `pg_try_advisory_lock(key)` on a dedicated connection it holds for the duration of the tick, then `pg_advisory_unlock(key)` when the tick ends. A replica that cannot acquire the lock logs at `debug` and returns. Only one replica across the fleet does the work on each tick.

The locks are session-scoped, which is why the pollers hold a dedicated connection for the lock's lifetime - releasing on a different connection would leak the lock ([`nhl_mirror.rs:43-53`](../backend/src/infra/db/nhl_mirror.rs)).

## Meta poller

File: [`backend/src/infra/jobs/meta_poller.rs`](../backend/src/infra/jobs/meta_poller.rs). Spawned from [`main.rs:166-172`](../backend/src/main.rs).

| Property | Value | Source |
| --- | --- | --- |
| Interval | 5 min | `live_mirror::META_POLL_INTERVAL` |
| Startup delay | 15 s | `live_mirror::META_POLL_STARTUP_DELAY` |
| Lock | `META_LOCK_KEY` | |
| Missed-tick behavior | `Skip` | `meta_poller.rs:40` |

The poller maintains a `counter: u32` and uses it to gate work at coarser cadences:

- **Every tick** - Today's schedule → `nhl_games`. Pre-game landing captures for FUT/PRE games → `nhl_game_landing` (write-once). "Today" is the Eastern Time date, not the UTC date, because NHL's `/schedule/{date}` is keyed by ET. The implementation uses `chrono_tz::America::New_York` ([`meta_poller.rs:148`](../backend/src/infra/jobs/meta_poller.rs)).
- **Every 6 ticks (≈30 min)** - Tomorrow's schedule, skater leaderboard, goalie leaderboard, standings, playoff carousel (if `game_type == 3`).
- **Every 288 ticks (≈24 h)** - Walk all 32 team rosters with a 250 ms delay between fetches (`ROSTER_FETCH_DELAY`).

Each step has a freshness gate that reads the mirror's `updated_at` and skips the fetch if the row was touched more recently than the step's TTL. This keeps a server restart from re-fetching everything on the first tick just because `counter` reset to 1 ([`meta_poller.rs:128-136`](../backend/src/infra/jobs/meta_poller.rs)).

Per-step errors are logged at `warn` and swallowed - a transient NHL outage on one endpoint does not prevent the others from running.

## Live poller

File: [`backend/src/infra/jobs/live_poller.rs`](../backend/src/infra/jobs/live_poller.rs). Spawned from [`main.rs:173-180`](../backend/src/main.rs).

| Property | Value | Source |
| --- | --- | --- |
| Interval | 60 s | `live_mirror::LIVE_POLL_INTERVAL` |
| Startup delay | 45 s | `live_mirror::LIVE_POLL_STARTUP_DELAY` |
| Lock | `LIVE_LOCK_KEY` | |

The tick body ([`live_poller.rs:85-109`](../backend/src/infra/jobs/live_poller.rs)):

1. Compute today's ET date.
2. `nhl_mirror::list_live_game_ids_for_date(pool, today)` returns game_ids where state is `LIVE`, `CRIT`, or `PRE`. If empty, return - this is the off-night cost: one SELECT per minute per leader.
3. For each live game, call `poll_one_game`.

`poll_one_game` ([`live_poller.rs:118-214`](../backend/src/infra/jobs/live_poller.rs)) does:

1. Snapshot the previous `game_state` from the mirror.
2. `get_game_boxscore(game_id)` → `upsert_boxscore_players` writes every skater and goalie row into `nhl_player_game_stats`.
3. `get_game_data(game_id)` returns the state/score/period block; `update_game_live_state` writes those columns on `nhl_games`.
4. If `(previous, new)` transitioned from `LIVE|CRIT` to `OFF|FINAL`, look up every league that had a rostered player in this game, and for each call `cache.invalidate_by_like(f"team_diagnosis:{league_id}:%:v2")`. Scores do not need invalidation — they live in the mirror. Only the narrative text, which refers to the in-progress game by name, needs regeneration. The sibling `:bundle:v1` payload is intentionally left in place: its projections, grades, and recent-games rollup are stable through the evening, and wiping it would stall the next Pulse load on a synchronous Claude rebuild.

The invalidation runs exactly once per game because the state write in step 3 flips the mirror before the check in step 4 fires; the next tick sees the new state and skips the block.

## Edge refresher

File: [`backend/src/infra/jobs/edge_refresher.rs`](../backend/src/infra/jobs/edge_refresher.rs). Spawned by cron at 09:30 UTC (see [`07-background-jobs.md`](./07-background-jobs.md)).

| Property | Value | Source |
| --- | --- | --- |
| Top-N cohort | 30 | `live_mirror::EDGE_REFRESH_TOP_N` |
| Per-fetch pace | 500 ms | `live_mirror::EDGE_REFRESH_DELAY` |
| Freshness gate | 18 h | `live_mirror::EDGE_REFRESH_FRESHNESS` |

Run sequence:

1. Unless `force=true`, check `nhl_mirror::last_update_nhl_skater_edge(pool)`; if it's within 18 hours, log and return without fetching.
2. Read the top 30 skaters from `nhl_skater_season_stats` for the active season and game_type.
3. Sequentially (with a 500 ms sleep between), call `nhl.get_skater_edge_detail(player_id)` and upsert `nhl_skater_edge(player_id, top_speed_mph, top_shot_speed_mph)`.
4. Return a `RefreshSummary { refreshed, errors, skipped_fresh }`.

At 30 players × 500 ms, the run takes about 15 seconds of wall-clock time. Operator admin prewarm also passes `force=false`, so a manual trigger respects the same freshness gate and does not immediately spend another 30 Edge calls after the scheduled refresh.

## Auto-seed on boot

File: [`backend/src/main.rs:200-233`](../backend/src/main.rs).

After startup, a background task sleeps 45 s (long enough for the meta poller to populate today's schedule), then runs:

```sql
SELECT COUNT(*) FROM nhl_player_game_stats
```

If the count is zero, it invokes [`infra/jobs/rehydrate::run`](../backend/src/infra/jobs/rehydrate.rs): iterate every game row in `nhl_games`, fetch its boxscore, upsert player stats. This recovers the "deploy in the middle of playoff day" case where every already-final game has no row in `nhl_player_game_stats` and would otherwise read as zero until the next live tick.

The same function is exposed at `GET /api/admin/rehydrate` for explicit reseeds.

## The rule: no NHL calls on the request path (mostly)

Handlers are expected to read from the mirror or from `response_cache`. Exceptions exist - for example the regular-season skater leaderboard falls back to a direct NHL call if the mirror has no rows for `game_type=2` - but every such call is wrapped in a `response_cache` read first, so repeat requests within the cache window do not re-fetch. The long-term direction, documented in the module docstring for `live_mirror`, is to move every step that still hits the NHL API on a request path behind the mirror.
