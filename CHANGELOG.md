# Changelog

All notable changes to Fantasy Puck are documented here.

## Unreleased

## v1.3.1 — 2026-04-10

### Fixed
- **Leagues nav link** — always visible for logged-out users browsing a league, so they can navigate back to the league picker

## v1.3.0 — 2026-04-10

### Added
- **Global Insights page** — Insights now accessible at `/insights` without selecting a league; shows NHL-wide game previews, hot players, and contenders

### Changed
- **Nav rework based on context** — navigation adapts to three states:
  - No league selected: Leagues, Games, Insights, Skaters
  - League selected, no team: Dashboard, Insights, Games, Stats, Skaters (Pulse hidden)
  - League selected, has team: Dashboard, Pulse, Insights, Games, Stats, Skaters
- **Leagues nav link** — now visible for all users when no league is selected (was login-only)

### Fixed
- **Insights game card header** — team name, record, and streak info stacked vertically so long names like "Maple Leafs" no longer push the record out of alignment

## v1.2.1 — 2026-04-09

### Fixed
- **Insights mobile layout** — game card player stats and goalie info no longer float/jump on narrow screens; stats stack vertically on mobile (side-by-side on desktop), player names truncate reliably, goalie record and save stats split into stable lines

## v1.2.0 — 2026-04-09

### Added
- **Pulse page** — new quick-glance dashboard (Dashboard > Pulse in nav) showing: my team rank/points/today, players grouped by tonight's games with start times, and league board with opponent activity
- **Sleeper delete endpoint** — `DELETE /api/fantasy/sleepers/:id` for removing sleeper picks
- **Sleeper management in admin** — sleepers now visible in Player Management with yellow badge and Remove button
- **Makefile improvements** — `make run` waits for backend to be ready before starting frontend; `make cache-clear` to wipe response cache

### Changed
- **Nav restructure** — Dashboard, Pulse, Insights, Games, Stats, Skaters in main nav; Teams moved to dropdown alongside Browse Leagues and Manage Leagues
- **Games page simplified** — removed My League and Player Matchups tabs; Games page now shows only NHL game cards
- **Insights narratives** — Claude no longer prefixes game narratives with matchup labels (e.g. "CBJ @ BUF:"); streak labels now readable ("Won 2" instead of "W2")
- **Insights layout** — game cards in 2-column grid on desktop
- **Fantasy summary and team cards** — redesigned with consistent black/white headers, compact player rows
- **Player matchups** — team logos instead of colored squares, compact VS rows
- **Pulse headers** — white background with black text, consistent across all sections

### Fixed
- **Draft finalize propagation** — non-owners now see sleeper round transition without page reload (invalidateQueries on sessionUpdated WS event)
- **Player delete** — admin page now correctly deletes players by NHL ID (was sending NHL ID to an endpoint expecting DB ID)
- **Admin player count** — includes sleeper in the total count per team
- **Admin player list** — correctly parses nested NHL-team-grouped API response instead of expecting flat array
- **AdminPage infinite loop** — fixed useEffect dependency on `members` array reference causing re-render loop
- **Dashboard post-draft-delete** — shows rankings instead of "Draft Hasn't Started" when teams have data but draft session was deleted
- **Sleeper visibility** — sleeper stays visible in admin even when all regular players are removed

### Removed
- GameTabs, FantasySummary, FantasyTeamCard, PlayerComparison, PlayerWithStats, FantasyTeamSummary components
- useFantasyTeams hook
- matchDay duplicate components

## v1.1.0 — 2026-04-08

### Fixed
- Draft state not propagating to other participants — finalize (sleeper transition) and complete (draft done) now update all clients in real-time without requiring a page reload. Root cause: LeagueContext and useDraftSession cached the same draft session under different React Query keys, so WebSocket updates only reached one of them.
- Makefile `run` target now always uses local dev database (`.env.development`), never connects to production
- Supabase local config slimmed to Postgres-only (no auth, storage, realtime, studio, edge runtime) — faster startup, fewer Docker images

### Changed
- Backend loads `.env` via standard dotenv (`.env.development` is copied to `.env` by Makefile)

## v1.0.0 — 2026-04-08

Initial stable release as a monorepo (`backend/` + `frontend/`).

### Features
- **NHL API integration** with in-memory caching (12 endpoint-specific TTLs) and semaphore-based rate limiting
- **Fantasy leagues** — create/join leagues, manage teams, snake draft with real-time WebSocket
- **AI-powered insights** — per-game narratives via Claude API, with standings, NHL Edge analytics, yesterday's scores
- **Playoff tracking** — daily rankings, historical performance, playoff bracket
- **Scheduled jobs** — rankings at 9am/3pm UTC, insights pre-warming at 10am UTC, weekly cache cleanup
- **JWT authentication** with Argon2 password hashing

### Bug Fixes (post-v1.0.0, pre-release)
- Admin endpoints now require JWT + `is_admin` check
- Player matching uses `nhl_id` (primary) with last-name fallback instead of fragile substring matching
- DST timezone handling uses `chrono-tz` America/New_York instead of crude month-range approximation
- Startup backfill runs in background (non-blocking) so Fly.io health checks pass
- Single WebSocket connection per draft page (was 3 independent connections)
- `daily_rankings` UNIQUE constraint includes `league_id`; goals/assists columns now populated
- Weekly cleanup of `response_cache` entries older than 7 days
- Orphaned sleeper scoping fixed (no longer leaks across leagues)
- Server-side WebSocket ping every 30s for keepalive through proxies
- `window.location.reload()` replaced with React Query invalidation / React Router navigation
- LeagueContext refactored from raw useEffect to React Query (caching, dedup, shared query keys)
- Season config moved to env vars (`NHL_SEASON`, `NHL_GAME_TYPE`, `NHL_PLAYOFF_START`, `NHL_SEASON_END`)
- Removed unused `@supabase/supabase-js` dependency and dead `DEFAULT_QUERY_OPTIONS` config
- Headline scraper logs warning when returning 0 results
- `search_players` searches all teams instead of stopping after first match

### Infrastructure
- Monorepo structure: `backend/` (Rust/Axum) + `frontend/` (React/Vite)
- Local dev via Supabase CLI (`make run` starts Postgres + backend + frontend)
- `.env.development` for local, Fly.io secrets for production
- Makefile with `run`, `dev`, `db-start`, `db-reset`, `install`, `check`, `deploy`
- Technical documentation in `docs/` (architecture, API reference, data flow, caching, operations)
