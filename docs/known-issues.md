# Known Issues

Comprehensive list of known bugs and technical debt across the Fantasy Puck monorepo.
Last updated: 2026-04-08.

---

## CRITICAL

### 1. No authentication on admin endpoints

**Where:** `backend/src/api/routes.rs:203-210`, `backend/src/api/handlers/admin.rs`

**Description:** The `/api/admin/process-rankings/{date}` and `/api/admin/cache/invalidate` endpoints have no auth middleware. Any HTTP client can trigger ranking reprocessing for all leagues or wipe the entire response cache (DB + in-memory).

**Impact:** A malicious actor could repeatedly invalidate caches causing elevated NHL API traffic and degraded performance, or trigger expensive ranking reprocessing across all leagues. There is no rate limiting either.

**Suggested fix:** Add the JWT auth middleware and an `is_admin` check (the `profiles.is_admin` column already exists). Extract the auth extractor used in other handlers and apply it to admin routes. Example:

```rust
// Add an AdminAuth extractor that checks both JWT validity and profiles.is_admin = true
.route("/api/admin/process-rankings/{date}", get(handlers::admin::process_rankings).route_layer(AdminAuth))
```

---

### 2. Player name matching fragility in `find_player_stats_by_name`

**Where:** `backend/src/utils/nhl.rs:65-146`

**Description:** The `is_name_match` function (line 65) matches players by comparing last names extracted from the boxscore format (e.g., "C. McDavid") against stored fantasy player names. The matching logic uses substring containment:

```rust
default_name.contains(search_name)
    || search_name.contains(&boxscore_last_name)
    || boxscore_last_name == search_name
```

This produces false positives for short last names (e.g., "Lee" matches "Fleeting"), common last names shared between teammates, and players whose names differ between the fantasy DB and the NHL boxscore (accented characters, suffixes, nicknames).

Additionally, the function returns `(0, 0)` silently when a player is not found (line 145), making it impossible to distinguish "player did not play" from "name matching failed."

**Impact:** Incorrect goal/assist attribution in daily rankings and match-day views. Points may be credited to the wrong player or lost entirely.

**Suggested fix:** Match by `nhl_id` (player ID) instead of name. The boxscore includes player IDs in its structure. Store and use `nhl_id` as the primary lookup key, falling back to name matching only when the ID is unavailable.

---

## HIGH

### 3. DST timezone handling uses crude month-based approximation

**Where:** `backend/src/api/handlers/admin.rs:53-54`, `backend/src/api/handlers/insights.rs:23`

**Description:** Eastern Time offset is calculated with a simple month check:

```rust
let nhl_tz_offset = if (3..=10).contains(&month) { -4 } else { -5 };
```

This is wrong during DST transition weeks. DST starts on the second Sunday of March and ends on the first Sunday of November -- not on the 1st of those months. For example, if a game is played on March 5 (still EST), the code already uses EDT (-4), producing a date that's one hour off, potentially assigning stats to the wrong calendar date.

The same pattern appears in `hockey_today()` in the insights handler.

**Impact:** During the ~1-week transition windows in March and November, cache invalidation targets the wrong date, daily rankings may be computed for the wrong day, and insights are cached under incorrect date keys.

**Suggested fix:** Use the `chrono-tz` crate with `America/New_York` to get the correct offset:

```rust
use chrono_tz::America::New_York;
let now_et = Utc::now().with_timezone(&New_York);
```

---

### 4. Multiple WebSocket connections per draft page

**Where:** `frontend/src/features/draft/hooks/use-draft-session.ts:25-42`, `frontend/src/features/draft/hooks/use-draft-picks.ts:24-47`, `frontend/src/features/draft/hooks/use-sleeper-round.ts:61-73`

**Description:** Each of the three draft hooks (`useDraftSession`, `useDraftPicks`, `useSleeperRound`) independently calls `realtimeService.subscribeToDraft(sessionId, ...)`, each creating a separate WebSocket connection. A user on the draft page has 3 concurrent WebSocket connections to the same session endpoint.

**Impact:** Tripled server resource usage per draft participant. The backend's `DraftHub` broadcast channel sends the same events to all three connections. During a draft with multiple participants, this multiplies quickly. It also creates potential race conditions when all three connections receive the same event and trigger overlapping React Query invalidations.

**Suggested fix:** Create a single shared WebSocket subscription at the `DraftPage` level (or via a shared context/hook) and fan out events to the individual hooks via callbacks or a local event emitter. The `realtimeService.subscribeToDraft` already accepts all three handler callbacks in one `DraftEventHandlers` object -- use a single call.

---

### 5. Startup backfill blocks server start

**Where:** `backend/src/main.rs:49-57`

**Description:** On first deploy after playoffs begin, `populate_historical_rankings` runs synchronously before the server starts listening. This iterates every date from playoff start to today, fetching NHL API data for each date and each league. For a multi-week playoff with multiple leagues, this can take several minutes.

**Impact:** Fly.io health checks timeout during this period, potentially causing the deploy to be marked as failed or the machine to be killed and restarted in a loop. The application is completely unavailable during backfill.

**Suggested fix:** Start the HTTP server first, then run the backfill in a background `tokio::spawn` task. Add a `/health` endpoint that returns 200 even while backfill is in progress, and expose backfill status via a simple flag or admin endpoint.

---

### 6. `daily_rankings` UNIQUE constraint missing `league_id`

**Where:** `backend/migrations/001_create_users_table.sql:121`, `backend/src/utils/scheduler.rs:127`

**Description:** The table's unique constraint is `UNIQUE(team_id, date)` but the `ON CONFLICT` upsert clause in `process_daily_rankings` is also `ON CONFLICT (team_id, date) DO UPDATE`. Since `league_id` is not part of the constraint, if the same fantasy team somehow appears in two leagues (unlikely with current schema but possible after data migrations), only one league's ranking would be stored.

More critically, the constraint was designed before multi-league support was added. While `team_id` is currently unique across leagues (teams belong to one league via `fantasy_teams.league_id`), the constraint doesn't express the intended invariant of "one ranking per team per date per league."

**Impact:** If the data model ever allows a team to participate in multiple leagues, rankings silently overwrite each other. The `ON CONFLICT` clause also doesn't match the full intended key, which could mask insertion bugs.

**Suggested fix:** Alter the unique constraint to `UNIQUE(team_id, date, league_id)` and update the `ON CONFLICT` clause to match:

```sql
ON CONFLICT (team_id, date, league_id) DO UPDATE SET rank = EXCLUDED.rank, points = EXCLUDED.points
```

---

## MEDIUM

### 7. `response_cache` table accumulates without cleanup

**Where:** `backend/src/db/cache.rs`

**Description:** The `response_cache` table stores cached API responses as JSON text with `created_at` and `last_updated` timestamps, but there is no TTL enforcement or periodic cleanup. Entries are only removed by explicit invalidation (`invalidate_cache`, `invalidate_by_date`, `invalidate_all`). The daily scheduler does not prune old entries.

**Impact:** Over the course of a season, the table grows unboundedly. Each entry contains a full serialized API response (potentially large for match-day and insights data). This increases database storage costs and slows queries against the table.

**Suggested fix:** Add a scheduled job or a `DELETE FROM response_cache WHERE created_at < NOW() - INTERVAL '7 days'` cleanup. Alternatively, add an `expires_at` column and clean up expired rows in the morning cron job.

---

### 8. Web scraping fragility (DailyFaceoff CSS selectors)

**Where:** `backend/src/api/handlers/insights.rs:658-726`

**Description:** The `scrape_headlines` function scrapes DailyFaceoff using multiple CSS selectors as fallbacks (`"h3 a"`, `".news-item h3"`, `".news-item__title"`, etc.). These selectors are brittle and will break whenever DailyFaceoff redesigns their HTML.

The scraper also has no caching of its own -- it is called every time insights are generated (though results are cached at the insights level). Errors are silently swallowed with `if let Ok(...)`, so a broken scraper produces empty headlines with no alerting.

**Impact:** When selectors break, the insights page shows no news headlines. Since errors are silently swallowed, there is no log entry to indicate the scraper is failing -- it simply returns an empty list.

**Suggested fix:** Log warnings when zero headlines are scraped. Consider using an RSS feed or a dedicated news API instead of HTML scraping. If scraping must stay, add monitoring for the zero-headline case and version the selectors so they can be updated without a redeploy (e.g., via environment variables or a config table).

---

### 9. LeagueContext not using React Query (no caching/dedup)

**Where:** `frontend/src/contexts/LeagueContext.tsx:95-203`

**Description:** The `LeagueProvider` fetches leagues, memberships, and draft sessions using raw `useEffect` + `useState` + manual `api.*` calls. Unlike other data-fetching hooks in the app (e.g., `useDraftSession`, `useDraftPicks`), this context does not use React Query, so there is:

- No automatic request deduplication (multiple mounts trigger multiple fetches)
- No stale-while-revalidate behavior
- No cache sharing with other components
- Manual `cancelled` flag tracking instead of React Query's built-in cancellation

**Impact:** On app load, leagues and memberships are fetched fresh every time the provider mounts. Tab switches cause redundant fetches. The draft session fetch (line 171-203) duplicates the work done by `useDraftSession` elsewhere.

**Suggested fix:** Replace the three `useEffect` blocks with `useQuery` hooks from `@tanstack/react-query`, using the same query keys as the existing feature hooks to enable cache sharing.

---

### 10. `window.location.reload()` anti-patterns

**Where:**
- `frontend/src/pages/HomePage.tsx:88` (after joining a league)
- `frontend/src/pages/InsightsPage.tsx:24` (error retry)
- `frontend/src/components/fantasyTeams/EmptyFantasyTeamsState.tsx:37` (retry button)
- `frontend/src/components/common/ErrorBoundary.tsx:51` (error recovery)

**Description:** Several components use `window.location.reload()` to handle state changes or error recovery. This is a full page reload that destroys all React state, React Query cache, WebSocket connections, and auth state in memory.

**Impact:** Poor user experience -- the entire app reinitializes, causing a flash of loading states. After joining a league on the home page (line 88), the user loses their scroll position and any other in-progress interactions. React Query's cache is cleared unnecessarily.

**Suggested fix:** Use React Query's `invalidateQueries` to refresh specific data after mutations (e.g., after joining a league, invalidate the leagues and memberships queries). For error recovery, use React Query's built-in retry or error boundary reset patterns.

---

### 11. Dead `DEFAULT_QUERY_OPTIONS` config code

**Where:** `frontend/src/config.ts:20-25`

**Description:** The `DEFAULT_QUERY_OPTIONS` constant is exported but never imported anywhere in the codebase. The `STALE_TIME` and `CACHE_TIME` values in `APP_CONFIG` (lines 16-17) that feed into it are also unused.

**Impact:** No functional impact, but the dead code misleads developers into thinking query defaults are centrally configured when they are not. Individual hooks set their own `staleTime` (or use React Query's defaults).

**Suggested fix:** Either remove the dead constants, or actually apply them as `defaultOptions` in the `QueryClient` configuration:

```typescript
const queryClient = new QueryClient({
  defaultOptions: { queries: DEFAULT_QUERY_OPTIONS },
});
```

---

## LOW

### 12. `search_players` stops after first team match

**Where:** `backend/src/nhl_api/nhl.rs:206-263`

**Description:** The `search_players` method iterates over all NHL team rosters to find players matching a search query. However, it sets `found_match = true` after finding matches on the first team and then `break`s (line 219), stopping the search entirely.

**Impact:** If a player name appears on multiple teams (e.g., after a trade where roster caches haven't updated), or if the user is searching for a common substring that matches players across teams, only results from the first matching team are returned.

**Suggested fix:** Remove the `found_match` flag and `break` to search all teams. If performance is a concern (32 roster fetches), consider maintaining a pre-built player index or adding pagination.

---

### 13. Hardcoded season and playoff dates

**Where:**
- `backend/src/api/mod.rs:15-16` -- `SEASON: u32 = 20252026` and `GAME_TYPE: u8 = 3`
- `backend/src/main.rs:46` -- `playoff_start = "2026-04-18"`
- `backend/src/main.rs:52` -- `end_date` capped at `"2026-06-15"`
- `frontend/src/config.ts:9-10` -- `DEFAULT_SEASON: "20252026"` and `DEFAULT_GAME_TYPE: 3`

**Description:** Season identifiers and playoff dates are hardcoded constants scattered across backend and frontend. Changing them requires code changes and a redeploy.

**Impact:** Every season requires a code change in at least 4 locations. Missing one causes data mismatches between frontend and backend. The `GAME_TYPE: 3` (playoffs) means the app only works correctly during the playoffs -- regular season would require changing this to `2`.

**Suggested fix:** Move these to environment variables or a shared configuration endpoint. The backend could expose a `/api/config` endpoint that the frontend consumes, ensuring both are always in sync.

---

### 14. Orphaned sleeper scoping via `WHERE s.team_id IS NULL`

**Where:** `backend/src/db/sleepers.rs:17-28`

**Description:** The `get_all_sleepers` query includes sleepers where `team_id IS NULL` (unassigned sleepers) regardless of league. This means unassigned sleepers from other leagues (or pre-league sleepers) appear in every league's sleeper list.

```sql
WHERE s.team_id IS NULL OR lm.league_id = $1::uuid
```

**Impact:** Users may see sleeper players that were not part of their league's draft. This is a minor data leak across leagues for unassigned sleeper entries.

**Suggested fix:** Add a `league_id` or `draft_session_id` column to `fantasy_sleepers` to properly scope unassigned sleepers to a specific league.

---

### 15. Unused `@supabase/supabase-js` dependency

**Where:** `frontend/package.json:14`

**Description:** The `@supabase/supabase-js` package is listed as a production dependency but is not imported anywhere in `frontend/src/`. The app migrated to a custom JWT auth backend but the Supabase client library was never removed.

**Impact:** Adds ~50KB to the bundle (if tree-shaking doesn't eliminate it), increases `npm install` time, and creates confusion about the auth architecture.

**Suggested fix:** Remove with `npm uninstall @supabase/supabase-js`.

---

### 16. No WebSocket heartbeat/keepalive

**Where:** `backend/src/ws/handler.rs:29-85`, `frontend/src/lib/realtime.ts:34-101`

**Description:** The WebSocket handler responds to client pings (line 60-63) but never initiates server-side pings. The frontend client has reconnection logic with exponential backoff (line 77-83) but does not send periodic pings either.

**Impact:** Idle connections may be silently dropped by intermediate proxies (Fly.io's proxy, CDNs, corporate firewalls) without either side knowing. The client only discovers the disconnection when the next message fails or `onclose` fires, which may not happen for minutes. During a draft, this means a participant could miss picks.

**Suggested fix:** Add a server-side ping interval (e.g., every 30 seconds) in `handle_draft_ws`:

```rust
let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
// In the select! loop:
_ = ping_interval.tick() => {
    if ws_sender.send(Message::Ping(vec![])).await.is_err() {
        break;
    }
}
```

---

### 17. `daily_rankings` goals/assists columns never populated

**Where:** `backend/src/utils/scheduler.rs:124-138`, `backend/migrations/001_create_users_table.sql:118-119`

**Description:** The `daily_rankings` table has `goals` and `assists` columns (defaulting to 0), but the INSERT in `process_daily_rankings` only writes `date`, `team_id`, `league_id`, `rank`, and `points`:

```sql
INSERT INTO daily_rankings (date, team_id, league_id, rank, points)
VALUES ($1, $2, $3::uuid, $4, $5)
```

The `DailyRanking` struct has `daily_goals` and `daily_assists` fields that are computed but never stored.

**Impact:** The `goals` and `assists` columns are always 0 in the database. Any future feature that tries to display historical goal/assist breakdowns from the `daily_rankings` table will show zeros.

**Suggested fix:** Include `goals` and `assists` in the INSERT:

```sql
INSERT INTO daily_rankings (date, team_id, league_id, rank, points, goals, assists)
VALUES ($1, $2, $3::uuid, $4, $5, $6, $7)
ON CONFLICT (team_id, date) DO UPDATE SET
    rank = EXCLUDED.rank, points = EXCLUDED.points,
    goals = EXCLUDED.goals, assists = EXCLUDED.assists
```
