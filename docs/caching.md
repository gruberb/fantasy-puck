# Fantasy Puck -- Caching Guide

This document covers every caching layer in the Fantasy Puck system, how they interact, known issues, and how to debug stale data problems.

---

## Layer 1: NhlClient In-Memory Cache

**Location**: `backend/src/nhl_api/nhl.rs`
**Type**: `Arc<RwLock<HashMap<String, CacheEntry>>>`
**Keyed by**: Full URL string (e.g., `https://api-web.nhle.com/v1/schedule/2026-04-08`)

### How It Works

Every NHL API request goes through `make_request_cached()`:

1. Acquire a **read lock** and check if the URL exists in the HashMap and is not expired.
2. On **cache hit**: Deserialize the stored JSON string and return it. No HTTP request.
3. On **cache miss or expired**: Release the read lock, call `fetch_raw()` (with semaphore for rate limiting), acquire a **write lock**, insert the new entry with a TTL.

### TTL Table (All 12 Endpoint Types)

| Constant | TTL | Duration | Endpoint Pattern |
|----------|-----|----------|------------------|
| `SKATER_STATS` | 300s | 5 minutes | `/v1/skater-stats-leaders/{season}/{gameType}` |
| `SCHEDULE` | 120s | 2 minutes | `/v1/schedule/now`, `/v1/schedule/{date}` |
| `GAME_CENTER` | 120s | 2 minutes | `/v1/gamecenter/{id}/landing` |
| `BOXSCORE_LIVE` | 60s | 1 minute | `/v1/gamecenter/{id}/boxscore` (game state != FINAL/OFF) |
| `BOXSCORE_FINAL` | 86400s | 24 hours | `/v1/gamecenter/{id}/boxscore` (game state == FINAL/OFF) |
| `PLAYOFF_CAROUSEL` | 900s | 15 minutes | `/v1/playoff-series/carousel/{season}` |
| `PLAYER_GAME_LOG` | 600s | 10 minutes | `/v1/player/{id}/game-log/{season}/{gameType}` |
| `PLAYER_DETAILS` | 1800s | 30 minutes | `/v1/player/{id}/landing` |
| `STANDINGS` | 1800s | 30 minutes | `/v1/standings/now` |
| `ROSTER` | 1800s | 30 minutes | `/v1/roster/{team}/current` |
| `EDGE` | 1800s | 30 minutes | `/v1/edge/skater-detail/{id}/now` |
| `SCORES` | 120s | 2 minutes | `/v1/score/{date}` |

### Adaptive Boxscore TTL

The boxscore endpoint (`get_game_boxscore`) has special handling. Unlike other endpoints that use `make_request_cached()` with a fixed TTL, it:

1. Checks the cache manually (read lock).
2. On miss, fetches the raw response.
3. **Inspects the `gameState` field** in the response JSON:
   - `"FINAL"` or `"OFF"` -> uses `BOXSCORE_FINAL` (24 hours)
   - Anything else (LIVE, CRIT, etc.) -> uses `BOXSCORE_LIVE` (1 minute)
4. Stores with the determined TTL.

This means a boxscore that was cached as LIVE (60s TTL) will be re-fetched quickly, while a final boxscore is cached for 24 hours. However, if a game transitions from LIVE to FINAL between fetches, the entry will be re-fetched after the 60s LIVE TTL expires and then re-cached with the 24-hour FINAL TTL.

### Cache Cleanup Task

Started on boot via `nhl_client.start_cache_cleanup(Duration::from_secs(300))` in `main.rs`:

- Runs every **5 minutes**.
- Acquires a **write lock** on the entire cache.
- Iterates all entries and removes those where `inserted_at.elapsed() > ttl`.
- Logs how many entries were removed.

**Note**: During cleanup, all cache reads and writes are blocked by the write lock. For a large cache, this could briefly delay NHL API requests.

### Cache Invalidation

- **Full invalidation**: `nhl_client.invalidate_cache()` clears the entire HashMap. Called from the admin endpoint `GET /api/admin/cache/invalidate?scope=all`.
- **There is no per-key invalidation** for the NhlClient in-memory cache. You can only clear everything.

### Race Conditions

Between the read lock release (cache miss) and the write lock acquisition (cache insert), another task could have already fetched and cached the same URL. The current code does not check for this -- it unconditionally overwrites. This is harmless (the duplicate fetch is wasted work, but the data is the same) but means that under high concurrency, the same URL could be fetched multiple times when the cache entry expires.

---

## Layer 2: Supabase `response_cache` Table

**Location**: `backend/src/db/cache.rs`
**Table**: `response_cache`
**Schema**: `(cache_key TEXT PRIMARY KEY, date TEXT, data TEXT, created_at TEXT, last_updated TEXT)`

### How It Works

The `CacheService` provides simple key-value storage in Postgres:

- **Read**: `get_cached_response(cache_key)` -- SELECT by primary key, deserialize JSON.
- **Write**: `store_response(cache_key, date, response)` -- UPSERT (INSERT ... ON CONFLICT DO UPDATE).
- **Invalidate**: Delete by key, by date, or all entries.

### Key Patterns

| Feature | Cache Key Pattern | Example |
|---------|-------------------|---------|
| Match Day | `match_day:{league_id}:{date}` | `match_day:abc-123:2026-04-08` |
| Games Extended | `games_extended:{league_id}:{date}` | `games_extended:abc-123:2026-04-08` |
| Insights | `insights:{league_id}:{date}` | `insights:abc-123:2026-04-08` |

### No Automatic TTL

Unlike the NhlClient cache, the response_cache table has **no automatic expiration**. Entries persist until:

1. **Overwritten** by a new `store_response` call with the same key.
2. **Explicitly deleted** via `invalidate_cache(key)`, `invalidate_by_date(date)`, or `invalidate_all()`.
3. **Admin endpoint** triggers deletion.

This means old cache entries from past dates accumulate indefinitely (see Known Issues).

### Smart Cache Bypass for Live Games

Both the match day and games extended handlers implement cache bypass logic:

**Match Day** (`get_match_day` in `games.rs`):
1. Check if any cached game has a `start_time` within -30 minutes to +4 hours of now (`has_potential_live_games`).
2. Check if any cached game has `game_state == LIVE` (`has_live_games`).
3. If either is true, call `update_live_game_data()` which:
   - Fetches current game state from NHL API for each game.
   - Fetches boxscores for live games.
   - Updates player stats in the cached response.
   - Stores the updated response back in the cache.

**Games Extended** (`process_games_extended` in `games.rs`):
1. Same potential-live and has-live checks.
2. If either is true, falls through to regenerate the full response.
3. If neither, returns the cached response directly.

---

## Layer 3: React Query Client-Side Cache

**Location**: `frontend/src/lib/react-query.ts`
**Instance**: Global `queryClient` singleton

### Global Defaults

```typescript
const queryConfig: DefaultOptions = {
  queries: {
    refetchOnWindowFocus: false,
    retry: false,
    staleTime: 1000 * 60 * 5,  // 5 minutes
  },
};
```

These defaults apply to **all** queries unless overridden per-hook.

- **staleTime: 5 minutes** -- Data is considered fresh for 5 minutes. Within this window, navigating back to a page will not trigger a refetch.
- **retry: false** -- Failed queries are not retried automatically.
- **refetchOnWindowFocus: false** -- Switching tabs does not trigger refetches.
- **gcTime** (garbage collection, formerly `cacheTime`): Not set in defaults, so React Query's built-in default of **5 minutes** applies. Inactive query data is garbage collected 5 minutes after the last observer unmounts.

### Per-Hook Overrides

| Hook | staleTime | gcTime | retry | Other |
|------|-----------|--------|-------|-------|
| `useInsights` | 15 min | 1 hour | 1 | -- |
| `useGamesData` | default (5 min) | default | 1 | 30s auto-refresh interval when live games detected |
| `useHomePageData` (sleepers) | default (5 min) | default | false | -- |
| `useSkaters` | default (5 min) | default | false | -- |
| `useRankingsData` | default (5 min) | default | 1 | -- |
| `useDraftSession` | default (5 min) | default | false | WebSocket optimistic updates via `setQueryData` |
| `useDraftPicks` | default (5 min) | default | false | WebSocket append via `setQueryData` |

### Cache Invalidation Patterns

1. **Mutation-triggered invalidation**: `useMakePick` calls `queryClient.invalidateQueries()` on both the session and picks query keys after a successful pick. This marks them as stale and triggers a background refetch.

2. **WebSocket optimistic updates**: Draft hooks use `queryClient.setQueryData()` to directly modify cached data without a network round-trip. The session hook merges partial session updates; the picks hook appends new picks.

3. **Manual refetch**: Several hooks expose a `refetch` function (e.g., `useGamesData`, `useInsights`). Components can call this to force a fresh fetch.

4. **Auto-refresh interval**: `useGamesData` sets up a 30-second `setInterval` when `autoRefresh` is enabled and live games are detected. Each tick calls `refetchGames()`.

### The Dead `DEFAULT_QUERY_OPTIONS` in `config.ts`

The file `frontend/src/config.ts` exports a `DEFAULT_QUERY_OPTIONS` object:

```typescript
export const DEFAULT_QUERY_OPTIONS = {
  staleTime: APP_CONFIG.STALE_TIME,   // 5 min
  cacheTime: APP_CONFIG.CACHE_TIME,   // 30 min
  retry: false as const,
  refetchOnWindowFocus: false,
};
```

**This object is never imported or used anywhere.** The actual defaults are set in `frontend/src/lib/react-query.ts` via the `queryConfig` passed to `QueryClient`. The `DEFAULT_QUERY_OPTIONS` is dead code. Notably, it specifies `cacheTime: 30 min` which differs from the actual default gcTime of 5 minutes.

---

## Interaction Between Layers

### Full Request Flow (e.g., Match Day)

```
  React Component renders
          |
          v
  React Query: is ["games", date, leagueId] fresh?
          |
     (stale or missing)
          |
          v
  fetchApi("/nhl/match-day?league_id=X")
          |
          v
  Backend handler: get_match_day()
          |
          v
  Layer 2: Check response_cache for "match_day:{league}:{date}"
          |
     (hit, no live games)          (miss or live games)
          |                              |
          v                              v
  Return cached JSON              Layer 1: NhlClient fetches
                                  - Schedule (TTL: 2 min)
                                  - Boxscores (TTL: 1 min live, 24hr final)
                                  - Player game logs (TTL: 10 min)
                                  - Player form (TTL: 10 min)
                                          |
                                          v
                                  Process and assemble response
                                          |
                                          v
                                  Store in Layer 2 (response_cache)
                                          |
                                          v
                                  Return JSON to frontend
          |
          v
  React Query: cache response in Layer 3
  Mark as fresh for staleTime (5 min)
```

### What Happens When Each Layer Misses/Expires

| Layer 3 (React Query) | Layer 2 (Supabase) | Layer 1 (NhlClient) | Result |
|---|---|---|---|
| Fresh | -- | -- | Instant: cached React data, no network request |
| Stale | Hit (no live) | -- | Fast: one DB read, return cached response |
| Stale | Hit (live games) | Hit | Medium: DB read + NhlClient cache reads for game state updates |
| Stale | Hit (live games) | Miss | Slow: DB read + NHL API fetches for boxscores |
| Stale | Miss | Hit | Medium: NhlClient cache reads for schedule/boxscores/stats |
| Stale | Miss | Miss | Slowest: full NHL API fetches for everything |

### Timing Example (Match Day with Live Game)

1. **T=0**: User opens Match Day page. React Query fires request.
2. **T=0.05s**: Backend checks response_cache. Hit, but has games starting within window.
3. **T=0.1s**: Backend calls `get_game_data()` for each game. NhlClient checks cache (game_center, TTL 2min).
4. **T=0.2s**: For live games, fetches boxscores. NhlClient may have 1-min-old cached boxscore.
5. **T=0.4s**: Updates player stats, stores updated response in response_cache, returns JSON.
6. **T=0.5s**: React Query caches the response. Component renders.
7. **T=30s**: If auto-refresh is on, `refetchGames()` fires. The cycle repeats from step 2.

---

## Cache Invalidation Strategies

### Admin Endpoints

`GET /api/admin/cache/invalidate` (`backend/src/api/handlers/admin.rs`):

| Scope Parameter | What It Does |
|-----------------|--------------|
| `?scope=all` | Deletes ALL response_cache rows + clears NhlClient in-memory cache |
| `?scope=today` | Deletes response_cache rows where `date = {hockey_today}` |
| `?scope=2026-04-08` (any date) | Deletes response_cache rows where `date = {date}` |
| (no scope) | Deletes only the `match_day:{today}` key from response_cache |

**Important**: The admin endpoint does NOT require admin auth in the route definition -- it is protected only by the `AuthUser` extractor (any authenticated user can call it). This is a potential security concern.

### Mutation-Triggered Invalidation

| Mutation | Invalidated Queries |
|----------|---------------------|
| `makePick` (draft) | `['draft', 'session', leagueId]`, `['draft', 'picks', draftSessionId]` |
| `makeSleeperPick` (draft) | Session broadcast via WebSocket triggers `setQueryData` |
| `updateProfile` | Not currently invalidated -- profile changes are reflected via `updateSessionProfile()` on the auth service |

### WebSocket Optimistic Updates

Draft picks bypass React Query's fetch cycle entirely:

1. **SessionUpdated**: `useDraftSession` merges the partial session update into the cached session object via `setQueryData`.
2. **PickMade**: `useDraftPicks` appends the new pick to the cached picks array, checking for duplicates by `pick.id`.
3. **SleeperUpdated**: Triggers a `refetch` of sleeper-related data.

---

## Known Issues

### 1. Stale Data After Game State Transitions

When a game transitions from `FUT` (future) to `LIVE`, the cached match day response still shows it as `FUT`. The smart bypass checks for `has_potential_live_games` (based on start time), but there is a window where:
- The game has started but no client has requested an update yet.
- The first client to request sees the stale `FUT` state.
- The `update_live_game_data` function then updates it.

**Mitigation**: The 30-second auto-refresh on the frontend when auto-refresh is enabled.

### 2. Dead `DEFAULT_QUERY_OPTIONS` in `config.ts`

`frontend/src/config.ts` exports `DEFAULT_QUERY_OPTIONS` which is never imported anywhere. It declares a `cacheTime` of 30 minutes and a `staleTime` of 5 minutes. The actual React Query config in `frontend/src/lib/react-query.ts` only sets `staleTime: 5 min` globally and uses React Query's default gcTime of 5 minutes.

Anyone reading `config.ts` might assume these values are in effect. They are not.

### 3. `response_cache` Accumulation

The Supabase `response_cache` table has no automatic cleanup. Every date generates new cache entries (match_day, games_extended, insights per league). Over a season, this could accumulate thousands of rows. The `date` column enables bulk deletion (`invalidate_by_date`), but nothing calls this automatically.

**Impact**: Mostly storage. No performance impact since lookups are by primary key (`cache_key`). But it creates unnecessary data in the database.

### 4. DST Timezone Bugs Affecting Cache Keys

The "hockey today" calculation in multiple handlers uses:

```rust
let month = now_utc.format("%m").to_string().parse::<u32>().unwrap_or(1);
let nhl_tz_offset: i64 = if (3..=10).contains(&month) { -4 } else { -5 };
```

This is a rough heuristic that assumes:
- March through October = EDT (UTC-4)
- November through February = EST (UTC-5)

In reality, DST transitions happen on specific Sundays:
- Spring forward: Second Sunday of March
- Fall back: First Sunday of November

During the transition week in early March, the app uses UTC-4 when it should still be UTC-5. This means:
- Cache keys use the wrong date (e.g., `match_day:league:2026-03-07` when the actual hockey date is still March 6).
- A user at 11:30pm ET on March 7 might see March 8's empty schedule instead of the current games.

The same heuristic appears in `insights.rs`, `games.rs` (multiple places), and `admin.rs`.

### 5. Cache Key Missing League ID in `update_live_game_data`

In `update_live_game_data()`, the cache key is constructed as:

```rust
let cache_key = format!("match_day:{}", hockey_today);
```

But the original cache key from `get_match_day()` includes the league_id:

```rust
let cache_key = format!("match_day:{}:{}", league_id, hockey_today);
```

This means live updates are stored under a different key than the one used for cache lookups, so:
- The live-updated data is stored under a key that will never be read.
- The original league-scoped cache entry is not updated.
- The next request re-reads the stale original and triggers another live update.

### 6. Insights Cache Never Refreshed During the Day

Insights are cached with key `insights:{league_id}:{hockey_today}`. Once generated (either by user request or the 10am UTC cron pre-warm), they are never regenerated for that day unless:
- An admin manually invalidates the cache.
- The cache entry is deleted.

If today's games change (postponement, trade deadline), the narratives and signals remain based on the state at generation time.

---

## Debugging Guide

### Diagnosing "Stale Data" Bugs

**Step 1: Identify which cache layer is suspect.**

| Symptom | Likely Layer |
|---------|-------------|
| Data updates immediately on hard refresh (Cmd+Shift+R) | Layer 3 (React Query) -- stale data in frontend cache |
| Data updates after waiting 2-5 minutes | Layer 1 (NhlClient) -- TTL hasn't expired yet |
| Data never updates unless admin clears cache | Layer 2 (Supabase response_cache) -- no automatic TTL |
| Data is wrong only for one league | Layer 2 cache key issue -- wrong league_id in key |
| Data is wrong around midnight | DST timezone bug in cache key calculation |

**Step 2: Check each layer.**

### Force-Refreshing Layer 3 (React Query)

From the browser console:

```javascript
// Clear ALL React Query cache
window.__REACT_QUERY_DEVTOOLS__?.queryClient?.clear()

// Or from within a component, use the refetch function:
// refetchGames(), refetch(), etc.
```

Or simply hard-refresh the page (Cmd+Shift+R) to reset all frontend state.

### Force-Refreshing Layer 2 (Supabase response_cache)

Use the admin endpoint:

```bash
# Clear everything (both DB cache and NhlClient in-memory)
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=all" \
  -H "Authorization: Bearer {token}"

# Clear just today's date
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=today" \
  -H "Authorization: Bearer {token}"

# Clear a specific date
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate?scope=2026-04-08" \
  -H "Authorization: Bearer {token}"

# Clear just today's match_day key (default)
curl "https://api.fantasy-puck.ca/api/admin/cache/invalidate" \
  -H "Authorization: Bearer {token}"
```

Or directly in the database:

```sql
-- See all cached entries
SELECT cache_key, date, created_at, last_updated,
       length(data) as data_size
FROM response_cache
ORDER BY last_updated DESC;

-- Delete specific entry
DELETE FROM response_cache
WHERE cache_key = 'match_day:abc-123:2026-04-08';

-- Delete all entries for a date
DELETE FROM response_cache WHERE date = '2026-04-08';

-- Nuclear option
DELETE FROM response_cache;
```

### Force-Refreshing Layer 1 (NhlClient In-Memory)

The only way to clear the NhlClient cache is via the admin endpoint with `scope=all`, which calls `nhl_client.invalidate_cache()`. There is no way to clear individual keys.

Alternatively, restarting the backend process clears the in-memory cache since it is not persisted.

### Checking What's Actually Cached

**Layer 1**: There is no inspection endpoint. Add logging or check the cache cleanup logs:

```
NHL cache cleanup: removed 12 expired entries (45 remaining)
```

**Layer 2**: Query the database directly:

```sql
SELECT cache_key, date,
       last_updated,
       length(data) as bytes
FROM response_cache
ORDER BY last_updated DESC
LIMIT 20;
```

**Layer 3**: If React Query Devtools are installed, open them in the browser. Otherwise, check the network tab for requests -- if a request is not being made, React Query is serving it from cache.

### Common Debugging Scenarios

**Scenario: Player scored a goal but it doesn't show up.**

1. Check: Is the game LIVE? If so, the boxscore TTL is 60 seconds. Wait and refresh.
2. Check: Is the player matched correctly? Look for name mismatches in the backend logs. The `find_player_stats_by_name` function uses fuzzy last-name matching which can fail for hyphenated names, Jr./Sr. suffixes, or players who go by nicknames.
3. Check: Is the response_cache returning stale data? Query the `last_updated` column.
4. Check: Is the frontend showing cached data? Hard refresh the browser.

**Scenario: Match Day shows no games but games are playing.**

1. Check: Is the date calculation correct? The DST heuristic might be using the wrong offset. Compare the `date` field in the response to the actual hockey date.
2. Check: Is the response_cache serving yesterday's "no games" response? Invalidate today's cache.
3. Check: Is the NHL schedule API returning data? Look for errors in the backend logs (`Fetching from NHL API: ...`).

**Scenario: Insights narratives reference wrong games.**

1. This is almost always a stale Layer 2 cache. Insights are date-keyed and never auto-refresh.
2. Invalidate: `DELETE FROM response_cache WHERE cache_key LIKE 'insights:%'`
3. The next request will regenerate fresh insights with current data.

**Scenario: Draft picks appear delayed for some users.**

1. Check WebSocket connection: Browser console should show the WebSocket connection to `wss://api.fantasy-puck.ca/ws/draft/{session_id}`.
2. Check for `RecvError::Lagged` in backend logs -- the client fell behind on the broadcast channel.
3. The mutation invalidation in `useMakePick` should reconcile state even if WebSocket messages were missed.
