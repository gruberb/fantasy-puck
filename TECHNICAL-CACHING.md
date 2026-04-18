# Caching and Live-Data Architecture

This document describes how Fantasy Puck caches data, which page reads what, and where the rate-limit pressure comes from. It is intended as a shared map for future changes to the data pipeline.

## Overview

Fantasy Puck has one origin for hockey data — the undocumented NHL API at `api-web.nhle.com` — and three layers that sit between it and a user's browser:

```
[ NHL API ]
     |
     |  retry + semaphore (10 concurrent, 5 retries, exp backoff)
     v
[ NhlClient in-memory cache ]   process-local, lost on deploy
     |                           TTLs: 1 min (live boxscore) to 30 min (rosters)
     v
[ Postgres response_cache ]     survives restart
     |                           keyed per-endpoint, per hockey-date
     v
[ Axum handlers ]
     v
[ React Query ]                  per-client browser state
     v
[ Browser ]
```

Each layer has a different purpose:

- The **NhlClient in-memory cache** is a per-URL map that deduplicates NHL API calls within a single backend process. TTLs vary by endpoint because the upstream data changes at different rates (a live boxscore is stale after a minute; a team roster is stable for 30 minutes).
- **Postgres `response_cache`** stores fully-composed handler responses — the shape the frontend consumes. It survives deploys. Writing here is what prevents "every user re-runs the expensive generation."
- **React Query** on the client has its own stale-time and refetch logic, independent of the server caches.

No NHL data has a single source of truth inside the app; the app is always a view over a cache of a cache of the NHL API. The exceptions are the things the app computes and owns itself: `daily_rankings` (written by the scheduler), `playoff_skater_game_stats` (written by the nightly ingest), and the draft/league/team data. Those are durable app state, not caches.

## The two cache layers

### `response_cache` — Postgres

Defined in `backend/src/db/cache.rs`. Shape:

```sql
response_cache (
  cache_key TEXT PRIMARY KEY,
  date TEXT,              -- hockey date, used for the 7-day cleanup
  data TEXT,              -- serialized JSON payload
  created_at TEXT,
  last_updated TEXT
)
```

Writers across the codebase:

| Cache key format                               | Handler path                                             | What is cached                                       |
|------------------------------------------------|----------------------------------------------------------|------------------------------------------------------|
| `insights:{league_id}:{season}:{gt}:{date}`    | `backend/src/api/handlers/insights.rs`                   | Full insights response (signals + Claude narratives) |
| `insights_landing:{game_id}`                   | `backend/src/api/handlers/insights.rs`                   | Per-game pre-game matchup block (leaders + goalies)  |
| `race_odds:v3:{league_id}:{season}:{gt}:{date}`| `backend/src/api/handlers/race_odds.rs`                  | Monte Carlo output + projections                     |
| `pulse:{league_id}:{team_id}:{season}:{gt}:{date}` | `backend/src/api/handlers/pulse.rs`                  | Personalized Pulse response + Claude narrative       |
| `match_day:{league_id}:{gt}:{date}`            | `backend/src/api/handlers/games.rs` (`get_match_day`)    | Match-day games + fantasy overlays                   |
| `games_extended:{league_id}:{gt}:{date}`       | `backend/src/api/handlers/games.rs` (extended mode)      | Games page payload with per-player extended stats    |
| `daily_rankings:{league_id}:{gt}:{date}`       | `backend/src/api/handlers/rankings.rs`                   | Computed daily leaderboard for a date                |

All keys include `game_type` (`2` regular season, `3` playoffs) so regular-season and playoff payloads cannot collide across a flip. All keys include `date` so rows expire naturally via the 7-day cleanup in `backend/src/utils/scheduler.rs`.

### NhlClient in-memory — per-process map

Defined in `backend/src/nhl_api/nhl.rs`. A `HashMap<url, CacheEntry>` guarded by an `RwLock`. Each entry stores the response body plus its insertion time and a TTL. A background task runs every 5 minutes and evicts expired entries.

TTLs are set per endpoint in the `ttl` submodule (`backend/src/nhl_api/nhl.rs:32-47`):

| Endpoint family        | TTL          | Rationale                                                        |
|------------------------|--------------|------------------------------------------------------------------|
| `BOXSCORE_LIVE`        | 1 minute     | Active game — scores and stats change minute-to-minute           |
| `BOXSCORE_FINAL`       | 24 hours     | Immutable after game ends                                        |
| `SCHEDULE`             | 2 minutes    | Game states transition (FUT → LIVE → OFF) but not continuously   |
| `SCORES`               | 2 minutes    | Day-scoped scores endpoint                                       |
| `GAME_CENTER`          | 2 minutes    | Includes live period/state                                       |
| `SKATER_STATS`         | 5 minutes    | Leaderboard; updates a few times per day                         |
| `PLAYER_GAME_LOG`      | 10 minutes   | Updates once per completed game                                  |
| `PLAYOFF_CAROUSEL`     | 15 minutes   | Changes only when a series resolves or a round starts            |
| `PLAYER_DETAILS`       | 30 minutes   | Bio / season totals                                              |
| `STANDINGS`            | 30 minutes   | Updates after every completed game but not urgent                |
| `ROSTER`               | 30 minutes   | Rosters shift infrequently (trades, call-ups)                    |
| `EDGE`                 | 30 minutes   | Skating/shot telemetry                                           |

Concurrency is capped by a `Semaphore::new(10)`; retries are exponential backoff (500ms → 1s → 2s → 4s → 8s) for up to 5 tries. Both live in `backend/src/nhl_api/nhl.rs:114-164`.

## Per-endpoint flow

This section describes what each user-visible page does when a request arrives, from cache check to NHL fanout.

### `/api/insights`

Handler: `backend/src/api/handlers/insights.rs` → `generate_and_cache_insights`.

1. Check `response_cache` for `insights:{league}:{season}:{gt}:{today}`. If present and has games, return.
2. Compute signals in parallel: `get_schedule_by_date`, `get_skater_stats` (playoff + regular-season fallback), `get_standings_raw`, `get_scores_by_date(yesterday)`, one `get_game_landing_raw` per game (now read-through `insights_landing:{game_id}` cache — see below), `get_player_form` for top 20 hot players, `get_skater_edge_detail` for top 5.
3. Call Claude to generate narratives.
4. Write the full payload to `response_cache` with today's date.

The per-game landing cache (`insights_landing:{game_id}`) is **write-once**: if a cached matchup block exists and has leaders or goalies, return it immediately. If not, fetch from NHL. If the NHL response has a populated `matchup` block (pre-game state), write it. If not (game is already LIVE, the matchup block is gone), do not write — return what we have for this request but leave the slot open for a future pre-game fetch.

### `/api/pulse`

Handler: `backend/src/api/handlers/pulse.rs` → `get_pulse`.

1. Check `response_cache` for `pulse:{league}:{team}:{season}:{gt}:{today}`.
2. On miss: fetch `get_schedule_by_date`, `get_playoff_carousel`, compute league board (reads `daily_rankings` DB table), compute series forecast, compute "my games tonight" per team.
3. Call Claude for a personal narrative (only if a team is resolved).
4. Write to `response_cache`.

Pulse is the most expensive per-team endpoint because it is personalized. It can fetch `get_player_game_log` per player in edge cases (sparklines). It is cached once per day per (league, team). **It does not currently reflect live game points** — if Taylor Hall scores during a slate, his 2 points do not appear on Pulse until the next day's rankings are rolled into the sparkline.

### `/api/games` (basic mode)

Handler: `backend/src/api/handlers/games.rs` → `list_games` → `process_games` (when no league or `detail=basic`).

1. `get_schedule_by_date` for the requested date.
2. For each game with a live/completed state, `get_game_boxscore` to fill player stats.
3. For each game with no `game_score` in the schedule, `get_game_scores` as a fallback.

No Postgres cache layer for basic mode. The response is cheap because it has no per-player fanout.

### `/api/games?detail=extended` (with `league_id`)

Handler: `backend/src/api/handlers/games.rs` → `list_games` → `process_games_extended`.

1. Check `response_cache` for `games_extended:{league}:{gt}:{date}`. If present and no live games, return. If live games exist, fall through and regenerate.
2. `get_schedule_by_date`. For early-morning in-ET hours, also fetch yesterday's live games.
3. In parallel: fetch `get_game_boxscore` per live/completed game AND `get_player_game_log` per unique rostered skater on the slate. The NhlClient semaphore caps concurrency at 10.
4. If any live-game boxscore came back `None`, retry each one sequentially once — this catches transient 429s.
5. Derive scores from the boxscore if both the schedule and `get_game_scores` returned null.
6. Build per-fantasy-team player breakdowns (requires `get_player_game_log` for form; the prefetch in step 3 warmed the NhlClient cache so these are in-memory hits).
7. Write to `response_cache`.

This is the heaviest endpoint. On a 4-game slate with 5 fantasy teams and ~12 skaters per team, the cold-load fetch count is ~60–80 NHL calls.

### `/api/fantasy/rankings/daily`

Handler: `backend/src/api/handlers/rankings.rs` → `get_daily_rankings`.

1. Check `response_cache` for `daily_rankings:{league}:{gt}:{date}`. If present, return.
2. `get_schedule_by_date`, filter to live/completed games.
3. `get_game_boxscore` per game (buffer_unordered(4)). Aggregate team performances from boxscores.
4. Return ranked teams. Write to `response_cache`.

The source of truth for finalized rankings is the `daily_rankings` Postgres table, populated by the scheduler at 09:00 and 15:00 UTC. This handler **recomputes from boxscores on every cache miss**, so it is used both by the Rankings page (for finalized days) and for in-progress days. The Postgres response cache prevents the per-request fanout; the scheduler keeps the authoritative table updated once games settle.

### `/api/fantasy/rankings` (overall)

Handler: `backend/src/api/handlers/rankings.rs` → `get_rankings`.

Single NHL call: `get_skater_stats`. Merges per-team rosters against the leader board in memory. No Postgres cache.

### `/api/race-odds`

Handler: `backend/src/api/handlers/race_odds.rs` → `generate_and_cache_race_odds`.

1. Check `response_cache` for `race_odds:v3:*:*:*:{date}`.
2. Run the Monte Carlo model. Inputs: `get_skater_stats`, `get_goalie_stats`, `get_standings_raw`, `get_playoff_carousel`, and the cached playoff roster pool (see `fetch_playoff_roster_pool_cached` in `backend/src/utils/player_pool.rs`).
3. Write to `response_cache`.

Prewarmed at 10:00 UTC by the scheduler so user visits hit the cache.

### `/api/stats/top-skaters`

Handler: `backend/src/api/handlers/stats.rs` → `get_top_skaters`.

- Regular season: `get_skater_stats` — one NHL call.
- Playoffs: `fetch_playoff_roster_pool_cached` — reads from the `playoff_roster_cache` Postgres table, falls back to 16 parallel `get_team_roster` calls only if the cache is empty.
- Optionally fires `get_player_form` per player if `include_form=true`.

## Data freshness table

How fresh is each piece of data when it reaches the user? This is the table to consult when debugging "why did the UI show X for five minutes after the real value changed."

| Data                          | Layer                       | Max staleness            | Writer                                          | Readers                                          |
|-------------------------------|-----------------------------|--------------------------|-------------------------------------------------|--------------------------------------------------|
| Today's schedule              | NhlClient in-mem            | 2 min                    | `get_schedule_by_date`                          | insights, games, pulse, rankings                 |
| Live boxscore                 | NhlClient in-mem            | 1 min                    | `get_game_boxscore` (LIVE)                      | games, rankings                                  |
| Final boxscore                | NhlClient in-mem            | 24 h                     | `get_game_boxscore` (OFF/FINAL)                 | games, rankings, scheduler                       |
| Game score (schedule fallback)| NhlClient in-mem            | 2 min                    | `get_game_scores`                               | games                                            |
| Skater stats leaders          | NhlClient in-mem            | 5 min                    | `get_skater_stats`                              | insights, pulse, stats, race-odds, rankings      |
| Goalie stats leaders          | NhlClient in-mem            | 5 min                    | `get_goalie_stats`                              | race-odds                                        |
| Standings                     | NhlClient in-mem            | 30 min                   | `get_standings_raw`                             | insights, race-odds, rankings                    |
| Team roster                   | NhlClient in-mem            | 30 min                   | `get_team_roster`                               | stats, draft, race-odds (via pool)               |
| Playoff roster pool           | Postgres + NhlClient        | 1 day (prewarm)          | `refresh_playoff_roster_cache` (10:00 UTC)      | stats, draft                                     |
| Playoff carousel              | NhlClient in-mem            | 15 min                   | `get_playoff_carousel`                          | insights, pulse, race-odds                       |
| Player game log (form)        | NhlClient in-mem            | 10 min                   | `get_player_game_log`                           | insights, pulse, stats, games extended           |
| Player edge telemetry         | NhlClient in-mem            | 30 min                   | `get_skater_edge_detail`                        | insights (hot players)                           |
| Insights response             | Postgres `response_cache`   | 1 hockey day             | `generate_and_cache_insights`                   | `/api/insights`                                  |
| Insights per-game landing     | Postgres `response_cache`   | 1 hockey day (write-once)| `get_or_fetch_landing_cached`                   | insights generation                              |
| Pulse response                | Postgres `response_cache`   | 1 hockey day             | `get_pulse`                                     | `/api/pulse`                                     |
| Games extended response       | Postgres `response_cache`   | 1 hockey day, invalidates on live-game detection | `process_games_extended`            | `/api/games?detail=extended`                     |
| Daily rankings response       | Postgres `response_cache`   | 1 hockey day             | `get_daily_rankings`                            | `/api/fantasy/rankings/daily`                    |
| Daily rankings (truth)        | Postgres `daily_rankings`   | updated 9am + 3pm UTC    | scheduler `process_daily_rankings`              | team stats, pulse sparklines                     |
| Playoff skater stats          | Postgres table              | daily (nightly ingest)   | `ingest_playoff_games_for_date` @ 10:00 UTC     | race-odds player projections                     |
| Race-odds response            | Postgres `response_cache`   | 1 hockey day             | `generate_and_cache_race_odds`                  | `/api/race-odds`                                 |

"1 hockey day" means the cache key includes the hockey-today date; when the date rolls over (midnight Eastern), the next request generates a fresh entry.

## Scheduled jobs

Three cron jobs in `backend/src/utils/scheduler.rs` plus a couple of startup tasks in `backend/src/main.rs`.

### 09:00 UTC — morning rankings

Job: `init_rankings_scheduler` morning task.

For every league, run `process_daily_rankings` against *yesterday's* date:

1. `get_schedule_by_date(yesterday)`.
2. Filter to completed games.
3. Fetch `get_game_boxscore` for each completed game (concurrent, buffer_unordered(4)).
4. Aggregate per fantasy-team performances, assign ranks, upsert into `daily_rankings` table.

Also cleans up `response_cache` rows with `date < yesterday - 7 days`.

### 15:00 UTC — afternoon rankings

Same as 09:00 UTC. This is a safety net for late-finishing games (Pacific-time games that end after 09:00 UTC but before 15:00 UTC).

### 10:00 UTC — daily prewarm

Three steps in order:

1. `ingest_yesterdays_playoff_games` — fetch yesterday's playoff boxscores into `playoff_skater_game_stats` and `playoff_game_results`. Used by the race-odds player-projection model.
2. `refresh_playoff_roster_cache` (when `game_type == 3`) — fetch 16 team rosters, write JSONB blob to `playoff_roster_cache`. Downstream reads hit Postgres only.
3. `prewarm_derived_payloads` — call `generate_and_cache_insights` and `generate_and_cache_race_odds` for the global view and every league. On a successful pass, every user visit that day is a cache hit.

### Startup (in `backend/src/main.rs`)

- `NhlClient::start_cache_cleanup(5 min)` — evicts expired in-memory entries.
- If today ≥ playoff start and the `daily_rankings` table is empty: background-spawn `populate_historical_rankings` for every league from playoff start to yesterday. One-time backfill.
- If `playoff_skater_game_stats` is empty: background-spawn the same for the playoff ingest.

## Frontend refresh patterns

The frontend is a Vite/React/React Query SPA. Each page's refetch behavior:

| Page            | React Query stale time | Auto-refresh                                                              |
|-----------------|-----------------------|---------------------------------------------------------------------------|
| Home            | 5 min (default)       | None                                                                      |
| Insights        | 15 min                | None — page is a daily preview, refetch on navigate                       |
| Pulse           | 1 min                 | None. Relies on React Query's focus-refetch                               |
| Race-Odds       | 15 min                | None                                                                      |
| Games           | 5 min                 | **Opt-in 30-second polling** via checkbox; auto-disables when no live games |
| Rankings        | 5 min                 | None                                                                      |

The Games page's auto-refresh is the only client-side polling in the app. It is off by default; the user toggles it with a checkbox. The toggle only takes effect while `hasLiveGames` is true, and it disables itself when the user navigates to a date with no live games.

## Rate-limit offenders, with post-fix status

Each of these is a path that triggered NHL rate limits this playoff season. The "status" column reflects the changes made in the current fix session.

| Path                                                    | Pre-fix behavior                                                         | Post-fix status                                                                 |
|---------------------------------------------------------|---------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `get_daily_rankings`                                    | N `get_game_boxscore` per request, no response cache                      | Postgres `daily_rankings:*` cache in front (~1 hockey day stale window)         |
| `fetch_playoff_roster_pool` (stats + draft)             | 16 parallel `get_team_roster` per cold request                            | `fetch_playoff_roster_pool_cached` reads from `playoff_roster_cache` Postgres table; prewarm refreshes at 10:00 UTC |
| `compute_todays_games` (insights)                       | N parallel `get_game_landing_raw`; one 429 killed the sidebar for the day | Per-game `insights_landing:{game_id}` cache with write-once semantics. First pre-game fetch locks in the sidebar |
| `process_games_extended` (games page)                   | Silent `0 pts` when any boxscore 429'd; "just the time" when both score endpoints 429'd | Boxscore retry once sequentially for live games. Score derived from boxscore as fallback |
| `NhlClient` retry budget                                | 3 retries linear (500 ms → 1.5 s), ~1.5 s worst case                      | 5 retries exponential (500 ms → 8 s), ~15 s worst case                          |
| Frontend rankings date default                          | Defaulted to today — always empty before games completed                  | Defaults to yesterday via `getMostRecentRankingsDate`                           |

Still on the books, not yet addressed:

- **Live games have no server-side refresher.** Every device hitting `/games` during a live slate independently fans out per-game boxscores (one request, cached in Postgres, but the cache is invalidated for live games so every request regenerates). A single server-side poller that updates a shared live-state map would collapse N devices into 1 call per game per minute.
- **Pulse does not reflect live points.** Because Pulse is cached for the hockey-day and there is no invalidation on live games, mid-game scoring never appears. Fixing this depends on the live-state map above.
- **Overall rankings have no daily update during live games.** Same root cause as Pulse — the `daily_rankings` table is only updated by the 09:00 / 15:00 UTC crons.

These are the targets of the next data-pipeline redesign.

## File index

When debugging, start at these files:

- `backend/src/db/cache.rs` — Postgres cache service.
- `backend/src/nhl_api/nhl.rs` — NHL client, in-memory cache, retry, semaphore.
- `backend/src/utils/scheduler.rs` — cron jobs, prewarm.
- `backend/src/utils/player_pool.rs` — roster pool with Postgres fallback.
- `backend/src/api/handlers/insights.rs` — insights + per-game landing cache.
- `backend/src/api/handlers/pulse.rs` — personalized Pulse.
- `backend/src/api/handlers/games.rs` — `/games` basic + extended.
- `backend/src/api/handlers/rankings.rs` — overall + daily rankings.
- `backend/src/api/handlers/race_odds.rs` — Monte Carlo.
- `frontend/src/features/games/hooks/use-games-data.ts` — the only opt-in polling on the client.
