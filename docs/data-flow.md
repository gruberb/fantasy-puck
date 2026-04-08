# Fantasy Puck -- Data Flow Documentation

This document traces every major data pipeline in the Fantasy Puck monorepo end-to-end, from external data sources through backend processing to frontend rendering.

---

## 1. NHL API -> Backend -> Frontend Pipeline

The foundational pipeline. All NHL data enters the system through `NhlClient`, a rate-limited, caching HTTP client.

### Flow Diagram

```
  NHL API (api-web.nhle.com)
          |
          v
  +-------------------+
  |    NhlClient       |  (max 5 concurrent, retry on 429)
  |  in-memory cache   |  (HashMap<URL, CacheEntry>)
  |  per-endpoint TTL  |
  +-------------------+
          |
          v
  +-------------------+
  |   Axum Handlers    |  (transform NHL models -> API DTOs)
  +-------------------+
          |
          v
  +-------------------+
  |  JSON over HTTP    |  (GET /api/nhl/*)
  +-------------------+
          |
          v
  +-------------------+
  |  fetchApi()        |  (frontend/src/lib/api-client.ts)
  |  + Bearer token    |
  +-------------------+
          |
          v
  +-------------------+
  |  React Query       |  (staleTime: 5min default)
  |  client-side cache |
  +-------------------+
          |
          v
  +-------------------+
  |  React Components  |
  +-------------------+
```

### Files Involved

| Step | File |
|------|------|
| NHL HTTP client | `backend/src/nhl_api/nhl.rs` |
| Endpoint URLs | `backend/src/nhl_api/nhl_constants.rs` |
| NHL data models | `backend/src/models/nhl.rs` |
| API DTOs / conversion | `backend/src/api/dtos/*.rs` |
| Route definitions | `backend/src/api/routes.rs` |
| Frontend API client | `frontend/src/lib/api-client.ts` |
| Frontend API methods | `frontend/src/api/client.ts` |
| React Query setup | `frontend/src/lib/react-query.ts` |

### Where Data Can Get Stale

- **NhlClient in-memory cache**: Each endpoint has its own TTL (2 min for schedules, 5 min for stats, 30 min for rosters). Data is at most TTL-old.
- **React Query cache**: Default staleTime of 5 minutes means the frontend will not refetch within that window even if the backend has fresher data.
- **Compounding staleness**: In the worst case, a schedule update is 2 min stale in NhlClient + 5 min stale in React Query = up to 7 minutes behind real NHL data.

### Common Failure Modes

- **NHL API rate limit (429)**: NhlClient retries up to 3 times with exponential backoff (500ms, 1000ms, 1500ms). If all retries fail, the handler returns an error.
- **NHL API downtime**: 30-second request timeout. Frontend `fetchApi` has optional fallback values for non-critical endpoints (e.g., rankings return `[]`).
- **Deserialization mismatch**: If the NHL API changes its JSON shape, `serde_json::from_str` fails and the error propagates to the frontend.

---

## 2. Insights Generation Pipeline

The most AI-intensive pipeline. Computes 6 concurrent signal categories, sends them to Claude, extracts structured JSON, caches the result, and renders it with bold name parsing on the frontend.

### Flow Diagram

```
  GET /api/insights?league_id=X
          |
          v
  +-------------------------------+
  |  Check Supabase cache         |
  |  key: "insights:{league}:{date}" |
  +-------------------------------+
          |  (miss)
          v
  +-------------------------------+
  |  compute_signals()            |
  |  6 concurrent signal tasks:   |
  |    1. hot_players (form + Edge) |
  |    2. cup_contenders (playoffs) |
  |    3. todays_games (schedule)  |
  |    4. fantasy_race (rankings)  |
  |    5. sleeper_alerts (stats)   |
  |    6. scrape_headlines (HTTP)  |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  call_claude_api()            |
  |  Model: claude-haiku-4-5      |
  |  Max tokens: 3072             |
  |  System prompt + signals JSON |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  extract_json_from_text()     |
  |  Strips ```json``` blocks     |
  |  Falls back to raw { } search |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Parse into InsightsNarratives|
  |  Fields: todays_watch,        |
  |    game_narratives[],         |
  |    hot_players, cup_contenders,|
  |    fantasy_race, sleeper_watch|
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Store in response_cache      |
  |  (Supabase, no TTL)           |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Frontend: useInsights()      |
  |  staleTime: 15 min            |
  |  gcTime: 1 hour               |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  <Narrative> component        |
  |  Splits on /(\*\*[^*]+\*\*)/g |
  |  Wraps matches in <strong>    |
  +-------------------------------+
```

### Signal Computation Detail

All 6 signals run via `tokio::join!` in `compute_signals()`:

1. **hot_players**: Fetches top 20 points leaders -> gets L5 form for each (concurrent via NhlClient semaphore) -> sorts by form_points, takes top 5 -> enriches with NHL Edge data (skating/shot speed).
2. **cup_contenders**: Fetches playoff carousel -> finds current round -> extracts series leads -> takes top 3.
3. **todays_games**: Fetches schedule + standings + yesterday's scores (concurrent) -> builds per-game matchup data with leaders, goalies, streaks.
4. **fantasy_race**: Gets all fantasy teams -> calculates total points from NHL stats -> counts players active today.
5. **sleeper_alerts**: Gets all sleepers for the league -> finds their goals/assists from NHL stats.
6. **scrape_headlines**: HTTP scrapes Daily Faceoff player news + injury report pages using CSS selectors.

### Files Involved

| Step | File |
|------|------|
| Handler + all signal computation | `backend/src/api/handlers/insights.rs` |
| Insight DTOs | `backend/src/api/dtos/insights.rs` |
| Supabase cache layer | `backend/src/db/cache.rs` |
| Scheduler pre-warming | `backend/src/utils/scheduler.rs` (10am UTC cron) |
| Frontend hook | `frontend/src/features/insights/hooks/use-insights.ts` |
| Bold name rendering | `frontend/src/pages/InsightsPage.tsx` (`Narrative` component) |

### Where Data Can Get Stale

- **Cache key is date-scoped** (`insights:{league_id}:{hockey_today}`). Once generated for a day, it will not regenerate unless the cache is invalidated manually. The cron job at 10am UTC pre-warms it daily.
- **Claude API narratives** reference game schedules and stats at generation time. If a game gets postponed or a trade happens after generation, narratives are wrong for the rest of the day.
- **Frontend staleTime of 15 min** means the same cached insights are reused across page navigations.

### Common Failure Modes

- **ANTHROPIC_API_KEY not set**: Falls back to `fallback_narratives()` with placeholder text "Unable to generate insights at this time."
- **Claude API timeout (30s)**: Same fallback. Signals are still returned even if narratives fail.
- **JSON extraction failure**: If Claude returns malformed JSON, `extract_json_from_text` tries multiple strategies (code block, raw `{...}`, substring extraction). If all fail, fallback narratives are used.
- **Scraper failures**: Headlines scraping silently returns an empty Vec -- it does not block insight generation.

---

## 3. Match Day Pipeline

The most complex flow. Handles schedule fetching, boxscore processing, fantasy player cross-referencing, live game updates, and smart cache bypass.

### Flow Diagram

```
  GET /api/nhl/match-day?league_id=X
          |
          v
  +-------------------------------+
  |  Calculate "hockey today"     |
  |  (Eastern Time offset)        |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Check Supabase cache         |
  |  key: "match_day:{league}:{date}" |
  +-------------------------------+
       /          \
    (hit)        (miss)
      |              |
      v              v
  +-----------+   +----------------------------+
  | Check for |   | Fetch today's schedule     |
  | live games|   | (+ yesterday if < noon)    |
  +-----------+   +----------------------------+
      |                     |
      | (has live or        v
      |  potential live)  +----------------------------+
      v                  | Get fantasy teams for      |
  +-----------+          | all playing NHL teams      |
  | update_   |          +----------------------------+
  | live_game |                   |
  | _data()   |                   v
  +-----------+          +----------------------------+
      |                  | Pre-load boxscores for     |
      |                  | live/completed games       |
      |                  +----------------------------+
      |                           |
      |                           v
      |                  +----------------------------+
      |                  | process_players_for_team() |
      |                  | For each game x each team: |
      |                  |  - Get player form (L5)    |
      |                  |  - Get playoff stats       |
      |                  |  - Match boxscore stats    |
      |                  +----------------------------+
      |                           |
      v                           v
  +-------------------------------+
  |  Store in response_cache      |
  |  Return MatchDayResponse      |
  +-------------------------------+
```

### Live Game Update Path (Cache Hit with Live Games)

When a cached response exists but games are potentially live:

```
  Cached MatchDayResponse
          |
          v
  +-------------------------------+
  |  has_potential_live_games?     |
  |  (start_time within           |
  |   -30min to +4hr of now)      |
  +-------------------------------+
          | yes
          v
  +-------------------------------+
  |  update_live_game_data()      |
  |  For each game:               |
  |    1. get_game_data() -> state,|
  |       scores, period          |
  |    2. If LIVE: fetch boxscore |
  |    3. Update player stats     |
  |       from boxscore by ID     |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  If changes detected:         |
  |  Re-store in response_cache   |
  +-------------------------------+
```

### Early Morning Logic

Between midnight and noon (hockey Eastern time), the pipeline also fetches yesterday's schedule and includes any games that are still in state `LIVE`. This handles late-night games that run past midnight.

### Name Matching for Fantasy Players

Player stats from boxscores are matched to fantasy players using `find_player_stats_by_name()` in `backend/src/utils/nhl.rs`. This is a fuzzy last-name match:
1. Extracts last name from both the fantasy player name and boxscore name (format: "C. McDavid").
2. Checks if either name contains the other's last name.
3. If not found on the expected team, falls back to searching the opposing team.

### Files Involved

| Step | File |
|------|------|
| Match day handler | `backend/src/api/handlers/games.rs` (`get_match_day`) |
| Live update function | `backend/src/api/handlers/games.rs` (`update_live_game_data`) |
| Player stats matching | `backend/src/utils/nhl.rs` (`find_player_stats_by_name`) |
| API utility functions | `backend/src/utils/api.rs` (`process_players_for_team`, `create_games_summary`) |
| Games extended handler | `backend/src/api/handlers/games.rs` (`process_games_extended`) |
| Frontend hook | `frontend/src/features/games/hooks/use-games-data.ts` |
| Frontend auto-refresh | Same hook, 30-second interval when live games detected |

### Where Data Can Get Stale

- **Supabase cache has no automatic TTL**: The cache entry persists until overwritten by a new response or manually invalidated. The "smart bypass" for live games only triggers when the frontend requests it and games are potentially live.
- **Boxscore TTL**: Live games have 60-second boxscore cache, finals have 24-hour. A goal scored will take up to 60 seconds to appear.
- **Between cache updates**: If no client is requesting data, the Supabase cache is never updated. The next request after a gap may return stale cached data before triggering the live update path.

### Common Failure Modes

- **Boxscore fetch failure**: Handled with `.ok()` -- returns `None` and players show 0 stats for that game.
- **Name matching failure**: If a player's name differs between the roster and boxscore (e.g., nicknames, accented characters), `find_player_stats_by_name` returns `(0, 0)`. The player appears in the response but with zero stats.
- **DST timezone bug**: The hockey date calculation uses a rough month-based heuristic (`March-October = EDT = -4, else EST = -5`) rather than proper timezone library. This is wrong during the DST transition weeks in early March and early November.
- **Cache key missing league_id in live update**: The `update_live_game_data` function constructs a cache key as `"match_day:{date}"` without the league_id, which can cause cross-league cache pollution.

---

## 4. Draft Pipeline

Real-time draft with REST for mutations and WebSocket for broadcasting updates to all participants.

### Flow Diagram (Pick)

```
  POST /api/draft/{draft_id}/pick
  Body: { playerPoolId: "..." }
          |
          v
  +-------------------------------+
  |  Validate session is "active" |
  |  Calculate pick order:        |
  |   - Global pick_index         |
  |   - Round = index / members   |
  |   - Snake: even=fwd, odd=rev |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Check player not already     |
  |  drafted (DB query)           |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  INSERT INTO draft_picks      |
  |  (Supabase/Postgres)          |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Advance session:             |
  |  new_pick_index + 1           |
  |  new_round (1-based)          |
  |  OR mark "picks_done"         |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  DraftHub.broadcast()         |
  |  1. PickMade { pick: JSON }   |
  |  2. SessionUpdated { ... }    |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  tokio broadcast::channel(64) |
  |  Serialized as JSON string    |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  ws_draft handler             |
  |  (ws/handler.rs)              |
  |  Forwards to WebSocket client |
  +-------------------------------+
          |
          v
  +-------------------------------+
  |  Frontend WebSocket client    |
  |  (lib/realtime.ts)            |
  |  Dispatches by message.type:  |
  |    "sessionUpdated" ->        |
  |      setQueryData (optimistic)|
  |    "pickMade" ->              |
  |      append to picks cache    |
  +-------------------------------+
```

### Frontend Cache Update on Pick

The frontend uses two complementary strategies:

1. **WebSocket optimistic update** (`use-draft-picks.ts`): On `onPickMade`, the new pick is appended directly to the React Query cache via `setQueryData`. No network request needed.
2. **Mutation invalidation** (`use-make-pick.ts`): After the REST `makePick` mutation succeeds, it invalidates both the session and picks query keys, triggering a refetch as a safety net.

### WebSocket Connection Lifecycle

```
  Frontend connects: wss://{host}/ws/draft/{session_id}?token={jwt}
          |
          v
  Backend: ws_draft() upgrades HTTP -> WebSocket
          |
          v
  Backend: DraftHub.subscribe(session_id) -> broadcast::Receiver
          |
          v
  Loop: select! {
    broadcast msg -> send to WebSocket
    WebSocket msg -> handle Ping/Pong/Close
  }
          |
  (client disconnects or channel closes)
          v
  Connection cleaned up, log "disconnected"
```

Reconnection uses exponential backoff starting at 1 second, doubling up to 30 seconds.

### Draft Lifecycle States

```
  pending -> active -> picks_done -> (finalize) -> sleeper round active -> completed
```

### Files Involved

| Step | File |
|------|------|
| Draft handlers (all operations) | `backend/src/api/handlers/draft.rs` |
| Draft database operations | `backend/src/db/draft.rs` |
| WebSocket hub (broadcast) | `backend/src/ws/draft_hub.rs` |
| WebSocket connection handler | `backend/src/ws/handler.rs` |
| Frontend draft session hook | `frontend/src/features/draft/hooks/use-draft-session.ts` |
| Frontend draft picks hook | `frontend/src/features/draft/hooks/use-draft-picks.ts` |
| Frontend make pick mutation | `frontend/src/features/draft/hooks/use-make-pick.ts` |
| Frontend realtime service | `frontend/src/lib/realtime.ts` |
| Frontend draft API | `frontend/src/features/draft/api/draft-api.ts` |

### Where Data Can Get Stale

- **WebSocket lag**: The broadcast channel has a capacity of 64 messages. If a client falls behind, `tokio::sync::broadcast` returns `RecvError::Lagged(n)` and the handler logs a warning. The client misses those messages but catches up on the next one.
- **Optimistic vs. server state**: The WebSocket `setQueryData` update is optimistic -- it assumes the pick succeeded. The subsequent mutation invalidation serves as reconciliation.

### Common Failure Modes

- **WebSocket connection fails**: Frontend catches errors and reconnects with backoff. During disconnection, users miss real-time updates but can manually refresh.
- **Duplicate pick attempt**: Backend checks `check_player_already_picked` and returns `Error::Validation("Player already drafted")`.
- **Race condition on concurrent picks**: The backend uses a global `current_pick_index` counter. Two simultaneous picks could both read the same index. The DB insert with a unique constraint on `(draft_session_id, pick_number)` would reject the duplicate.

---

## 5. Rankings Pipeline

An automated pipeline that runs via cron jobs to compute daily rankings from boxscores.

### Flow Diagram

```
  Cron Jobs (tokio_cron_scheduler)
  +------------------------------+
  | 9am UTC:  process yesterday  |
  | 3pm UTC:  process yesterday  |
  | 10am UTC: prewarm insights   |
  +------------------------------+
          |
          v
  +------------------------------+
  | process_daily_rankings()     |
  | For each league:             |
  +------------------------------+
          |
          v
  +------------------------------+
  | Fetch schedule for date      |
  | Filter: completed games only |
  | (skip if any games still     |
  |  LIVE -- wait for next run)  |
  +------------------------------+
          |
          v
  +------------------------------+
  | For each completed game      |
  | (up to 4 concurrently):      |
  |  1. Fetch boxscore           |
  |  2. Get fantasy players for  |
  |     both NHL teams in league |
  |  3. process_game_performances|
  |     - Name match each player |
  |     - Extract goals, assists |
  |     - Filter: points > 0     |
  +------------------------------+
          |
          v
  +------------------------------+
  | Aggregate by fantasy team:   |
  | Merge player performances    |
  | across multiple games        |
  +------------------------------+
          |
          v
  +------------------------------+
  | DailyRanking::build_rankings |
  | Sort by daily_points desc    |
  | Assign ranks                 |
  +------------------------------+
          |
          v
  +------------------------------+
  | INSERT INTO daily_rankings   |
  | ON CONFLICT (team_id, date)  |
  | DO UPDATE                    |
  +------------------------------+
```

### Frontend Query

```
  GET /api/fantasy/rankings/daily?league_id=X&date=YYYY-MM-DD
          |
          v
  +------------------------------+
  | Fetch schedule for date      |
  | Filter: completed or live    |
  | Fetch boxscores concurrently |
  | process_game_performances()  |
  | Build and return rankings    |
  +------------------------------+
```

The daily rankings endpoint computes rankings on-the-fly from boxscores, not from the `daily_rankings` table. The table is used for the scheduler's historical record.

### Historical Backfill

On startup, if `daily_rankings` is empty and the playoff start date has passed, `populate_historical_rankings()` iterates from the playoff start date to today, processing each date for each league.

### Files Involved

| Step | File |
|------|------|
| Scheduler setup + cron jobs | `backend/src/utils/scheduler.rs` |
| Rankings handler (live query) | `backend/src/api/handlers/rankings.rs` |
| Game performance processing | `backend/src/utils/fantasy.rs` |
| Player name matching | `backend/src/utils/nhl.rs` |
| Startup initialization | `backend/src/main.rs` |
| Frontend rankings hook | `frontend/src/features/rankings/hooks/use-rankings-data.ts` |
| Frontend home page hook | `frontend/src/hooks/use-home-page-data.ts` |

### Where Data Can Get Stale

- **Cron runs at fixed UTC times**: If games finish after 3pm UTC (11am ET), the rankings for that date are not computed until the next morning's 9am UTC run. Late-night West Coast games may not be captured until the next day.
- **"Skip if any games live" guard**: If even one game is still LIVE at cron time, the entire date is skipped. This is a conservative choice to avoid partial rankings. The 3pm UTC re-run is the safety net.
- **Live daily rankings endpoint**: The `get_daily_rankings` handler computes on-the-fly and includes LIVE game data, but this is not stored in the `daily_rankings` table.

### Common Failure Modes

- **Boxscore fetch failure for one game**: The entire date processing fails for that league due to the error propagation through `try_fold`. Other leagues are processed independently.
- **Name matching misses**: Same issue as match day -- players with name mismatches show 0 points in rankings.
- **Empty league**: `get_all_league_ids()` returns leagues that may have no members or no fantasy teams, causing the processing to run but produce empty results.

---

## 6. Auth Pipeline

JWT-based authentication with localStorage persistence and cross-tab synchronization.

### Flow Diagram (Registration)

```
  POST /api/auth/register
  Body: { email, password, displayName }
          |
          v
  +------------------------------+
  |  Check email not taken       |
  |  (get_user_by_email)         |
  +------------------------------+
          |
          v
  +------------------------------+
  |  hash_password() (bcrypt)    |
  |  INSERT INTO users           |
  |  INSERT INTO profiles        |
  +------------------------------+
          |
          v
  +------------------------------+
  |  issue_token()               |
  |  JWT with Claims:            |
  |    sub: user_id (UUID)       |
  |    email: email              |
  |    is_admin: bool            |
  |    exp: now + 7 days         |
  |    iat: now                  |
  |  Signed with JWT_SECRET      |
  +------------------------------+
          |
          v
  +------------------------------+
  |  Response: { token, user,    |
  |              profile }       |
  +------------------------------+
          |
          v (frontend)
  +------------------------------+
  |  BackendAuthService          |
  |  .register() receives token  |
  |  Stores in localStorage      |
  |  key: "auth_session"         |
  |  value: { token, user,       |
  |           profile }          |
  +------------------------------+
          |
          v
  +------------------------------+
  |  AuthContext.signUp() sets   |
  |  React state: user, profile  |
  |  App re-renders as logged in |
  +------------------------------+
```

### Request Authentication Flow

```
  Any authenticated request
          |
          v
  +------------------------------+
  |  fetchApi() in api-client.ts |
  |  Reads: authService.getToken()|
  |  Sets header:                |
  |  Authorization: Bearer {jwt} |
  +------------------------------+
          |
          v
  +------------------------------+
  |  Axum middleware: AuthUser    |
  |  (FromRequestParts impl)     |
  |  1. Extract Authorization    |
  |     header                   |
  |  2. Strip "Bearer " prefix   |
  |  3. validate_token(jwt,      |
  |     jwt_secret)              |
  |  4. Return AuthUser {        |
  |     id, email, is_admin }    |
  +------------------------------+
          |
     (success)           (failure)
          |                   |
          v                   v
  +----------------+  +------------------+
  | Handler runs   |  | Error::Unauthorized |
  | with AuthUser  |  | 401 response     |
  +----------------+  +------------------+
```

### Session Validation on App Load

```
  App mount -> AuthProvider -> useEffect
          |
          v
  +------------------------------+
  |  authService.waitForInit()   |
  |  1. Read localStorage        |
  |  2. If token exists:         |
  |     GET /api/auth/me         |
  |     with Bearer token        |
  |  3. If 401: clear session    |
  |  4. If network error: keep   |
  |     session (offline-friendly)|
  +------------------------------+
```

### Cross-Tab Synchronization

The `BackendAuthService` listens for `window.storage` events. When another tab logs in or out, the `"auth_session"` key changes, and all tabs update their session state via the listener callback chain.

### Files Involved

| Step | File |
|------|------|
| JWT issue/validate | `backend/src/auth/jwt.rs` |
| Password hashing | `backend/src/auth/password.rs` |
| Auth middleware (AuthUser extractor) | `backend/src/auth/middleware.rs` |
| Auth handlers (login/register/me) | `backend/src/api/handlers/auth.rs` |
| Frontend auth service | `frontend/src/features/auth/api/auth-service.ts` |
| Frontend auth context | `frontend/src/contexts/AuthContext.tsx` |
| Frontend API client (token attachment) | `frontend/src/lib/api-client.ts` |

### Where Data Can Get Stale

- **JWT expiry is 7 days**: There is no refresh token mechanism. After 7 days, all requests fail with 401 and the user must re-login.
- **Profile changes**: If `is_admin` changes in the database, the JWT still contains the old value until the user re-authenticates. The `updateSessionProfile` method on the auth service only updates localStorage, not the JWT itself.

### Common Failure Modes

- **JWT_SECRET rotation**: If the server's `JWT_SECRET` environment variable changes, all existing tokens become invalid. Every user gets 401 on their next request.
- **localStorage quota exceeded**: Extremely unlikely but `writeToStorage` has no error handling for `localStorage.setItem` quota errors.
- **Missing Authorization header**: Returns `Error::Unauthorized("Missing authorization header")`. The `OptionalAuth` extractor returns `None` instead of erroring, used for endpoints that work with or without auth.
