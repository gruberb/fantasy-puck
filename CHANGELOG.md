# Changelog

All notable changes to Fantasy Puck are documented here.

## Unreleased

## v1.5.0 — 2026-04-17

### Added
- **Playoff draft pool** — when `NHL_GAME_TYPE=3`, the draft player pool sources from the 16 playoff team rosters via `/playoff-series/carousel/{season}` + `/roster/{team}/current` instead of the `skater-stats-leaders` endpoint, which returns 0 players until playoff games have been played. Falls back to the top 16 teams from standings if the carousel hasn't been published yet. New helper module at `backend/src/utils/player_pool.rs` is shared with `/nhl/skaters/top`.
- **`PlayerPoolUpdated` WebSocket event** — broadcast when an admin repopulates the pool; draft clients invalidate their player-pool query and see the fresh roster without a manual refresh.
- **Config-derived UI labels** — `APP_CONFIG` exposes `SEASON_LABEL` ("2025/2026 Playoffs"), `GAME_TYPE_LABEL`, and `BRAND_LABEL` ("NHL 2026"), all computed from `VITE_NHL_SEASON` / `VITE_NHL_GAME_TYPE`. Flipping two env vars per side now retargets the whole app to any season or game type.
- **Season/game-type flip workflow documented** in `CLAUDE.md`.

### Fixed
- **Games page missed fantasy overlay** — `useGamesData` was calling `api.getGames(date)` without forwarding `activeLeagueId`, so every game rendered "No fantasy team has players for this team" even when players were rostered. Now forwards the league id and keys the React Query cache by it.
- **Hard refresh dropped the user out of their league** — `LeagueProvider` initialized `activeLeagueId` to `null` and never rehydrated from `localStorage.lastViewedLeagueId`. Global routes like `/games/:date` (which don't run `LeagueShell`) lost the active league on refresh. Lazy state initializer now reads the key on first mount.
- **Hardcoded `game_type=2` in `create_draft_session`** removed — both draft-creation and populate-pool paths now honor the configured `game_type()`.

### Changed
- **Cache hygiene** — response-cache keys for `insights`, `games_extended`, and `match_day` now include `game_type()` so payloads don't collide across a regular-season → playoffs flip. Old keys age out via the existing 7-day cleanup.
- **`/nhl/skaters/top`** — when `game_type=3`, serves from the playoff roster pool (same source as the draft) instead of the empty skater-stats-leaders endpoint.
- **All hardcoded `"2025/2026 Playoffs"`, `"NHL 2026"`, and `"20252026"` literals** in the frontend now read from `APP_CONFIG` (HomePage, RankingsPage, DraftPage, AdminPage, LoginPage, LeaguePickerPage, LeagueSettingsPage, NavBar, TeamBetsTable, PlayerRoster, `api/client.ts`).

## v1.4.0 — 2026-04-15

### Added
- **League-scoped settings page** — `/league/:id/settings` replaces the monolithic admin page for managing a single league's members, draft, and player pool
- **Rich league preview for non-members** — visiting a league via invite link now shows members list, draft status, and a prominent join CTA
- **Join from league picker** — non-member public leagues show a "Join" button directly on the card alongside "View League"
- **League-specific invite links** — "Copy Invite Link" now copies `/league/:id` instead of a generic `/join-league` URL
- **Login return-to support** — after signing in via an invite link, users are redirected back to the league page
- **Health check endpoints** — `GET /health/live` and `GET /health/ready` (verifies DB connectivity)
- **Typed config module** — `Config::from_env()` loads all settings eagerly at startup with clear panic messages for missing vars
- **DB authorization helpers** — `verify_league_owner`, `verify_user_in_league`, `get_league_id_for_draft/team/player`

### Changed
- **Create league flow** — now prompts for team name alongside league name, auto-joins the creator, and navigates to the league dashboard
- **Admin page simplified** — shows only "Create League" form and a grid of owned leagues linking to per-league settings
- **NavBar** — "Manage Leagues" renamed to "My Leagues"; new "League Settings" link for league owners
- **`/join-league` retired** — now redirects to `/league/:id` or `/` (old links still work)
- **Backend authorization hardened** — all draft, league member, team, and player endpoints now verify the caller is a league member or owner (previously only checked authentication)
- **JWT secret wrapped in `secrecy::SecretString`** — prevents accidental logging of the secret
- **Password hashing moved to blocking threads** — `hash_password`/`verify_password` run on `spawn_blocking` to avoid stalling the async runtime
- **HTTP middleware stack** — added gzip compression, 30s request timeout, 1MB body limit, configurable CORS origins
- **Graceful shutdown** — server handles SIGTERM/Ctrl+C cleanly
- **Structured logging** — JSON format via `LOG_JSON=true`, env-filter support via `RUST_LOG`
- **Error handling** — new `Conflict` (409) variant; NHL API errors no longer leak internal details

### Fixed
- **Total picks display** — admin draft stats now show correct pick count (was off-by-one showing 0-based index) and includes sleeper picks in the total

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
