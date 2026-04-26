# Business logic

What the app is actually computing for its users: fantasy points, daily rankings, and the path from a live NHL event to a number on a dashboard.

## Fantasy scoring rule

Goals plus assists. No weights for shots, plus-minus, TOI, or any other stat.

The rule lives in one place, [`backend/src/domain/services/fantasy_points.rs:20`](../backend/src/domain/services/fantasy_points.rs):

```rust
let (goals, assists) =
    find_player_stats_by_name(boxscore, &player.nhl_team, &player.player_name, Some(player.nhl_id));
let points = goals + assists;
```

`process_game_performances` ([`fantasy_points.rs:5-51`](../backend/src/domain/services/fantasy_points.rs)) takes a list of fantasy teams and one NHL boxscore, resolves every rostered player's stats in that boxscore, and produces a `TeamDailyPerformance` per team. Teams with zero points are filtered out.

Goalies are not drafted, so the rule never applies to a goalie. See the [No Goalies in Fantasy Format](../../.claude/projects/-Users-bastian-CodingIsFun-fun-fantasy-puck/memory/project_no_goalies.md) memory and the [`CLAUDE.md`](../CLAUDE.md) under "Player-pool sourcing".

## League lifecycle

```
           create                          join                            draft                             score                          rank
POST           POST                           POST → pick → pick → …         (background crons)              (derived from rankings)
/api/leagues   /api/leagues/{id}/join         /api/leagues/{id}/draft          09:00/15:00 UTC                per request
           → fantasy_teams row              → draft_sessions row          v_daily_fantasy_totals view      GET /api/fantasy/rankings
           → league_members row               → player_pool rows            read by Pulse live            reads daily_rankings
                                              → draft_picks rows           → daily_rankings snapshot      or sums nhl_player_game_stats
                                              → fantasy_players (on finalize)
```

Table by table, what each step writes:

| Step | Endpoint | Tables affected |
| --- | --- | --- |
| Create league | `POST /api/leagues` | `leagues` |
| Join league | `POST /api/leagues/{id}/join` | `fantasy_teams`, `league_members` |
| Create draft | `POST /api/leagues/{id}/draft` | `draft_sessions`, `player_pool` |
| Pick | `POST /api/draft/{id}/pick` | `draft_picks` |
| Finalize | `POST /api/draft/{id}/finalize` | `fantasy_players` (copy-out), `draft_sessions.sleeper_status = 'active'` |
| Sleeper pick | `POST /api/draft/{id}/sleeper/pick` | `fantasy_sleepers`, `draft_picks` |
| Score (daily cron) | 09:00 / 15:00 UTC | `daily_rankings` |
| Score (live) | Read-only | `v_daily_fantasy_totals` (view, recomputes on every SELECT) |

Draft details are in [`08-draft.md`](./08-draft.md). Cron details are in [`07-background-jobs.md`](./07-background-jobs.md).

## Live data flow during a game

A walk through what happens between a goal being scored in an NHL arena and the user seeing the updated number on the Pulse page.

```
┌───────────────────────────┐
│  NHL arena                │
│  goal scored at t=0       │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│  NHL boxscore endpoint    │
│  api-web.nhle.com         │
│  updated within seconds   │
└────────────┬──────────────┘
             │
             ▼    next tick: ≤60 s later
┌───────────────────────────┐
│  live_poller tick         │
│  fetches boxscore,        │
│  upserts                  │
│  nhl_player_game_stats    │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│  v_daily_fantasy_totals   │
│  recomputes on every      │
│  read (joins mirror       │
│  against fantasy_players) │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│  GET /api/pulse           │
│  sums goals+assists for   │
│  the user's team, today   │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│  frontend React Query     │
│  staleTime = 60 s         │
│  (PULSE_STALE_MS)         │
└────────────┬──────────────┘
             │
             ▼
         User sees
       updated score
      0–120 s after goal
```

### Step-by-step

1. **`t = 0`** - a goal is credited in the NHL arena. Within seconds, the NHL's boxscore endpoint for that `game_id` returns updated skater lines.
2. **`t ≤ 60 s`** - the live poller ticks (every 60 s). If it acquires the advisory lock, it lists live games and calls [`poll_one_game`](../backend/src/infra/jobs/live_poller.rs#L112) for each. The function:
   - Snapshots previous `game_state` from the mirror.
   - Fetches the boxscore via `NhlClient::get_game_boxscore` (its 60-s TTL means one NHL call per minute even if multiple handlers want the same boxscore).
   - Calls `nhl_mirror::upsert_boxscore_players`, which writes one row per player into `nhl_player_game_stats`.
   - Fetches the `game_data` block for state + score + period, calls `update_game_live_state`.
3. **`t ≤ 60 s`** - the view `v_daily_fantasy_totals` recomputes on next read. It joins `nhl_player_game_stats` against `fantasy_players` and groups by `(league_id, team_id, date)`.
4. **User refreshes Pulse or React Query's 60-s staleTime expires** - `GET /api/pulse?league_id=...`. The handler reads from the mirror (`list_games_for_date`, `get_playoff_carousel`, `v_daily_fantasy_totals`) and composes the payload. No NHL calls on the request path.
5. **Total visible delay**: 0–120 s. Worst case: the poller just missed its tick (near 60 s), then the user just missed the React Query refetch window (another 60 s).

### Cache invalidation on game end

The live poller detects the `LIVE|CRIT → OFF|FINAL` state transition once per game ([`live_poller.rs:172-210`](../backend/src/infra/jobs/live_poller.rs)) and invalidates only the `:v2` narrative tail of `team_diagnosis:{league}:*` for every league that had a rostered player in that game:

```rust
if was_live && is_final {
    let leagues = nhl_mirror::list_leagues_with_player_in_game(pool, game_id).await?;
    for league_id in &leagues {
        let pattern = format!("team_diagnosis:{}:%:v2", league_id);
        cache.invalidate_by_like(&pattern).await?;
    }
}
```

**Scores do not need to be invalidated.** They live in the mirror and are always fresh. Only the Claude-generated narrative text, which names the game and references in-progress stats, gets regenerated on the next Pulse visit. That's the expensive bit worth caching ([`handlers/pulse.rs:1-21`](../backend/src/api/handlers/pulse.rs)).

The sibling `team_diagnosis:{league}:{team}:{season}:{gt}:{date}:bundle:v1` payload is **not** wiped on this transition. The bundle's projections, grades, recent-games rollup, and yesterday recap are stable through the evening, so wiping it would force every Pulse load until the next prewarm to re-run `compose_team_breakdown` (seven batched DB reads + projection fold) just to surface the same numbers. The bundle ages out naturally on the date roll, and the daily prewarm at 10:00 UTC rebuilds it with the freshly regenerated narrative nested inside.

### What Pulse reads

From [`handlers/pulse.rs`](../backend/src/api/handlers/pulse.rs):

| Source | Data | Cached? |
| --- | --- | --- |
| `db.get_all_teams_with_players(league_id)` | Fantasy rosters | No |
| `nhl_mirror::get_playoff_carousel(pool, season)` | Playoff bracket shape | No (mirror is itself a cache) |
| `nhl_mirror::list_games_for_date(pool, today)` | Today's games (also surfaced as `PulseResponse.games_today` for the dashboard's Live Rankings section) | No |
| `nhl_mirror::list_games_for_date(pool, yesterday)` + `list_league_player_stats_for_date(pool, league_id, yesterday)` | Previous hockey-date recap for the caller's roster and league top performers | No |
| `nhl_mirror::list_team_daily_totals(pool, league_id, today)` | `v_daily_fantasy_totals` sum per team | No |
| `nhl_mirror::list_league_team_season_totals(..., current_date_window())` | Season-to-date totals, clamped to `[playoff_start, season_end]` in playoff mode | No |
| Cached `race_odds:v4:*` payload | `nhl_team_cup_odds: HashMap<String, f32>` for the narrator — best-effort, empty if the morning cron hasn't warmed | Yes (reused, not regenerated) |
| `state.prediction.team_diagnosis(...)` via `response_cache` | Structured "Your Read" narrative (`### Yesterday` / `### Where You Stand` / `### Player-by-Player` / `### What to Expect`) | Yes - `team_diagnosis:{league}:{team}:{season}:{gt}:{date}:v2` |
| `compose_team_breakdown(...)` result via `response_cache` | The full caller-specific breakdown (per-player projections, grades, recent games, yesterday recap, narrative) returned as `MyTeamDiagnosis` | Yes - `team_diagnosis:{league}:{team}:{season}:{gt}:{date}:bundle:v1` |

Everything except the narrative, the race-odds cross-read, and the per-caller breakdown bundle is recomputed on every request. The data sizes are small enough (one league × ~10 teams × ~30 players × a few live games) that this stays in the single-digit millisecond range. The bundle is what keeps the request path off Claude during the playoffs — a warm bundle turns the Pulse "Your Read" block into one SELECT.

### Playoff window clamping

Aggregations across date-keyed history (daily wins/top-3, season totals) are scoped to the active window via `crate::api::current_date_window()` — returns `DateWindow::between(playoff_start, season_end)` in playoff mode, unbounded otherwise. Without this, `daily_rankings` rows from the regular season bled into the playoff Season Overview, and `list_league_team_season_totals` let off-mode stats into the SUMs. The helper is consumed by `team_stats`, `rankings`, `pulse`, and `race_odds` handlers; the per-date query `list_live_game_ids_for_date` is kept untouched because `process_daily_rankings` wants strict per-date semantics for its safety gate.

## Daily-rankings snapshot

At 09:00 and 15:00 UTC, the scheduler locks in *yesterday's* totals. See [`07-background-jobs.md`](./07-background-jobs.md) for the cron definitions; the function is [`process_daily_rankings`](../backend/src/infra/jobs/scheduler.rs#L20) in `scheduler.rs:19-99`.

```sql
SELECT team_id, goals::int, assists::int, points::int
  FROM v_daily_fantasy_totals
 WHERE league_id = $1::uuid
   AND date      = $2::date
   AND points > 0
 ORDER BY points DESC, team_id
```

For each row (ordered by points descending), insert/update `daily_rankings`:

```sql
INSERT INTO daily_rankings (date, team_id, league_id, rank, points, goals, assists)
VALUES ($1, $2, $3::uuid, $4, $5, $6, $7)
ON CONFLICT (team_id, date, league_id) DO UPDATE SET
    rank = EXCLUDED.rank, ...
```

The **safety gate** (`scheduler.rs:38-46`) skips the write if any game on that date is still `LIVE` / `CRIT` / `PRE`. It almost never triggers during the scheduled runs (they operate on yesterday), but it matters for the manual admin trigger `GET /api/admin/process-rankings/{date}` which can be fired against today.

The 15:00 UTC run exists as a safety net for late-published NHL boxscores. The upsert means re-running does the right thing.

## Why two different data stores for "today's points"

- **Live:** `v_daily_fantasy_totals` - recomputes from `nhl_player_game_stats` on every SELECT. Reflects the current minute's state while games are in progress.
- **Historical:** `daily_rankings` - one row per `(team_id, date, league_id)` with a frozen rank. Used for charts, sparklines, and "days ago" reads that should not churn.

The Pulse page reads from the view. The Rankings page and the sparkline widget read from `daily_rankings`. One place where both are involved is [`TeamDbService::get_team_sparklines_with_live`](../backend/src/infra/db/teams.rs): it unions `daily_rankings` for the last N-1 days with `v_daily_fantasy_totals` for today, so the trailing point on the chart is always the live value.

## Points reflection timing - summary

| Event | Visible in `nhl_player_game_stats` | Visible in `v_daily_fantasy_totals` | Visible in `daily_rankings` |
| --- | --- | --- | --- |
| Goal scored, game in progress | ≤ 60 s (live poller) | Same tick | Next day's 09:00/15:00 UTC cron |
| Goal scored, game ends this tick | ≤ 60 s | Same tick | Same as above; the state-transition detection fires invalidation but does not write `daily_rankings` |
| Game finalized before process boot | After auto-seed rehydrate (~45 s after boot) | After rehydrate | After cron or manual `/api/admin/process-rankings/{date}` |

## What the handlers never do

- Handlers do not call `NhlClient::get_game_boxscore` directly for live points. That is the live poller's job. Handlers read the mirror.
- Handlers do not write to `nhl_player_game_stats` or other mirror tables (the admin rehydrate endpoint calls the job module, not SQL directly).
- Handlers do not freeze `daily_rankings`. That is the cron's job, explicitly gated on "no game still in progress".

Keeping these responsibilities separated is what makes the cache invalidation correct: the live poller owns state transitions, so it alone can reliably fire the right invalidation at the right moment.
