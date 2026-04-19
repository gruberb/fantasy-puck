# Database schema

Every table and view, grouped by the concern they serve. Migration files live under [`backend/supabase/migrations/`](../backend/supabase/migrations/) and are applied at boot by `sqlx::migrate!` in [`main.rs:87`](../backend/src/main.rs) (idempotent; every migration uses `CREATE ... IF NOT EXISTS` or explicit `DO $$` guards, so applying an already-migrated database is a no-op).

## Migrations

| File | What it does |
| --- | --- |
| `20260408000000_init_schema.sql` | Creates the fantasy core: `users`, `profiles`, `leagues`, `fantasy_teams`, `league_members`, `fantasy_players`, `fantasy_sleepers`, `draft_sessions`, `player_pool`, `draft_picks`, `daily_rankings`, `response_cache`. Defines `delete_user_account()`. |
| `20260411000000_fix_daily_rankings_constraint.sql` | Corrects the unique constraint on `daily_rankings` from `(team_id, date)` to `(team_id, date, league_id)` - the old shape collided across leagues. |
| `20260418000000_daily_rankings_team_index.sql` | Adds a composite index tuned for Pulse and Insights sparkline reads. |
| `20260418100000_playoff_skater_game_stats.sql` | Per-player per-game playoff fact table. Feeds the player-projection model. |
| `20260418100001_historical_playoff_skater_totals.sql` | Five-year playoff aggregate. Seeded from a bundled CSV; used as the Bayesian shrinkage prior. |
| `20260418100002_playoff_game_results.sql` | Team-level playoff game result. Feeds the playoff Elo update loop. |
| `20260419000000_playoff_roster_cache.sql` | Single JSONB blob per `(season, game_type)` holding 16 playoff rosters so the draft does not fan out to NHL on every cold read. |
| `20260420000000_nhl_mirror.sql` | Creates the eight NHL mirror tables plus `v_daily_fantasy_totals`. This is the big one. |
| `20260420010000_nhl_skater_edge.sql` | Per-skater top speed / top shot speed from NHL Edge; written nightly. |

## Four groups of tables

```
┌─────────────────────────┐   ┌─────────────────────────┐
│       Fantasy state     │   │          Draft          │
│  users, profiles,       │   │  draft_sessions,        │
│  leagues, fantasy_teams │   │  player_pool,           │
│  league_members,        │   │  draft_picks,           │
│  fantasy_players,       │   │  playoff_roster_cache   │
│  fantasy_sleepers,      │   │                         │
│  daily_rankings         │   │                         │
└───────────┬─────────────┘   └────────────┬────────────┘
            │                              │
            ▼                              ▼
     v_daily_fantasy_totals ◄──┐   ┌── player_pool rows converted
                               │   │   to fantasy_players at finalize
                               │   │
┌──────────────────────────────┴───┴─────────┐   ┌─────────────────┐
│               NHL mirror                   │   │      Cache      │
│  nhl_games, nhl_player_game_stats,         │   │  response_cache │
│  nhl_skater_season_stats,                  │   │                 │
│  nhl_goalie_season_stats,                  │   └─────────────────┘
│  nhl_team_rosters, nhl_standings,          │
│  nhl_playoff_bracket, nhl_game_landing,    │
│  nhl_skater_edge,                          │
│  playoff_skater_game_stats,                │
│  playoff_game_results,                     │
│  historical_playoff_skater_totals          │
└────────────────────────────────────────────┘
```

---

## 1. Fantasy state

These tables hold what the app is "about": who the users are, which leagues exist, which teams they drafted, and how those teams have scored day by day.

### `users`
User accounts. Independent of Supabase's `auth.users`.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | `gen_random_uuid()` default |
| `email` | TEXT UNIQUE | Login identifier |
| `password_hash` | TEXT | bcrypt |
| `created_at`, `updated_at` | TIMESTAMPTZ | |

### `profiles`
One-to-one sidecar for display fields that do not belong on `users`.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK, FK `users.id` ON DELETE CASCADE) | |
| `display_name` | TEXT | Defaults to empty string |
| `is_admin` | BOOLEAN | Controls access to `/api/admin/*` |
| `created_at` | TIMESTAMPTZ | |

### `leagues`

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | |
| `name` | TEXT | |
| `season` | TEXT | Defaults `'20252026'`. Frozen at league creation, independent of the global `NHL_SEASON` env var. |
| `visibility` | TEXT | `'public'` or `'private'` |
| `created_by` | UUID (FK `users.id` ON DELETE SET NULL) | |
| `created_at` | TIMESTAMPTZ | |

### `fantasy_teams`

| Column | Type | Notes |
| --- | --- | --- |
| `id` | BIGSERIAL (PK) | Numeric so query strings are short |
| `name` | TEXT | |
| `user_id` | UUID (FK `users.id` ON DELETE CASCADE) | |
| `league_id` | UUID (FK `leagues.id` ON DELETE CASCADE) | |
| `created_at` | TIMESTAMPTZ | |

### `league_members`
Join table between users and leagues, plus the user's draft position in that league.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | |
| `league_id` | UUID (FK) | |
| `user_id` | UUID (FK) | |
| `fantasy_team_id` | BIGINT (FK `fantasy_teams.id` ON DELETE SET NULL) | |
| `draft_order` | INTEGER | |
| `created_at` | TIMESTAMPTZ | |
| Unique | `(league_id, user_id)` | |

### `fantasy_players`
The drafted roster. One row per `(team, player)`.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | BIGSERIAL (PK) | |
| `team_id` | BIGINT (FK `fantasy_teams.id` ON DELETE CASCADE) | |
| `nhl_id` | BIGINT | NHL's player id |
| `name` | TEXT | Denormalized for display |
| `position` | TEXT | |
| `nhl_team` | TEXT | 3-letter abbrev |

### `fantasy_sleepers`
Same shape as `fantasy_players`, but for the sleeper round (see [`08-draft.md`](./08-draft.md)). Unique index on `team_id` enforces one sleeper per team.

### `daily_rankings`
Per-team per-day snapshot. Written by the 09:00 and 15:00 UTC cron jobs (see [`07-background-jobs.md`](./07-background-jobs.md)).

| Column | Type | Notes |
| --- | --- | --- |
| `id` | BIGSERIAL (PK) | |
| `team_id` | BIGINT (FK `fantasy_teams.id` ON DELETE CASCADE) | |
| `league_id` | UUID (FK `leagues.id` ON DELETE CASCADE) | |
| `date` | TEXT | `YYYY-MM-DD` |
| `rank` | INTEGER | 1-based within league on that date |
| `points` | INTEGER | `goals + assists` |
| `goals` | INTEGER | |
| `assists` | INTEGER | |
| `created_at` | TIMESTAMPTZ | |
| Unique | `(team_id, date, league_id)` | |

Index: `idx_daily_rankings_league_date` on `(league_id, date)`. A second index (migration `20260418000000`) is tuned for per-team sparkline reads.

The 09:00 UTC cron also prunes `response_cache` rows older than seven days; it does not prune `daily_rankings` (historical rankings stay forever).

---

## 2. Draft

### `draft_sessions`
One active draft per league at a time; historical drafts remain in the table.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | |
| `league_id` | UUID (FK) | |
| `status` | TEXT | `pending`, `active`, `paused`, `picks_done`, `complete` |
| `current_round` | INTEGER | |
| `current_pick_index` | INTEGER | Global index, 0-based; maps to `(round, slot)` via snake math |
| `total_rounds` | INTEGER | Defaults to 10 |
| `snake_draft` | BOOLEAN | Defaults true |
| `started_at`, `completed_at` | TIMESTAMPTZ | Null until the event |
| `sleeper_status` | TEXT | Separate sub-flow for the sleeper round |
| `sleeper_pick_index` | INTEGER | |
| `created_at` | TIMESTAMPTZ | |

### `player_pool`
Eligible players for one draft session. Repopulated by `POST /api/draft/{id}/populate`. See [`08-draft.md`](./08-draft.md) for how the pool is sourced.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | |
| `draft_session_id` | UUID (FK ON DELETE CASCADE) | |
| `nhl_id` | BIGINT | |
| `name`, `position`, `nhl_team`, `headshot_url` | TEXT | |

### `draft_picks`
One row per pick, both the regular draft and the sleeper round.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | UUID (PK) | |
| `draft_session_id` | UUID (FK ON DELETE CASCADE) | |
| `league_member_id` | UUID (FK ON DELETE CASCADE) | Who picked |
| `player_pool_id` | UUID (FK `player_pool.id` ON DELETE SET NULL) | Ref may go null if pool is repopulated |
| `nhl_id`, `player_name`, `nhl_team`, `position` | TEXT/BIGINT | Denormalized at pick time |
| `round` | INTEGER | 1-based |
| `pick_number` | INTEGER | Global 0-based pick index |
| `picked_at` | TIMESTAMPTZ | |

Finalizing a draft (`POST /api/draft/{id}/finalize`) copies `draft_picks` rows into `fantasy_players`. Picks stay in `draft_picks` for history.

### `playoff_roster_cache`
Cache table, not a mirror. One JSONB blob per `(season, game_type)`:

| Column | Type |
| --- | --- |
| `season` | INTEGER (PK part) |
| `game_type` | SMALLINT (PK part) |
| `rosters` | JSONB |
| `updated_at` | TIMESTAMPTZ |

Refreshed daily by the 10:00 UTC prewarm job. Reads are one row per draft-pool populate; misses fall back to a 16-team NHL fan-out. See [`infra/jobs/player_pool.rs`](../backend/src/infra/jobs/player_pool.rs).

---

## 3. NHL mirror

This is the read-through cache of the NHL API that lives in Postgres. Background pollers populate it; user-facing handlers read from it. See [`04-nhl-integration.md`](./04-nhl-integration.md) for which poller owns which table.

### `nhl_games`
Schedule + live state + final score + series context, one row per game.

| Column | Type | Notes |
| --- | --- | --- |
| `game_id` | BIGINT (PK) | NHL's game id |
| `season` | INTEGER | |
| `game_type` | SMALLINT | 2 regular, 3 playoffs |
| `game_date` | DATE | Eastern Time date |
| `start_time_utc` | TIMESTAMPTZ | |
| `game_state` | TEXT | `FUT` → `PRE` → `LIVE` → `CRIT` → `OFF` → `FINAL` |
| `home_team`, `away_team` | TEXT | |
| `home_score`, `away_score` | INTEGER nullable | Filled in by the live poller |
| `period_number` | SMALLINT nullable | `1`–`3` regulation, `4`+ playoff OT. Live poller writes it straight from `periodDescriptor.number` — do not stuff a composite label here. |
| `period_type` | TEXT nullable | Raw upstream label: `REG`, `OT`, `SO`. The API handler (`format_period` in `handlers/games.rs`) maps to a human string at render time. |
| `series_status` | JSONB | Playoffs only |
| `venue` | TEXT | |
| `updated_at` | TIMESTAMPTZ | |

Indexes: `idx_nhl_games_date`, `idx_nhl_games_state`.

### `nhl_player_game_stats`
Per-player per-game box score. This is the table that `v_daily_fantasy_totals` joins against.

| Column | Type | Notes |
| --- | --- | --- |
| `game_id` | BIGINT | FK-by-convention into `nhl_games` |
| `player_id` | BIGINT | |
| `team_abbrev`, `position`, `name` | TEXT | |
| `goals`, `assists`, `points` | INTEGER | |
| `sog`, `pim`, `plus_minus`, `hits`, `toi_seconds` | INTEGER nullable | |
| `updated_at` | TIMESTAMPTZ | |
| PK | `(game_id, player_id)` | |

Indexes: `idx_npgs_player`, `idx_npgs_team`.

### `nhl_skater_season_stats`
Season leaderboard mirror. One row per `(player_id, season, game_type)`.

| Column | Type | Notes |
| --- | --- | --- |
| `player_id`, `season`, `game_type` | composite PK | |
| `first_name`, `last_name`, `team_abbrev`, `position` | TEXT | |
| `goals`, `assists`, `points`, `plus_minus`, `sog` | INTEGER | |
| `faceoff_pct`, `toi_per_game` | REAL / INTEGER | |
| `headshot_url` | TEXT | |
| `updated_at` | TIMESTAMPTZ | |

Index: `idx_nsss_season_gt_points` on `(season, game_type, points DESC)` for top-N reads.

### `nhl_goalie_season_stats`
Narrow mirror - only the fields that show up in the Pulse matchup block and the goalie Elo bonus.

| Column | Type | Notes |
| --- | --- | --- |
| `player_id`, `season`, `game_type` | composite PK | |
| `team_abbrev`, `name`, `record` | TEXT | |
| `gaa`, `save_pctg` | REAL | |
| `shutouts` | INTEGER | |

### `nhl_team_rosters`
Current roster per team. JSONB column holds the raw NHL roster shape.

| Column | Type |
| --- | --- |
| `team_abbrev`, `season` | composite PK |
| `roster` | JSONB |
| `updated_at` | TIMESTAMPTZ |

Refreshed every 288 meta-poll ticks (≈24 h) and explicitly during the 10:00 UTC prewarm.

### `nhl_standings`
One row per team per season.

| Column | Type | Notes |
| --- | --- | --- |
| `season`, `team_abbrev` | composite PK | |
| `points`, `games_played`, `wins`, `losses`, `ot_losses` | INTEGER | |
| `point_pctg` | REAL | |
| `streak_code`, `streak_count` | TEXT/INTEGER | `W7`, `L2`, etc. |
| `l10_wins`, `l10_losses`, `l10_ot_losses` | INTEGER | Last-ten, frozen once the regular season ends |
| `updated_at` | TIMESTAMPTZ | |

### `nhl_playoff_bracket`
One row per season. The `carousel` JSONB column stores the raw NHL carousel feed (the tree shape with series, rounds, and scores).

### `nhl_game_landing`
Pre-game matchup block per game, captured once while the game is `FUT`. Write-once semantics: `captured_at` is set on insert and never updated.

### `nhl_skater_edge`
Edge telemetry for the top-cohort skaters.

| Column | Type |
| --- | --- |
| `player_id` | BIGINT (PK) |
| `top_speed_mph`, `top_shot_speed_mph` | REAL nullable |
| `updated_at` | TIMESTAMPTZ |

### `playoff_skater_game_stats`
Per-player per-game playoff fact table (separate from `nhl_player_game_stats` because it is written by the nightly ingest in a different shape and serves a different consumer - the player projection model).

| Column | Type | Notes |
| --- | --- | --- |
| `season`, `game_type`, `game_id`, `player_id` | Natural key components | `(game_id, player_id)` is PK |
| `game_date` | DATE | |
| `team_abbrev`, `opponent` | TEXT | |
| `home` | BOOLEAN | |
| `goals`, `assists`, `points`, `shots`, `pp_points`, `toi_seconds` | INTEGER nullable | |

Indexes for the three read patterns: per-player descending date (recency weighting), per-team per-game (ingest walks), per-date (backtest harness).

### `historical_playoff_skater_totals`
Five-year aggregated playoff totals per skater. Natural key is `(player_name, born)` so the two Sebastian Ahos resolve without needing an NHL player id.

| Column | Type |
| --- | --- |
| `player_name`, `born` | composite PK |
| `team`, `position` | TEXT |
| `gp`, `g`, `a`, `p` | INTEGER |
| `shots`, `toi_seconds` | INTEGER nullable |

Seeded once from a CSV bundled into the binary; see [`infra/jobs/historical_seed.rs`](../backend/src/infra/jobs/historical_seed.rs). The projection model uses this as a regression-to-mean anchor (see [`05-prediction-engine.md`](./05-prediction-engine.md)).

### `playoff_game_results`
Team-level playoff result. Chronological replay fuels the playoff Elo loop.

| Column | Type | Notes |
| --- | --- | --- |
| `season`, `game_type` | INTEGER/SMALLINT | |
| `game_id` | BIGINT (PK) | |
| `game_date` | DATE | |
| `home_team`, `away_team`, `winner` | TEXT | |
| `home_score`, `away_score` | INTEGER | |
| `round` | SMALLINT nullable | 1–4 |

Indexes on `(game_date, game_id)` for chronological replay and `(home_team | away_team, game_date)` for per-team lookups.

---

## 4. Cache

### `response_cache`
Keyed JSON blobs. Readers and writers are in [`infra/db/cache.rs`](../backend/src/infra/db/cache.rs).

| Column | Type | Notes |
| --- | --- | --- |
| `cache_key` | TEXT (PK) | Free-form; prefixes used for bulk invalidation |
| `data` | TEXT | JSON payload |
| `date` | TEXT nullable | `YYYY-MM-DD` for date-scoped invalidation |
| `created_at`, `last_updated` | TEXT | ISO strings |

Cache-key patterns in use:

| Prefix | Written by | Invalidated by |
| --- | --- | --- |
| `match_day:{date}` | `handlers/games.rs` | `/api/admin/cache/invalidate?scope={date}` |
| `insights:...` | `handlers/insights.rs` | Daily prewarm rewrites; operator prewarm forces |
| `race_odds:...` | `handlers/race_odds.rs` | Same as insights |
| `pulse_narrative:{league_id}:{game_id}` | `handlers/pulse.rs` | Live poller on `LIVE → OFF/FINAL` transition |

Retention: the 09:00 UTC cron deletes rows whose `date` is older than seven days ([`scheduler.rs:256-267`](../backend/src/infra/jobs/scheduler.rs)).

---

## The `v_daily_fantasy_totals` view

Defined at [`20260420000000_nhl_mirror.sql:172-186`](../backend/supabase/migrations/20260420000000_nhl_mirror.sql):

```sql
CREATE OR REPLACE VIEW public.v_daily_fantasy_totals AS
SELECT
    ft.league_id,
    ft.id       AS team_id,
    ft.name     AS team_name,
    g.game_date AS date,
    COALESCE(SUM(pgs.goals), 0)   AS goals,
    COALESCE(SUM(pgs.assists), 0) AS assists,
    COALESCE(SUM(pgs.points), 0)  AS points
FROM public.nhl_player_game_stats pgs
JOIN public.nhl_games g           ON g.game_id = pgs.game_id
JOIN public.fantasy_players fp    ON fp.nhl_id = pgs.player_id
JOIN public.fantasy_teams   ft    ON ft.id = fp.team_id
GROUP BY ft.league_id, ft.id, ft.name, g.game_date;
```

This is the one place where the fantasy and NHL sides meet. Every team's "points today" comes from this view: the live poller writes `nhl_player_game_stats` as goals are credited, and the view recomputes on every read. At current scale (a handful of leagues, fewer than thirty teams each) the view is cheap enough to call on every Pulse request; the 09:00 and 15:00 UTC crons freeze yesterday's row into `daily_rankings` for stable history.

## Advisory locks

Not rows, but worth documenting alongside the schema. [`infra/db/nhl_mirror.rs`](../backend/src/infra/db/nhl_mirror.rs) uses two Postgres advisory locks to coordinate pollers across replicas:

| Lock key | Purpose | Functions |
| --- | --- | --- |
| `884_471_193_001` | Meta-poller leader election | `try_meta_lock`, `release_meta_lock` |
| `884_471_193_002` | Live-poller leader election | `try_live_lock`, `release_live_lock` |

A replica that cannot acquire its lock skips the tick entirely. Only one replica does the work; the others are quiet on the poller path.
