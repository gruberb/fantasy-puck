# Operations Guide

Operational runbook for the Fantasy Puck monorepo. Covers deployment, configuration, database management, scheduled jobs, cache, monitoring, season changeover, and troubleshooting.

---

## Architecture Overview

```
                     +-----------------------+
   Users ----------> | fantasy-frontend      |  Fly.io (yyz)
                     | nginx serving Vite    |  Port 80
                     | fantasy-puck.ca       |
                     +----------+------------+
                                |
                     +----------v------------+
                     | fantasy-hockey        |  Fly.io (yyz)
                     | Rust / Axum           |  Port 3000
                     | api.fantasy-puck.ca   |
                     +---+------+------+-----+
                         |      |      |
              +----------+  +---+---+  +----------+
              | Supabase |  | NHL   |  | Anthropic|
              | Postgres |  | API   |  | API      |
              +----------+  +-------+  +----------+
```

- **Frontend:** Static SPA built with Vite, served by nginx. Fly app name: `fantasy-frontend`.
- **Backend:** Rust binary with embedded scheduler, WebSocket support, and in-memory NHL API cache. Fly app name: `fantasy-hockey`.
- **Database:** Supabase-hosted PostgreSQL (session pooler on port 5432).
- **External APIs:** NHL API (`api-web.nhle.com`), Anthropic Messages API (for insights narratives), DailyFaceoff (web scraping for news headlines).

---

## Deployment

### Backend (`fantasy-hockey`)

```bash
cd backend/
fly deploy
```

The `Dockerfile` uses a multi-stage build:
1. `rust:1.88` builder compiles a release binary.
2. `debian:bookworm-slim` runtime with just `libssl3` and `ca-certificates`.

The binary starts with `./fantasy-hockey serve --port 3000`.

Configuration in `fly.toml`:
- Region: `yyz` (Toronto)
- Internal port: 3000
- HTTPS forced, auto-stop off, min 1 machine running
- VM: 1 shared CPU, 1GB memory

### Frontend (`fantasy-frontend`)

```bash
cd frontend/
fly deploy
```

The `Dockerfile` uses a multi-stage build:
1. `node:20.18.0-slim` builds the Vite app (`npm ci && npm run build`).
2. `nginx` serves the built static files from `/usr/share/nginx/html`.

The `VITE_API_URL` must be set as a build argument or in a `.env` file before building, since Vite inlines environment variables at build time.

Configuration in `fly.toml`:
- Region: `yyz`
- Internal port: 80
- HTTPS forced, auto-stop off, min 1 machine running

### Deployment Order

Deploy backend first when making breaking API changes. Frontend can be deployed independently for UI-only changes.

---

## Environment Variables

### Backend (runtime secrets on Fly.io)

| Variable | Required | Description |
|---|---|---|
| `DATABASE_URL` | Yes | Supabase PostgreSQL connection string. Use the **session pooler** URL (port 5432) which supports prepared statements. Example: `postgresql://postgres.xxxxx:password@aws-0-ca-central-1.pooler.supabase.com:5432/postgres` |
| `JWT_SECRET` | Yes | Secret key for signing and verifying JWT tokens. Must match between backend deployments. |
| `ANTHROPIC_API_KEY` | No | Anthropic API key for generating AI-powered insights narratives. If not set, insights fall back to a static "Unable to generate insights" message. |

Set with:
```bash
fly secrets set DATABASE_URL="..." JWT_SECRET="..." ANTHROPIC_API_KEY="..." --app fantasy-hockey
```

### Frontend (build-time variables)

| Variable | Required | Description |
|---|---|---|
| `VITE_API_URL` | No | Backend API base URL. Defaults to `https://api.fantasy-puck.ca/api`. Set for local dev to `http://localhost:3000/api`. |

For local development, create `frontend/.env.local`:
```
VITE_API_URL=http://localhost:3000/api
```

For production builds on Fly.io, the default is baked into `frontend/src/config.ts`.

---

## Database

### Supabase Setup

The database is hosted on Supabase. The schema is defined in `backend/migrations/001_create_users_table.sql` and includes:

- `users` / `profiles` -- authentication (custom JWT, not Supabase Auth)
- `leagues` / `league_members` -- multi-league support
- `fantasy_teams` / `fantasy_players` / `fantasy_sleepers` -- rosters
- `draft_sessions` / `player_pool` / `draft_picks` -- draft state
- `daily_rankings` -- historical ranking snapshots
- `response_cache` -- server-side response cache

### Connection Pooling

The backend uses Supabase's **session pooler** (port 5432) rather than the transaction pooler (port 6543). This is configured in `backend/src/db/mod.rs`:

```rust
let connect_options = PgConnectOptions::from_str(db_url)?
    .statement_cache_capacity(0);  // Disable prepared statement cache for PgBouncer compatibility

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect_with(connect_options)
    .await?;
```

Key notes:
- `statement_cache_capacity(0)` is set for PgBouncer compatibility even with the session pooler.
- Max connections is 5 (Supabase free tier allows ~20 direct connections).
- If you see `prepared statement "sqlx_..." already exists` errors, the pooler mode is likely wrong -- ensure you're using the session pooler URL.

### Migrations

Migrations are not auto-run. The single migration file creates all tables idempotently with `CREATE TABLE IF NOT EXISTS`. To apply:

```bash
psql "$DATABASE_URL" -f backend/migrations/001_create_users_table.sql
```

Or connect through the Supabase SQL editor.

---

## Scheduler

The backend runs an in-process cron scheduler (`tokio-cron-scheduler`) initialized in `backend/src/utils/scheduler.rs`.

### Scheduled Jobs

| Job | Schedule (UTC) | Description |
|---|---|---|
| Morning rankings | `0 9 * * * *` (9:00 AM) | Processes yesterday's daily rankings for all leagues. Fetches NHL schedule + boxscores, calculates fantasy points per team, stores in `daily_rankings`. |
| Afternoon rankings | `0 15 * * * *` (3:00 PM) | Same as morning -- a second pass to catch late-finishing games (West Coast late games finish ~7 AM UTC). |
| Insights pre-warming | `0 10 * * * *` (10:00 AM) | Generates and caches insights (including Claude API calls) for all leagues so they're ready when users visit. |

### Startup Backfill

On first start after playoffs begin (when `daily_rankings` table is empty), the server runs `populate_historical_rankings` synchronously before starting the HTTP listener. This iterates from playoff start date to today, processing each date for each league.

**Warning:** This blocks the server from accepting connections and can take several minutes. See Known Issues #5.

### Verifying the Scheduler

Check Fly.io logs for these messages:
```
Scheduler initialized: rankings at 9am/3pm UTC, insights at 10am UTC
```

After a job runs, you should see:
```
Processing daily rankings for date: 2026-04-07, league: <uuid>
Successfully stored daily rankings for date: 2026-04-07, league: <uuid>
```

For insights:
```
Running daily insights pre-warming job
Pre-warmed global insights
Pre-warmed insights for league <uuid>
```

Query the database to verify rankings are being stored:
```sql
SELECT date, COUNT(*) FROM daily_rankings GROUP BY date ORDER BY date DESC LIMIT 10;
```

---

## Cache Management

### Two-Layer Cache

The app has two independent caching layers:

1. **In-memory NHL API cache** (`NhlClient.cache`): HashMap of URL -> response body with per-endpoint TTLs (60s to 24hr). Cleaned up every 5 minutes by a background task (`start_cache_cleanup`). Lives only in the backend process.

2. **Database response cache** (`response_cache` table): Stores expensive computed responses (match-day data, insights) as JSON text. No automatic TTL -- entries persist until explicitly invalidated.

### Cache TTLs (in-memory)

| Endpoint Type | TTL |
|---|---|
| Boxscore (live game) | 1 min |
| Schedule, Game Center, Scores | 2 min |
| Skater Stats | 5 min |
| Player Game Log | 10 min |
| Playoff Carousel | 15 min |
| Player Details, Standings, Roster, Edge | 30 min |
| Boxscore (final game) | 24 hr |

### Admin Invalidation Endpoints

**Warning:** These endpoints currently have no authentication. See Known Issues #1.

**Invalidate all caches (DB + in-memory):**
```bash
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=all"
```

**Invalidate today's DB cache only:**
```bash
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=today"
```

**Invalidate a specific date's DB cache:**
```bash
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=2026-04-07"
```

**Invalidate match-day cache only (default, no scope param):**
```bash
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate"
```

**Trigger ranking reprocessing for a specific date:**
```bash
curl "https://api.fantasy-puck.ca/api/admin/process-rankings/2026-04-07"
```

### When to Invalidate

- **After a player trade/roster change:** Invalidate all (`scope=all`) so stale roster data is refreshed.
- **If insights show stale data:** Invalidate today's cache (`scope=today`), which removes the cached insights response and forces regeneration on next request.
- **If match-day scores seem stuck:** Invalidate the default match-day cache (no scope). The in-memory NHL API cache also has a 1-2 minute TTL for live data, so it self-corrects quickly.
- **After fixing a rankings bug:** Use `process-rankings/{date}` to reprocess a specific date.

---

## Monitoring

### Fly.io Logs

```bash
fly logs --app fantasy-hockey
fly logs --app fantasy-frontend
```

### Common Log Patterns

**Healthy operation:**
```
Starting fantasy hockey application
Starting web server on port 3000
Scheduler initialized: rankings at 9am/3pm UTC, insights at 10am UTC
```

**NHL API issues:**
```
NHL API rate limited (429), retrying in 500ms...
NHL API returned status 404: ...
```
The client retries up to 3 times on 429 with exponential backoff (500ms, 1s, 1.5s).

**Database connectivity:**
```
Database error: pool timed out while waiting for an open connection
```
Indicates max connections (5) are exhausted. Check for slow queries or connection leaks.

**WebSocket activity:**
```
WebSocket connected for draft session <uuid>
WebSocket disconnected for draft session <uuid>
WebSocket client lagged by N messages for session <uuid>
```
Lagged messages indicate a slow client -- the broadcast channel auto-skips missed messages.

**Insights generation:**
```
Failed to generate narratives: ANTHROPIC_API_KEY not set
Failed to generate narratives: Claude API returned 429: ...
```
The app falls back to a static message when the Claude API is unavailable.

**Scraping failures** are silent (see Known Issues #8). If insights have no news headlines, check if DailyFaceoff is accessible and if their HTML structure has changed.

### What to Watch

- **Rankings cron:** Verify `Successfully stored daily rankings` appears at 9 AM and 3 PM UTC.
- **NHL API errors:** Spikes in 429/502 errors indicate rate limiting or API downtime.
- **Memory usage:** The in-memory cache can grow during heavy game days. The cleanup task runs every 5 minutes. Monitor via `fly status`.
- **Database cache growth:** The `response_cache` table grows indefinitely. Periodically check:
  ```sql
  SELECT COUNT(*), pg_size_pretty(pg_total_relation_size('response_cache')) FROM response_cache;
  ```

---

## Season Changeover

At the start of each new NHL season/playoff, update the following constants:

### Backend

| File | Line | Constant | Example |
|---|---|---|---|
| `backend/src/api/mod.rs` | 15 | `SEASON` | `20262027` |
| `backend/src/api/mod.rs` | 16 | `GAME_TYPE` | `2` (regular season) or `3` (playoffs) |
| `backend/src/main.rs` | 46 | `playoff_start` | `"2027-04-18"` |
| `backend/src/main.rs` | 52 | end date cap | `"2027-06-15"` |

### Frontend

| File | Line | Constant | Example |
|---|---|---|---|
| `frontend/src/config.ts` | 9 | `DEFAULT_SEASON` | `"20262027"` |
| `frontend/src/config.ts` | 10 | `DEFAULT_GAME_TYPE` | `3` (playoffs) or `2` (regular season) |

### Database

- Truncate the `daily_rankings` table (or let the startup backfill repopulate it).
- Clear the `response_cache` table: `DELETE FROM response_cache;`
- Optionally create new leagues for the new season.

### Checklist

1. Update `SEASON` and `GAME_TYPE` in both backend and frontend.
2. Update `playoff_start` and end date in `main.rs`.
3. Deploy backend first, then frontend.
4. Invalidate all caches: `curl "...api/admin/cache/invalidate?scope=all"`
5. Verify the scheduler logs show the new season's data being fetched.

---

## Troubleshooting

### Stale insights

**Symptoms:** Insights page shows yesterday's data, or narratives reference old games.

**Cause:** Insights are cached in the `response_cache` table with a key format `insights:{league_id}:{date}`. If the scheduler job at 10 AM UTC ran but the date calculation was off (see DST issue), or if the Claude API failed silently, the cache may contain stale or fallback data.

**Fix:**
1. Invalidate today's cache: `curl "...api/admin/cache/invalidate?scope=today"`
2. Revisit the insights page -- it will regenerate on the next request.
3. Check logs for `Failed to generate narratives` errors.

### Missing rankings for a date

**Symptoms:** The rankings chart has gaps, or a specific date shows no data.

**Cause:** The cron job may have run before West Coast games finished (unlikely with the 9 AM/3 PM UTC schedule), or the NHL API returned an error for that date's boxscores.

**Fix:**
1. Manually trigger reprocessing: `curl "...api/admin/process-rankings/2026-04-07"`
2. Check logs for `Failed to process rankings` errors.
3. Verify the date had games: `curl "https://api-web.nhle.com/v1/schedule/2026-04-07"`.

### Draft stuck / picks not appearing

**Symptoms:** The draft page shows the wrong current picker, picks don't appear in real time, or the page is unresponsive.

**Cause:** WebSocket disconnection (no heartbeat -- see Known Issues #16), stale React Query cache, or backend state mismatch.

**Fix:**
1. Hard refresh the page (Cmd+Shift+R) to re-establish WebSocket connections.
2. If the draft is genuinely stuck at the backend level, check the `draft_sessions` table:
   ```sql
   SELECT id, status, current_round, current_pick_index FROM draft_sessions WHERE league_id = '<uuid>';
   ```
3. The `current_pick_index` can be manually corrected in the database if needed.

### Player scores showing as 0 during a live game

**Symptoms:** A player's goals/assists show 0 on the match-day page even though they've scored.

**Cause:** Name matching failure (see Known Issues #2). The player's name in the `fantasy_players` table doesn't match the boxscore format.

**Fix:**
1. Check what name is stored: `SELECT name FROM fantasy_players WHERE nhl_id = <id>;`
2. Check the boxscore format: `curl "https://api-web.nhle.com/v1/gamecenter/<game_id>/boxscore"` and search for the player.
3. Update the player's name in the database to match the boxscore format if needed.
4. Invalidate the match-day cache.

### Backend fails to start on Fly.io

**Symptoms:** Deploy succeeds but health checks fail, machine restarts in a loop.

**Cause:** Most likely the startup backfill is blocking (Known Issues #5). Can also be a missing environment variable (`DATABASE_URL` or `JWT_SECRET` will panic on missing values).

**Fix:**
1. Check logs: `fly logs --app fantasy-hockey` for the panic message.
2. If it's the backfill, wait for it to complete (check for `Completed historical rankings population` in logs).
3. If it's a missing secret: `fly secrets list --app fantasy-hockey` and set any missing ones.
4. If the backfill is too slow, you can pre-populate the `daily_rankings` table manually and redeploy -- the backfill only runs when the table is empty.

### "Database error: pool timed out" in logs

**Symptoms:** API requests start failing with 500 errors, logs show pool timeout.

**Cause:** All 5 connection pool slots are occupied. This can happen if the scheduler is processing rankings (which makes many concurrent queries) while users are also hitting the API.

**Fix:**
1. This is usually transient -- wait for the scheduler job to finish.
2. If persistent, increase `max_connections` in `backend/src/db/mod.rs` (but stay within Supabase's connection limit).
3. Check for long-running queries in Supabase dashboard.
