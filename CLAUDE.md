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

## Code style

- Frontend: Tailwind CSS with brutalist design system (no rounded corners, thick borders `border-2`, bold uppercase text, `tracking-wider`). CSS variables in `frontend/src/index.css`.
- Import paths use `@/` alias (maps to `frontend/src/`)
- React Query for data fetching; hooks live in `frontend/src/features/{feature}/hooks/`
- Backend: Axum handlers in `backend/src/api/handlers/`, DTOs in `backend/src/api/dtos/`. IMPORTANT: No SQL in route handlers — all database queries go in `backend/src/db/` modules. Handlers call db functions, never `sqlx::query` directly.
- NHL API client in `backend/src/nhl_api/nhl.rs` — undocumented API at `api-web.nhle.com`, has built-in rate limiting and caching

## Versioning

- IMPORTANT: Only bump the version of the package that actually changed. Frontend-only change → bump `frontend/package.json` only. Backend-only → `backend/Cargo.toml` only. Don't bump both unless both changed.
- CHANGELOG.md and git tags always get updated regardless.
- Use annotated tags: `git tag -a v1.x.x -m "message"`

## Git

- Commit messages: imperative mood, short first line. `fix:`, `feat:`, `chore:` prefixes.
- Auto-deploy: pushing to `main` triggers Fly.io deploy for the paths that changed (backend/** or frontend/**)
- Never add Co-Authored-By trailers.

## Architecture notes

- Navigation adapts to three states: no league → league without team → league with team. See `NavBar.tsx` and `LeagueContext.tsx`.
- `useInsights()` works with or without a league ID. Global route at `/insights`, league-scoped at `/league/:id/insights`.
- Draft system uses WebSocket (`DraftHub`) for real-time updates.
- Scheduled background tasks in `backend/src/utils/scheduler.rs` (daily rankings, playoff info, insights).
- AI insights use the Anthropic API; generated narratives are cached per hockey-date in the `response_cache` table.
