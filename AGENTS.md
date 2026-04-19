# Fantasy Puck

Monorepo: Rust/Axum backend + React/TypeScript/Vite frontend, Supabase PostgreSQL database.

## Commands

```bash
make run              # Start everything: Supabase DB + backend (port 3000) + frontend
make check            # Type-check both backend and frontend
make db-reset         # Reset DB and reapply migrations from backend/supabase/migrations/
make cache-clear      # Clear response cache
```

Frontend only:
```bash
cd frontend
npm run validate      # Typecheck + ESLint (run before committing frontend changes)
npm run dev           # Dev server only
```

Backend only:
```bash
cd backend
cargo check           # Type-check
cargo run -- serve --port 3000
```

## Documentation

In-depth technical documentation lives in `/docs` (11 files, indexed by `docs/README.md`): backend architecture, database schema, HTTP API, NHL integration, prediction engine, business logic, background jobs, draft system, frontend architecture, frontend data flow. Consult it before making non-trivial changes so you understand the surrounding system, and cite it when you need to justify a decision.

Keep `/docs` in sync: after every change that affects behaviour described in those files, update the relevant doc in the same commit. Dead docs are worse than no docs — they mislead and erode trust. If a change doesn't touch anything documented, say so explicitly in your summary so the reviewer knows you checked.

## Code style

- Frontend: Tailwind CSS with brutalist design system (no rounded corners, thick borders `border-2`, bold uppercase text, `tracking-wider`). CSS variables in `frontend/src/index.css`.
- Import paths use `@/` alias (maps to `frontend/src/`)
- React Query for data fetching; hooks live in `frontend/src/features/{feature}/hooks/`
- Backend: Axum handlers in `backend/src/api/handlers/`, DTOs in `backend/src/api/dtos/`. IMPORTANT: No SQL in route handlers — all database queries go in `backend/src/db/` modules. Handlers call db functions, never `sqlx::query` directly.
- NHL API client in `backend/src/nhl_api/nhl.rs` — undocumented API at `api-web.nhle.com`, has built-in rate limiting and caching
- Comments: write in the voice of a Staff Software Engineer addressing teammates and future maintainers, never in the voice of an LLM. That means: no restating what well-named code already says, no mentioning the current task / PR / fix ("added for X", "handles case from issue Y"), no hedging or marketing tone. A comment earns its place only when the *why* is non-obvious — a hidden constraint, a subtle invariant, a workaround for a specific bug, or behaviour that would surprise a reader. If removing the comment would not confuse a future reader, do not write it.

## Versioning

- IMPORTANT: Only bump the version of the package that actually changed. Frontend-only change → bump `frontend/package.json` only. Backend-only → `backend/Cargo.toml` only. Don't bump both unless both changed.
- CHANGELOG.md and git tags always get updated regardless.
- Use annotated tags: `git tag -a v1.x.x -m "message"`

## Git

- Commit messages: imperative mood, short first line. `fix:`, `feat:`, `chore:` prefixes.
- Auto-deploy: pushing to `main` triggers Fly.io deploy for the paths that changed (backend/** or frontend/**)
- Never add Co-Authored-By trailers.

## Release workflow

After every substantial change (new feature, meaningful fix, docs rewrite, anything a user or operator would want to know about), ask whether to run the release workflow before moving on. Do not run it unprompted.

The workflow is:

1. Stage the changes and write a concise commit message (`feat:` / `fix:` / `chore:` / `docs:` prefix).
2. Bump the package version that actually changed (`backend/Cargo.toml` or `frontend/package.json`; both only if both changed). Docs-only changes follow the user's preference at ask-time.
3. Add a CHANGELOG.md entry under a new `## vX.Y.Z - YYYY-MM-DD` header describing what changed and why. Description-only - no commands, curl snippets, or next-steps advice.
4. Commit everything together. If `cargo check` or `npm run validate` is relevant, run it first and only commit on green.
5. Push to `main` (this triggers the Fly.io deploy for the paths that changed).
6. Create an annotated git tag: `git tag -a vX.Y.Z -m "message"` and `git push origin vX.Y.Z`.

For multi-phase plans, commit each phase locally but hold the push + tag until the whole plan is done.

## Architecture notes

- Navigation adapts to three states: no league → league without team → league with team. See `NavBar.tsx` and `LeagueContext.tsx`.
- `useInsights()` works with or without a league ID. Global route at `/insights`, league-scoped at `/league/:id/insights`.
- Draft system uses WebSocket (`DraftHub`) for real-time updates.
- Scheduled background tasks in `backend/src/utils/scheduler.rs` (daily rankings, playoff info, insights).
- AI insights use the Anthropic API; generated narratives are cached per hockey-date in the `response_cache` table.

## Switching season / game type

Season and game-type flow from two env vars on each side. All visible labels and API calls derive from these.

Backend (`backend/.env`):
- `NHL_SEASON` (u32, default `20252026`) — 8-digit YYYYYYYY
- `NHL_GAME_TYPE` (u8, default `3`) — `2` = Regular Season, `3` = Playoffs

Frontend (`frontend/.env`):
- `VITE_NHL_SEASON` (string, default `20252026`)
- `VITE_NHL_GAME_TYPE` (number, default `3`)

Flip workflow:
1. Update both envs, restart (`make run`).
2. For drafts started under the old game type, admin hits "Repopulate Player Pool" → `POST /api/draft/:id/populate`. The backend emits a WS `playerPoolUpdated` event and connected clients refresh.
3. Existing leagues keep their own `season` column (set at creation). Only the global default changes.

Player-pool sourcing (`backend/src/utils/player_pool.rs`):
- `game_type == 3`: 16 playoff team rosters via `/playoff-series/carousel/{season}` (falls back to top 16 from standings if the carousel isn't published yet).
- Otherwise: skater-stats leaders across 9 categories.

Cache keys in `insights.rs` and `games.rs` include `game_type()` so regular-season and playoff payloads cannot collide across a flip.
