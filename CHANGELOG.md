# Changelog

All notable changes to Fantasy Puck are documented here.

## Unreleased

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
