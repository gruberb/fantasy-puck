# Data Pipeline Redesign: NHL Mirror + Live Poller

Status: **proposed** — not yet implemented.
Owner: backend.
Supersedes: the per-endpoint NHL fan-out pattern currently documented in [TECHNICAL-CACHING.md](../TECHNICAL-CACHING.md).

## Why this redesign

Fantasy Puck calls the NHL API from the request path. Every `/games`, `/pulse`, `/insights`, `/rankings`, `/stats`, or `/race-odds` request fans out some number of NHL calls on a cache miss. During the 2026 playoffs this pattern produced three recurring failure modes:

1. **Rate-limit cascades.** A single browser tab that triggered `/api/fantasy/rankings/daily` could fan out 1 NHL boxscore call per live or completed game on the date. Multiple concurrent tabs multiplied the fanout. When one call 429'd past the retry budget, the handler returned 500 and the browser refreshed, re-triggering the same fan-out. The log symptom was the same `game_id` appearing in rate-limit errors every 2–4 minutes.
2. **Silent partial payloads.** Insights' pre-game matchup block is only present in the NHL landing response while a game is in FUT state. If the first fetch happened after puck drop, the matchup came back empty, the payload was cached (post [initial bandaid fix](#applied-bandaid-fixes)), and the sidebar was blank for the rest of the day.
3. **"Live but not really."** Pulse, Rankings, and Stats cache their responses for the full hockey day. When a rostered player scores mid-game, the update does not appear on any of those surfaces until the next day's scheduler cron writes the finalized daily-rankings row.

The underlying cause is shared by all three: the app asks NHL for data when users ask the app for data. The target architecture inverts that.

## Target architecture

> One backend process owns all NHL traffic. Every other read path is a pure database read.

Two background pollers run in the single backend process:

- **Metadata poller** (every 5 min): schedule, standings, skater/goalie leaderboards, playoff carousel, and (every 6th tick) team rosters. Populates the corresponding mirror tables.
- **Live poller** (every 60 s, only when at least one game today is LIVE): boxscore + scores for each live game. Updates `nhl_games` and `nhl_player_game_stats`.

A small set of Postgres tables mirrors every shape the app reads:

| Table | Contents | Written by |
|---|---|---|
| `nhl_games` | Schedule, state, scores, period, series status, venue. One row per game. | Both pollers |
| `nhl_player_game_stats` | Per-game per-player stats. Live rows are mutable; final rows are immutable. | Live poller |
| `nhl_skater_season_stats` | Season leaderboard (goals/assists/points + extras per player). | Metadata poller |
| `nhl_goalie_season_stats` | Season goalie leaderboard. | Metadata poller |
| `nhl_team_rosters` | Roster JSONB per team + season. | Metadata poller |
| `nhl_standings` | Latest standings row per team. | Metadata poller |
| `nhl_playoff_bracket` | Playoff carousel JSONB. | Metadata poller |
| `nhl_game_landing` | Pre-game matchup block per game. Write-once: captured while FUT, never overwritten by a later LIVE-state fetch. | 10:00 UTC prewarm job |
| `playoff_roster_cache` | Playoff 16-team roster pool. *(already implemented this session.)* | 10:00 UTC prewarm job |

The schemas are defined in [`backend/supabase/migrations/20260420000000_nhl_mirror.sql`](../backend/supabase/migrations/20260420000000_nhl_mirror.sql).

A Postgres view provides today's running fantasy totals without a materialized cache:

```sql
CREATE VIEW v_daily_fantasy_totals AS
SELECT
    ft.league_id, ft.id AS team_id, ft.name AS team_name,
    g.game_date AS date,
    COALESCE(SUM(pgs.goals),   0) AS goals,
    COALESCE(SUM(pgs.assists), 0) AS assists,
    COALESCE(SUM(pgs.points),  0) AS points
FROM   nhl_player_game_stats pgs
JOIN   nhl_games g         ON g.game_id = pgs.game_id
JOIN   fantasy_players fp  ON fp.nhl_id  = pgs.player_id
JOIN   fantasy_teams   ft  ON ft.id      = fp.team_id
GROUP  BY ft.league_id, ft.id, ft.name, g.game_date;
```

Pulse's "points today", the Rankings daily board, and the Stats/Sleepers season totals all read this view. The live poller writes player stats → the view recomputes on read. No materialization, no cache-bust logic.

## Poller cadences

All intervals live in [`backend/src/tuning.rs`](../backend/src/tuning.rs) under the `live_mirror` and `scheduler` modules. The compiled-in defaults:

| Setting | Value | Constant |
|---|---|---|
| Live poller period | 60 s | `tuning::live_mirror::LIVE_POLL_INTERVAL` |
| Metadata poller period | 5 min | `tuning::live_mirror::META_POLL_INTERVAL` |
| Roster refresh cadence | Every 6 meta ticks (30 min) | `tuning::live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS` |
| Morning rankings cron | 09:00 UTC | `tuning::scheduler::MORNING_RANKINGS_CRON` |
| Afternoon rankings cron | 15:00 UTC | `tuning::scheduler::AFTERNOON_RANKINGS_CRON` |
| Daily prewarm cron | 10:00 UTC | `tuning::scheduler::DAILY_PREWARM_CRON` |

Changing any of these is a code-review, not a deploy flag. The cross-cutting constraint — `nhl_client::REQUEST_TIMEOUT` ≥ backoff-sum, `http::AXUM_REQUEST_TIMEOUT` ≥ `REQUEST_TIMEOUT` — is documented in the module-level comments in `tuning.rs`.

## Per-page rewrite

Each user-facing handler becomes a pure database read. The new source column below refers exclusively to tables in this repo.

| Page | New source |
|---|---|
| Home | `nhl_skater_season_stats`, `daily_rankings` (unchanged) |
| Insights | `nhl_games`, `nhl_game_landing`, `nhl_skater_season_stats`, `nhl_standings`, plus `response_cache` for the Claude narrative |
| Pulse | `nhl_games`, `nhl_playoff_bracket`, `v_daily_fantasy_totals` for today, `daily_rankings` for historical |
| Games (basic + extended) | `nhl_games`, `nhl_player_game_stats` |
| Rankings (overall) | `nhl_skater_season_stats` joined to league roster |
| Rankings (daily) | `v_daily_fantasy_totals` for today, `daily_rankings` for historical |
| Race-odds | `nhl_skater_season_stats`, `nhl_goalie_season_stats`, `nhl_standings`, `nhl_playoff_bracket`, `playoff_roster_cache` |
| Stats (top skaters) | `nhl_skater_season_stats`, `nhl_player_game_stats` (for form) |
| Sleepers | Existing DB tables + `nhl_skater_season_stats` |
| Draft | `nhl_team_rosters` (playoffs) or `nhl_skater_season_stats` (regular season) |

`NhlClient` stays in the process but leaves the request path. Only the pollers, the 10:00 UTC prewarm, and the playoff ingest call it.

## Files

New:

- `backend/supabase/migrations/20260420000000_nhl_mirror.sql` — tables and view. (**Already committed** — sits unused until the pollers land.)
- `backend/src/utils/nhl_mirror.rs` — poller implementations and upsert helpers.
- `backend/src/db/nhl_mirror.rs` — typed read-side queries.

Modified:

- `backend/src/main.rs` — spawn both pollers.
- `backend/src/api/handlers/insights.rs` — read from mirror.
- `backend/src/api/handlers/pulse.rs` — read from mirror + view.
- `backend/src/api/handlers/games.rs` — read from mirror. Biggest single rewrite.
- `backend/src/api/handlers/rankings.rs` — view for today, unchanged for historical.
- `backend/src/api/handlers/race_odds.rs` — read inputs from mirror.
- `backend/src/api/handlers/stats.rs` — read from mirror.
- `backend/src/api/handlers/draft.rs` — read from mirror.
- `backend/src/utils/scheduler.rs` — add pre-game landing capture to the 10:00 UTC prewarm.

## Cutover, ordering, and cold-start

The change is designed to ship as one coordinated PR but the internal ordering matters so the working tree never breaks between commits in the sequence.

1. Migrations land. Empty tables are harmless.
2. `nhl_mirror.rs` pollers land with `db/nhl_mirror.rs`. Tables populate on the first post-deploy boot.
3. Handler rewrites in this order: Games (extended), Pulse, Rankings (daily), Stats, Rankings (overall), Insights, Race-odds, Draft. Each rewrite is validated in the browser before the next one.
4. Last commit removes the NHL client from the handler layer. A grep for `state.nhl_client` in `api/handlers/` should return zero results.

Cold-start behaviour on first production deploy: the tables are empty. Handlers must tolerate empty reads by returning an empty-but-well-shaped response rather than 500. Within 5 minutes the metadata poller has populated every mirror table except `nhl_player_game_stats` (which fills as live games happen). A one-shot backfill endpoint `GET /api/admin/rehydrate` triggers every poller step synchronously, intended for use post-deploy to skip the warmup window.

## Rollback

Revert the commit. The mirror tables remain in Postgres (harmless), the pollers stop spawning, and the handlers revert to their current NHL-per-request behaviour. No data migration to undo.

## Risks

- **Schema drift between NHL and mirror.** The NHL API is unofficial; fields can appear or disappear without notice. Mitigation: store hand-shaped responses in `JSONB` (`series_status`, `carousel`, `roster`); only lift a column into typed storage when a handler queries into it.
- **View performance at scale.** `v_daily_fantasy_totals` scans `nhl_player_game_stats` on every read. Safe at current scale (a few leagues, <30 teams, <500 players on a slate). If the user base grows beyond that, materialize the view into a table and have the live poller write updates inline.
- **Multi-replica safety.** The pollers assume single-instance deployment. If the service ever goes multi-machine, wrap poller bodies in a leader-election primitive (Postgres advisory lock, for instance) before enabling the second replica — otherwise every poller runs N times per tick.
- **Historical form data.** `nhl_player_game_stats` starts empty. For players whose "last 5 games" includes dates before the first deploy, handlers fall back to the existing `playoff_skater_game_stats` table (owned by the nightly ingest). Document this fallback in the handler comment; remove the fallback once the table has accumulated 5 games of history.

## What changes for the user

- Pulse, Rankings daily, Stats, and Sleepers all reflect live in-game points within 60 seconds.
- The blank-sidebar bug in Insights disappears. Pre-game matchups are captured at 10:00 UTC (well before any puck drop) and never overwritten.
- Games-page scores update live without the opt-in polling checkbox doing the work client-side (the server is now the source of truth for live state).
- "Generating insights" is instant on every visit after the first render of the day.
- The NHL API sees approximately `(2N + 5)` calls per minute during live slates (N live games + metadata keep-alive), regardless of how many users are on the site.

## Verification

Post-deploy checklist:

1. `make cache-clear` to start from a cold state. Tail backend logs. Expect `live_mirror: tick` and `nhl_mirror: meta tick` lines within a minute.
2. After 5 minutes, confirm `nhl_games`, `nhl_skater_season_stats`, `nhl_standings`, and `nhl_playoff_bracket` all have rows.
3. Hit every user page in sequence (`/insights`, `/pulse`, `/games/<today>`, `/rankings`, `/race-odds`, `/stats`). Each should render within 200 ms.
4. During a live game: open the Games page, watch `updated_at` on the relevant `nhl_player_game_stats` row tick every 60 s. The UI reflects new points on the next React Query refetch.
5. Open Pulse after a rostered player scores. Confirm "points today" picks up the new value within 60 s.
6. Tail Fly.io logs during a playoff night. Baseline pre-redesign was ~20 429s/hour during live slates; target ≤1/hour (only possible when the metadata poller hits an NHL rate-limit tick).
7. Leave the backend idle for 10 minutes with no users. NHL call rate in logs should be ~5 calls per minute regardless of traffic.

## Applied bandaid fixes

The following changes landed this session as short-term mitigations. The redesign absorbs or obsoletes each one; nothing listed here requires further action.

- Insights cache gate removed; once-per-day response cached regardless of partial landings.
- Per-game pre-game landing cache with write-once semantics (`insights_landing:{game_id}`). The redesign promotes this to the typed `nhl_game_landing` table.
- Games-extended: retry live boxscores once sequentially; derive score from the boxscore when schedule and `get_game_scores` both return null.
- Daily rankings handler: Postgres response cache in front of the boxscore fan-out.
- Playoff roster pool: Postgres cache via `playoff_roster_cache`, refreshed by the 10:00 UTC prewarm.
- `NhlClient` retry budget: 3 linear retries → 5 exponential (~15 s total).
- Frontend rankings default date: yesterday (first date with real data).
- All intervals, timeouts, cron schedules, and cache TTLs centralized in [`backend/src/tuning.rs`](../backend/src/tuning.rs). Frontend counterparts in [`frontend/src/config.ts`](../frontend/src/config.ts) under `QUERY_INTERVALS`.
