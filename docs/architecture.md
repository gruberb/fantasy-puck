# Fantasy Puck -- Architecture

Technical architecture reference for the Fantasy Puck monorepo: a fantasy hockey application for NHL playoff pools.

---

## System Overview

Fantasy Puck is a full-stack web application where users create leagues, draft NHL players onto fantasy teams, and track their performance through the NHL playoffs. The system pulls live data from the NHL API, computes rankings and statistics, and delivers real-time draft experiences via WebSocket.

```
                         +-------------------------+
    Users  ----------->  | fantasy-frontend        |  Fly.io (yyz)
    (browser)            | React SPA / nginx       |  Port 80
                         | fantasy-puck.ca         |
                         +----------+--------------+
                                    |
                         +----------v--------------+
                         | fantasy-hockey           |  Fly.io (yyz)
                         | Rust / Axum / Tokio      |  Port 3000
                         | api.fantasy-puck.ca      |
                         +---+------+------+--------+
                             |      |      |
                  +----------+  +---+---+  +-----------+
                  | Supabase |  | NHL   |  | Anthropic  |
                  | Postgres |  | API   |  | API        |
                  +----------+  +-------+  +------------+
```

---

## Technology Stack

### Backend

| Component | Technology | Version |
|-----------|-----------|---------|
| Language | Rust | 2021 edition |
| HTTP framework | Axum | 0.8 |
| Async runtime | Tokio | 1.x (full features) |
| Database driver | SQLx | 0.8 (Postgres, macros, migrations, UUID) |
| HTTP client | Reqwest | 0.12 |
| Serialization | Serde / Serde JSON | 1.x |
| Authentication | jsonwebtoken + argon2 | 9.x / 0.5 |
| WebSocket | Axum built-in WS | via `axum::extract::ws` |
| Scheduling | tokio-cron-scheduler | 0.13 |
| Web scraping | scraper | 0.22 |
| CLI parsing | clap | 4.x |

### Frontend

| Component | Technology | Version |
|-----------|-----------|---------|
| Framework | React | 19 |
| Build tool | Vite | 6.3 |
| Routing | React Router | 7.5 |
| Data fetching | TanStack React Query | 5.74 |
| Styling | Tailwind CSS | 4.1 |
| Language | TypeScript | 5.7 |

### Infrastructure

| Component | Details |
|-----------|---------|
| Hosting | Fly.io, `yyz` region (Toronto) |
| Database | Supabase-hosted PostgreSQL (session pooler, port 5432) |
| Frontend serving | nginx (static SPA, Dockerized) |
| Backend runtime | Debian bookworm-slim (multi-stage Docker build) |

---

## Backend Architecture

### Module Structure

```
backend/src/
  main.rs                   # Entry point: init services, scheduler, start server
  lib.rs                    # Module declarations and re-exports
  error.rs                  # Error types (Database, NhlApi, NotFound, Validation, etc.)
  api/
    mod.rs                  # Server setup, CORS, constants (SEASON, GAME_TYPE)
    routes.rs               # All route definitions + AppState struct
    response.rs             # ApiResponse<T> wrapper for consistent JSON shape
    handlers/               # Request handlers by domain
      auth.rs               # Login, register, get_me, update_profile, delete_account
      leagues.rs            # CRUD for leagues, join, member management
      draft.rs              # Full draft lifecycle (create, start, pick, finalize, sleeper)
      teams.rs              # Fantasy team CRUD, player add/remove
      rankings.rs           # Season, daily, and playoff rankings
      games.rs              # NHL games list, match-day with live updates
      stats.rs              # Top skaters endpoint
      playoffs.rs           # Playoff bracket/carousel
      players.rs            # Fantasy players per NHL team
      team_stats.rs         # Aggregated team statistics
      sleepers.rs           # Sleeper player data
      insights.rs           # AI-generated narratives (Claude API + signal computation)
      nhl_rosters.rs        # NHL team rosters
      admin.rs              # Cache invalidation, manual ranking reprocessing
    dtos/                   # Data transfer objects for API responses
  auth/
    jwt.rs                  # JWT issue (7-day expiry) and validate
    middleware.rs           # AuthUser and OptionalAuth extractors (FromRequestParts)
    password.rs             # Argon2 password hashing and verification
  db/
    mod.rs                  # FantasyDb struct (PgPool wrapper) + delegation methods
    users.rs                # User and profile CRUD, membership queries
    leagues.rs              # League CRUD, member management, join transactions
    teams.rs                # Fantasy team queries
    players.rs              # Fantasy player queries, NHL team grouping
    draft.rs                # Draft sessions, player pool, picks, sleeper picks
    sleepers.rs             # Sleeper player queries
    cache.rs                # CacheService for response_cache table (get/store/invalidate)
  models/
    db.rs                   # Database row types (FantasyTeam, FantasyPlayer, League, etc.)
    fantasy.rs              # Domain models (TeamRanking, DailyRanking, PlayerStats, etc.)
    nhl.rs                  # NHL API response types (Player, GameBoxscore, Playoff, etc.)
  nhl_api/
    nhl.rs                  # NhlClient: rate-limited, caching HTTP client for NHL API
    nhl_constants.rs        # All NHL API endpoint URLs, team name mappings
  ws/
    draft_hub.rs            # DraftHub: broadcast channel manager for draft sessions
    handler.rs              # WebSocket upgrade handler + connection loop
  utils/
    api.rs                  # Shared API helper functions (process_players_for_team, etc.)
    fantasy.rs              # Fantasy point calculations from boxscores
    nhl.rs                  # NHL data utilities (name matching, stat aggregation)
    scheduler.rs            # Cron job setup, daily rankings processing, historical backfill
```

### Request Lifecycle

1. HTTP request arrives at Axum router
2. CORS middleware applies headers
3. Route matched in `routes.rs`, shared `AppState` (db, nhl_client, jwt_secret, draft_hub) injected
4. If route requires auth, `AuthUser` extractor validates JWT from `Authorization: Bearer` header
5. Handler executes business logic, possibly calling `NhlClient` (with caching) and `FantasyDb`
6. Response wrapped in `ApiResponse { success: true, data: T }` or `ErrorResponse { success: false, error: String }`
7. Errors mapped to HTTP status codes via `IntoResponse` impl on the custom `Error` enum

### AppState

```rust
pub struct AppState {
    pub db: FantasyDb,           // Postgres connection pool (max 5 connections)
    pub nhl_client: NhlClient,   // Rate-limited HTTP client with in-memory cache
    pub jwt_secret: String,      // Shared secret for JWT signing/verification
    pub draft_hub: DraftHub,     // WebSocket broadcast channel manager
}
```

Wrapped in `Arc<AppState>` and shared across all routes via Axum's state extraction.

### NhlClient (NHL API Integration)

The `NhlClient` is the sole interface to the NHL's public API (`api-web.nhle.com`). Key characteristics:

- **Rate limiting**: Tokio semaphore limits concurrent requests to 5
- **Retry logic**: Retries up to 3 times on HTTP 429 with exponential backoff (500ms, 1s, 1.5s)
- **In-memory cache**: `HashMap<String, CacheEntry>` behind `Arc<RwLock>`, keyed by full URL
- **Per-endpoint TTLs**: Range from 60 seconds (live boxscores) to 24 hours (final boxscores)
- **Background cleanup**: Spawned task removes expired entries every 5 minutes
- **Adaptive boxscore caching**: Inspects `gameState` in response to choose TTL (1 min for live, 24 hr for final)

### Authentication

- Custom JWT-based auth (not Supabase Auth)
- Passwords hashed with Argon2
- JWTs contain `sub` (user UUID), `email`, `is_admin`, with 7-day expiry
- Two extractors: `AuthUser` (required) and `OptionalAuth` (returns `None` when no token present)
- No refresh token mechanism; users must re-login after 7 days

### Database Layer

`FantasyDb` wraps a `PgPool` and delegates to domain-specific service structs:

- `TeamDbService` -- fantasy team queries
- `PlayerDbService` -- fantasy player queries
- `SleeperDbService` -- sleeper player queries
- `CacheService` -- response cache get/store/invalidate

Direct methods on `FantasyDb` handle leagues, users, and draft operations. All queries use SQLx's compile-time-checked query macros with explicit column casts (`id::text` for UUID-to-string conversion).

Connection pool: 5 max connections with `statement_cache_capacity(0)` for PgBouncer compatibility.

### Scheduled Jobs

The backend runs an in-process cron scheduler (`tokio-cron-scheduler`):

| Job | Schedule (UTC) | Purpose |
|-----|---------------|---------|
| Morning rankings | 9:00 AM | Process yesterday's daily rankings for all leagues |
| Afternoon rankings | 3:00 PM | Second pass for late-finishing games |
| Insights pre-warming | 10:00 AM | Generate and cache Claude-powered insights for all leagues |

On startup, if the `daily_rankings` table is empty and playoffs have started, a historical backfill runs synchronously before the server accepts connections.

### WebSocket (Draft)

Real-time draft updates use a pub/sub model:

1. `DraftHub` manages a `HashMap<String, broadcast::Sender<String>>` keyed by session ID
2. When a client connects to `/ws/draft/{session_id}`, the handler subscribes to the session's broadcast channel
3. Draft mutations (pick, start, pause, resume, sleeper pick) broadcast `DraftEvent` variants to all subscribers
4. The WebSocket handler forwards broadcasts to the client and handles ping/pong for keep-alive
5. Broadcast channel capacity: 64 messages; lagging clients auto-skip missed messages

Event types:
- `SessionUpdated` -- draft state change (status, round, pick index, sleeper status)
- `PickMade` -- a player was drafted
- `SleeperUpdated` -- sleeper round state changed

---

## Frontend Architecture

### Directory Structure

```
frontend/src/
  main.tsx                  # React entry point, QueryClientProvider, BrowserRouter
  App.tsx                   # Route definitions
  config.ts                 # API URL, app constants (season, game type, cache times)
  api/
    client.ts               # High-level API methods (api.getTeams, api.makePick, etc.)
  lib/
    api-client.ts           # fetchApi() -- generic HTTP client with token injection
    react-query.ts          # QueryClient configuration (staleTime: 5min default)
    realtime.ts             # WebSocket client for draft (auto-reconnect with backoff)
  contexts/
    AuthContext.tsx          # Auth state provider (user, profile, signIn/signUp/signOut)
    LeagueContext.tsx        # Active league, memberships, draft session state
  features/                 # Domain-organized feature modules
    auth/                   # Auth service, types
    draft/                  # Draft hooks (session, picks, player pool, sleeper, admin)
    games/                  # Game data hooks
    insights/               # Insights hook
    leagues/                # League types
    rankings/               # Rankings and playoff hooks
    skaters/                # Skater data hook
    teams/                  # Team detail and list hooks
  pages/                    # Route-level page components
    LoginPage.tsx
    LeaguePickerPage.tsx
    HomePage.tsx
    FantasyTeamsPage.tsx
    FantasyTeamDetailPage.tsx
    SkatersPage.tsx
    GamesPage.tsx
    RankingsPage.tsx
    InsightsPage.tsx
    DraftPage.tsx
    AdminPage.tsx
    JoinLeaguePage.tsx
    SettingsPage.tsx
  components/               # Shared and feature-specific UI components
    common/                 # LoadingSpinner, ErrorMessage, PageHeader, RankingTable, etc.
    layout/                 # Layout, NavBar, LeagueShell, ProtectedRoute
    games/                  # Game cards, player comparisons, fantasy summaries
    fantasyTeams/           # Team cards, grid, empty state
    fantasyTeamDetail/      # Roster, playoff status, team stats, bets
    dailyRankings/          # Daily rankings section
    rankingsPageTableColumns/ # Column definitions for ranking tables
    skaters/                # Top skaters table
    matchDay/               # Match day specific components
    home/                   # Home page action buttons
  hooks/                    # Legacy hooks (being migrated to features/)
  services/                 # Legacy service re-exports
  types/                    # Shared TypeScript type definitions
  utils/                    # Formatting, NHL team data, timezone helpers
```

### Routing

The app uses React Router v7 with nested routes:

```
/login                          -- LoginPage (public)
/                               -- LeaguePickerPage (within Layout)
/skaters                        -- SkatersPage (global, not league-scoped)
/games/:date                    -- GamesPage (global)
/admin                          -- AdminPage (protected)
/join-league                    -- JoinLeaguePage (protected)
/settings                       -- SettingsPage (protected)
/league/:leagueId               -- LeagueShell wrapper
  /league/:leagueId/            -- HomePage
  /league/:leagueId/teams       -- FantasyTeamsPage
  /league/:leagueId/teams/:id   -- FantasyTeamDetailPage
  /league/:leagueId/rankings    -- RankingsPage
  /league/:leagueId/insights    -- InsightsPage
  /league/:leagueId/draft       -- DraftPage (protected)
```

`LeagueShell` reads `leagueId` from the URL, sets it as the active league in `LeagueContext`, and renders the nested child route via `<Outlet />`.

`ProtectedRoute` checks `AuthContext` for a logged-in user and redirects to `/login` if absent.

### State Management

**AuthContext** -- Wraps the entire app. Manages user/profile state, delegates to `BackendAuthService` for API calls and localStorage persistence. Supports cross-tab sync via `window.storage` events.

**LeagueContext** -- Manages the currently active league, fetches all leagues and user memberships, and provides the draft session for the active league. Derived state includes `myLeagues` (leagues the user belongs to) and `activeLeague` (the full league object).

**React Query** -- Primary data fetching and caching layer. Global defaults: `staleTime: 5 min`, `retry: false`, `refetchOnWindowFocus: false`. Individual hooks override these (e.g., insights uses `staleTime: 15 min`, games uses 30-second auto-refresh when live).

### API Client

`fetchApi<T>()` in `lib/api-client.ts` is the sole HTTP interface:

- Prepends `API_URL` (defaults to `https://api.fantasy-puck.ca/api`)
- Attaches JWT from `authService.getToken()` as `Authorization: Bearer` header
- Unwraps the `{ success, data }` envelope, throwing on `success: false`
- Supports optional `fallback` values for non-critical endpoints (returns fallback instead of throwing)
- Handles 204 No Content for DELETE operations

`api/client.ts` exposes typed methods (e.g., `api.getTeams(leagueId)`, `api.makePick(draftId, playerPoolId)`) that call `fetchApi`.

### Realtime (WebSocket)

`lib/realtime.ts` provides a `WebSocketRealtimeService`:

- Derives WebSocket URL from API URL (`https:` -> `wss:`, strips `/api` suffix)
- `subscribeToDraft(sessionId, handlers)` creates a WebSocket connection to `/ws/draft/{sessionId}`
- Dispatches incoming messages by `type` field: `sessionUpdated`, `pickMade`, `sleeperUpdated`
- Auto-reconnects with exponential backoff (1s initial, 30s max)
- Returns an unsubscribe function that closes the connection and stops reconnection

---

## Database Schema

All tables live in the `public` schema. Defined in `backend/migrations/001_create_users_table.sql`.

### Entity Relationship

```
users 1--1 profiles
users 1--* fantasy_teams
users 1--* league_members

leagues 1--* league_members
leagues 1--* fantasy_teams
leagues 1--* draft_sessions
leagues 1--* daily_rankings

fantasy_teams 1--* fantasy_players
fantasy_teams 1--1 fantasy_sleepers
fantasy_teams 1--* daily_rankings

league_members *--1 fantasy_teams
league_members 1--* draft_picks

draft_sessions 1--* player_pool
draft_sessions 1--* draft_picks
```

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `users` | Authentication accounts | `id` (UUID), `email`, `password_hash` |
| `profiles` | Display names and admin flag | `id` (FK to users), `display_name`, `is_admin` |
| `leagues` | Fantasy leagues | `id` (UUID), `name`, `season`, `visibility`, `created_by` |
| `league_members` | User-league associations | `league_id`, `user_id`, `fantasy_team_id`, `draft_order` |
| `fantasy_teams` | User teams within leagues | `id` (BIGSERIAL), `name`, `user_id`, `league_id` |
| `fantasy_players` | Drafted player rosters | `team_id`, `nhl_id`, `name`, `position`, `nhl_team` |
| `fantasy_sleepers` | Sleeper round picks | `team_id`, `nhl_id`, `name`, `position`, `nhl_team` |
| `draft_sessions` | Draft state machines | `league_id`, `status`, `current_round`, `current_pick_index`, `snake_draft`, `sleeper_status` |
| `player_pool` | Available players for a draft | `draft_session_id`, `nhl_id`, `name`, `position`, `headshot_url` |
| `draft_picks` | Individual draft selections | `draft_session_id`, `league_member_id`, `nhl_id`, `round`, `pick_number` |
| `daily_rankings` | Historical ranking snapshots | `team_id`, `league_id`, `date`, `rank`, `points` |
| `response_cache` | Server-side response cache | `cache_key` (PK), `data` (JSON text), `date` |

### Cascade Behavior

- Deleting a `user` cascades to `profiles`, `fantasy_teams`, `league_members`
- Deleting a `league` cascades to `league_members`, `draft_sessions`, `fantasy_teams`, `daily_rankings`
- Deleting a `fantasy_team` cascades to `fantasy_players`, `fantasy_sleepers`, `daily_rankings`
- Deleting a `draft_session` cascades to `player_pool`, `draft_picks`

---

## API Surface

### Auth (`/api/auth/*`)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| POST | `/auth/login` | No | Login, returns JWT + user + profile |
| POST | `/auth/register` | No | Register, returns JWT + user + profile |
| GET | `/auth/me` | Yes | Validate token, return current user |
| PUT | `/auth/profile` | Yes | Update display name |
| DELETE | `/auth/account` | Yes | Delete account (cascading) |
| GET | `/auth/memberships` | Yes | Get all league memberships for current user |

### Leagues (`/api/leagues/*`)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/leagues` | Optional | List leagues (all if authed, public-only otherwise) |
| POST | `/leagues` | Yes | Create a league |
| DELETE | `/leagues/{id}` | Yes | Delete a league |
| GET | `/leagues/{id}/members` | Yes | List league members with team info |
| POST | `/leagues/{id}/join` | Yes | Join league (creates team + membership) |
| DELETE | `/leagues/{id}/members/{mid}` | Yes | Remove a member |

### Draft (`/api/draft/*` and `/api/leagues/{id}/draft/*`)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/leagues/{id}/draft` | Yes | Get draft session for league |
| POST | `/leagues/{id}/draft` | Yes | Create draft session |
| POST | `/leagues/{id}/draft/randomize-order` | Yes | Randomize member draft order |
| GET | `/draft/{id}` | Yes | Get full draft state (session + picks + pool) |
| DELETE | `/draft/{id}` | Yes | Delete draft session |
| POST | `/draft/{id}/populate` | Yes | Populate player pool from NHL rosters |
| POST | `/draft/{id}/start` | Yes | Start draft (pending -> active) |
| POST | `/draft/{id}/pause` | Yes | Pause draft |
| POST | `/draft/{id}/resume` | Yes | Resume draft |
| POST | `/draft/{id}/pick` | Yes | Make a pick (broadcasts via WebSocket) |
| POST | `/draft/{id}/finalize` | Yes | Copy picks to fantasy_players |
| POST | `/draft/{id}/complete` | Yes | Mark draft as completed |
| GET | `/draft/{id}/sleepers` | Yes | Get eligible sleeper players |
| GET | `/draft/{id}/sleeper-picks` | Yes | Get sleeper picks made |
| POST | `/draft/{id}/sleeper/start` | Yes | Start sleeper round |
| POST | `/draft/{id}/sleeper/pick` | Yes | Make a sleeper pick |

### Fantasy Teams (`/api/fantasy/*`)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/fantasy/teams?league_id=` | No | List teams with player counts |
| GET | `/fantasy/teams/{id}?league_id=` | No | Team detail with stats |
| PUT | `/fantasy/teams/{id}` | Yes | Update team name |
| POST | `/fantasy/teams/{id}/players` | Yes | Add player to team |
| DELETE | `/fantasy/players/{id}` | Yes | Remove player from team |
| GET | `/fantasy/rankings?league_id=` | No | Season rankings (computed from NHL stats) |
| GET | `/fantasy/rankings/daily?league_id=&date=` | No | Daily rankings from boxscores |
| GET | `/fantasy/rankings/playoffs?league_id=` | No | Playoff-specific rankings |
| GET | `/fantasy/team-bets?league_id=` | No | NHL team exposure per fantasy team |
| GET | `/fantasy/players?league_id=` | No | All players grouped by NHL team |
| GET | `/fantasy/team-stats?league_id=` | No | Aggregated team statistics |
| GET | `/fantasy/sleepers?league_id=` | No | Sleeper player list with stats |

### NHL Data (`/api/nhl/*`)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/nhl/skaters/top` | No | Top skaters with form stats |
| GET | `/nhl/games?date=` | No | NHL games for a date |
| GET | `/nhl/playoffs?season=` | No | Playoff bracket |
| GET | `/nhl/match-day?league_id=` | No | Match day with fantasy player stats |
| GET | `/nhl/roster/{team}` | No | NHL team roster |

### Other

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/insights?league_id=` | No | AI-generated insights (Claude API) |
| GET | `/admin/process-rankings/{date}` | No* | Trigger ranking reprocessing |
| GET | `/admin/cache/invalidate` | No* | Invalidate caches |
| WS | `/ws/draft/{session_id}` | No | WebSocket for draft real-time updates |

*Admin endpoints lack authentication -- see known issues.

---

## Caching Architecture

The system has three caching layers:

### Layer 1: NhlClient In-Memory Cache

- **Location**: Backend process memory
- **Implementation**: `HashMap<URL, CacheEntry>` behind `Arc<RwLock>`
- **TTLs**: Per-endpoint, from 60 seconds (live boxscores) to 24 hours (final boxscores)
- **Cleanup**: Background task every 5 minutes removes expired entries
- **Invalidation**: Full clear via admin endpoint only; no per-key invalidation

### Layer 2: Database Response Cache

- **Location**: `response_cache` table in Postgres
- **Implementation**: Key-value store with JSON text values
- **TTL**: None (entries persist until explicit deletion or overwrite)
- **Key patterns**: `match_day:{league}:{date}`, `games_extended:{league}:{date}`, `insights:{league}:{date}`
- **Smart bypass**: Match-day and games handlers check for live games and update cached responses

### Layer 3: React Query Client-Side Cache

- **Location**: Browser memory
- **Default staleTime**: 5 minutes (data considered fresh, no refetch)
- **Default gcTime**: 5 minutes (React Query default; inactive data garbage collected)
- **Overrides**: Insights (15 min stale, 1 hr gc), Games (30s auto-refresh when live)
- **Optimistic updates**: Draft hooks use `setQueryData` for instant UI updates via WebSocket

### Cache Interaction

A typical request traverses all three layers:

1. React Query checks if data is fresh (Layer 3). If so, returns immediately with no network request.
2. If stale, `fetchApi` sends a request to the backend.
3. Backend handler checks `response_cache` (Layer 2). If hit and no live games, returns cached response.
4. If cache miss or live games detected, `NhlClient` checks its in-memory cache (Layer 1).
5. If Layer 1 misses, fetches from the NHL API with rate limiting and retry.
6. Response stored in Layer 2, returned to frontend, cached in Layer 3.

---

## Draft System

### Lifecycle States

```
pending -> active -> picks_done -> (finalize) -> sleeper round active -> completed
```

### Snake Draft Algorithm

- Members are ordered by `draft_order` (randomizable)
- Even rounds: pick in order (0, 1, 2, ...)
- Odd rounds: pick in reverse order (..., 2, 1, 0)
- Global `current_pick_index` tracks position across all rounds
- Round is computed as `pick_index / num_members`
- When all rounds complete, status changes to `picks_done`

### Finalization

`finalize_draft` copies all `draft_picks` to `fantasy_players` in a transaction:
1. Joins `draft_picks` with `league_members` to get `fantasy_team_id`
2. Inserts each pick into `fantasy_players` (ON CONFLICT DO NOTHING)
3. Marks the session as `completed`

### Sleeper Round

After the main draft completes, an optional sleeper round allows each team to pick one additional "sleeper" player from undrafted players in the pool. Sleeper picks are stored in `fantasy_sleepers` (separate from `fantasy_players`).

---

## Insights System

The insights feature generates AI-powered hockey narratives using the Anthropic Claude API.

### Signal Pipeline

Six concurrent signal computations run via `tokio::join!`:

1. **Hot Players** -- Top 5 by recent form (L5 games), enriched with NHL Edge analytics
2. **Cup Contenders** -- Current playoff series leads from the carousel endpoint
3. **Today's Games** -- Schedule + standings + yesterday's scores for matchup context
4. **Fantasy Race** -- League rankings with active player counts
5. **Sleeper Alerts** -- Sleeper player stats from NHL API
6. **Headlines** -- Web-scraped news from DailyFaceoff

### Generation Flow

1. Compute all signals concurrently
2. Send signals as JSON context to Claude (claude-haiku-4-5, 3072 max tokens)
3. Extract structured JSON from Claude's response
4. Cache in `response_cache` with date-scoped key
5. Pre-warmed daily at 10am UTC by the scheduler

Fallback: If the Anthropic API key is missing or the API fails, static placeholder narratives are returned.

---

## Deployment

Both services are deployed to Fly.io using multi-stage Docker builds.

### Backend

- Builder: `rust:1.88` compiles a release binary
- Runtime: `debian:bookworm-slim` with `libssl3` and `ca-certificates`
- Binary: `./fantasy-hockey serve --port 3000`
- VM: 1 shared CPU, 1 GB memory, always-on (auto-stop disabled)

### Frontend

- Builder: `node:20.18.0-slim` runs `npm ci && npm run build`
- Runtime: `nginx` serves static files from `/usr/share/nginx/html`
- `VITE_API_URL` must be set at build time (Vite inlines env vars)
- VM: 1 shared CPU, 1 GB memory, always-on

### Environment Variables

**Backend (runtime secrets):**

| Variable | Required | Purpose |
|----------|----------|---------|
| `DATABASE_URL` | Yes | Supabase PostgreSQL connection string (session pooler, port 5432) |
| `JWT_SECRET` | Yes | Secret for JWT signing/verification |
| `ANTHROPIC_API_KEY` | No | Claude API key for insights generation |

**Frontend (build-time):**

| Variable | Required | Purpose |
|----------|----------|---------|
| `VITE_API_URL` | No | Backend API URL (defaults to `https://api.fantasy-puck.ca/api`) |

---

## Cross-References

For detailed information on specific topics, see the companion documents:

- **[data-flow.md](data-flow.md)** -- End-to-end data pipeline traces for all major flows
- **[caching.md](caching.md)** -- Detailed caching layer documentation with debugging guide
- **[operations.md](operations.md)** -- Deployment, monitoring, season changeover, troubleshooting
- **[known-issues.md](known-issues.md)** -- Comprehensive bug list and technical debt
