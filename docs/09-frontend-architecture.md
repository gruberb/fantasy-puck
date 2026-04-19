# Frontend architecture

Structure, routing, and state management for the React/Vite app under [`frontend/`](../frontend/). The data-fetching details (React Query keys, per-page staleTimes, WebSocket wiring) are in [`10-frontend-data-flow.md`](./10-frontend-data-flow.md).

## Stack

| Concern | Choice |
| --- | --- |
| Build / dev server | Vite |
| UI library | React (function components, hooks) |
| Language | TypeScript |
| Styling | Tailwind CSS with CSS custom properties in `frontend/src/index.css` |
| Data fetching | `@tanstack/react-query` |
| Routing | `react-router-dom` |
| WebSocket client | Native `WebSocket` wrapped in [`lib/realtime.ts`](../frontend/src/lib/realtime.ts) |

Import alias: `@/*` maps to `frontend/src/*` (see `tsconfig.json`). Commands (`npm run validate` runs typecheck + ESLint) live in [`CLAUDE.md`](../CLAUDE.md).

## Design system

Brutalist: no rounded corners, thick borders (`border-2`), bold uppercase text, `tracking-wider`. CSS variables in [`frontend/src/index.css`](../frontend/src/index.css). Avoid the usual AI-generated "rounded everything with a gradient" aesthetic.

## Directory layout

```
frontend/src/
├── App.tsx                 # Routes and top-level providers
├── main.tsx                # React entry; mounts App
├── config.ts               # APP_CONFIG + QUERY_INTERVALS
├── index.css               # Tailwind + CSS variables
├── vite-env.d.ts
│
├── api/                    # Low-level HTTP helpers
├── lib/
│   ├── api-client.ts       # `fetchApi` wrapper around fetch+envelope
│   ├── react-query.ts      # QueryClient with global defaults
│   └── realtime.ts         # WebSocketRealtimeService (draft only)
│
├── contexts/
│   ├── AuthContext.tsx     # Current session, sign-in/out, cross-tab sync
│   └── LeagueContext.tsx   # Active league; memberships; last-viewed persistence
│
├── hooks/                  # Cross-feature generic hooks
├── services/               # Cross-feature services
├── types/                  # Cross-feature TypeScript types
├── utils/                  # Pure utilities (format, math, date)
│
├── features/               # Feature-scoped code (hooks, api, components, types)
│   ├── auth/
│   ├── draft/
│   ├── games/
│   ├── insights/
│   ├── leagues/
│   ├── pulse/
│   ├── race-odds/
│   ├── rankings/
│   ├── skaters/
│   └── teams/
│
├── components/             # Shared UI components
│   ├── common/
│   ├── dailyRankings/
│   ├── fantasyTeamDetail/
│   ├── fantasyTeams/
│   ├── games/
│   ├── home/
│   ├── layout/
│   │   ├── Layout.tsx
│   │   ├── LeagueShell.tsx
│   │   └── ProtectedRoute.tsx
│   ├── matchDay/
│   ├── pulse/
│   ├── rankingsPageTableColumns/
│   ├── skaters/
│   └── ui/
│
└── pages/                  # Top-level route components
    ├── AdminPage.tsx
    ├── DraftPage.tsx
    ├── FantasyTeamDetailPage.tsx
    ├── FantasyTeamsPage.tsx
    ├── GamesPage.tsx
    ├── HomePage.tsx
    ├── InsightsPage.tsx
    ├── JoinLeaguePage.tsx
    ├── LeaguePickerPage.tsx
    ├── LeagueSettingsPage.tsx
    ├── LoginPage.tsx
    ├── PulsePage.tsx
    ├── RankingsPage.tsx
    ├── SettingsPage.tsx
    └── SkatersPage.tsx
```

Each `features/*` folder is self-contained:

```
features/draft/
├── api/               # fetch helpers specific to this feature
├── components/        # draft-only UI pieces
├── hooks/             # React Query hooks (useDraftSession, usePlayerPool, ...)
├── types/             # TS types (DraftSession, DraftPick, ...)
└── index.ts           # Public surface
```

## Routing

Defined in [`frontend/src/App.tsx`](../frontend/src/App.tsx). Everything except `/login` renders inside `<Layout />` (the global chrome). League-scoped pages render additionally inside `<LeagueShell />`.

```
/login                             LoginPage                 (no Layout)
/                                  LeaguePickerPage          (Layout)
/skaters                           SkatersPage               (Layout)
/games/:date                       GamesPage                 (Layout)
/insights                          InsightsPage              (Layout, global variant)
/admin                             AdminPage                 (Layout + ProtectedRoute)
/join-league                       JoinLeaguePage            (Layout)
/settings                          SettingsPage              (Layout + ProtectedRoute)

/league/:leagueId                  LeagueShell
   (index)                           HomePage
   /teams                            FantasyTeamsPage
   /teams/:teamId                    FantasyTeamDetailPage
   /rankings                         RankingsPage
   /insights                         InsightsPage            (league variant)
   /pulse                            PulsePage
   /draft                            DraftPage               (ProtectedRoute)
   /settings                         LeagueSettingsPage      (ProtectedRoute)
```

Three routes are auth-gated via `<ProtectedRoute>`: `/admin`, `/settings`, `/league/:id/draft`, `/league/:id/settings`. The rest are viewable without signing in; pages that need auth-specific data simply return a placeholder when `user` is null.

`InsightsPage` is used in both the global `/insights` route and the league-scoped `/league/:id/insights` route. The component decides which variant it is showing by reading `activeLeagueId` from `LeagueContext` - the hook `useInsights` passes `league_id` to the backend when it's set.

## Global providers

From [`App.tsx:23-61`](../frontend/src/App.tsx):

```tsx
<AuthProvider>
  <LeagueProvider>
    <Routes>...</Routes>
  </LeagueProvider>
</AuthProvider>
```

`AuthProvider` comes first so `LeagueProvider` can read the current user when loading memberships. React Query's `QueryClientProvider` wraps the whole app in `main.tsx` (not shown above).

### AuthContext

File: [`frontend/src/contexts/AuthContext.tsx`](../frontend/src/contexts/AuthContext.tsx). Reads from and forwards to the singleton `authService` ([`features/auth/api/auth-service.ts`](../frontend/src/features/auth/api/auth-service.ts)).

Session flow:

1. On construction, `BackendAuthService` reads the session from `localStorage["auth_session"]` and stores it in memory ([`auth-service.ts:12-27`](../frontend/src/features/auth/api/auth-service.ts)).
2. In the background, it hits `GET /api/auth/me` with the token. If the response is not OK, the session is cleared and listeners are notified ([`auth-service.ts:63-88`](../frontend/src/features/auth/api/auth-service.ts)). A network error leaves the session intact - useful for offline navigation.
3. `login()` / `register()` POST to the backend, store the returned session, notify listeners.
4. A `window.addEventListener("storage", ...)` handler syncs sessions across open tabs ([`auth-service.ts:19-27`](../frontend/src/features/auth/api/auth-service.ts)).

`AuthContext` exposes `user`, `profile`, `loading`, `signIn`, `signUp`, `signOut`. All listeners are triggered when `authService.session` changes.

### LeagueContext

File: [`frontend/src/contexts/LeagueContext.tsx`](../frontend/src/contexts/LeagueContext.tsx).

Holds `activeLeagueId` (persisted to `localStorage["lastViewedLeagueId"]`), fetches the current user's memberships via React Query, and exposes:

| Field | Type | Source |
| --- | --- | --- |
| `activeLeagueId` | `string \| null` | `useState` backed by localStorage |
| `setActiveLeagueId` | `(id \| null) => void` | Writes to state + localStorage |
| `activeLeague` | `League \| null` | Derived from `allLeagues` |
| `allLeagues` | `League[]` | `GET /api/leagues` |
| `leaguesLoading` | boolean | React Query loading flag |
| `myMemberships` | `LeagueMembership[]` | `GET /api/auth/memberships` |
| `myLeagues` | `League[]` | Derived: `allLeagues` filtered by membership |
| `draftSession` | `DraftSession \| null` | `GET /api/leagues/{id}/draft` for active league |

Persisting `activeLeagueId` to localStorage matters because global routes like `/games/:date` render outside `<LeagueShell />` but still need to know which league the user was last in for overlay features (rostered-player tags, etc.).

## `LeagueShell`

File: [`frontend/src/components/layout/LeagueShell.tsx`](../frontend/src/components/layout/LeagueShell.tsx).

Renders when the user navigates to `/league/:leagueId/*`. Responsibilities:

- Reads `:leagueId` from the URL and sets it as the active league.
- Renders the league-scoped navigation (Home / Teams / Rankings / Insights / Pulse / Draft / Settings).
- Wraps `<Outlet />` so child routes render inside this shell.

The navigation bar adapts to three cases - no league selected, league without team, league with team - per [`CLAUDE.md`](../CLAUDE.md) under "Architecture notes".

## Feature folders

| Folder | What it owns |
| --- | --- |
| `features/auth/` | Session service, login/register forms, `useAuth` hook, account deletion |
| `features/draft/` | Draft session hook, pick mutation, player-pool query, sleeper-round flow, WebSocket subscription glue |
| `features/games/` | Today/past/future game listings with fantasy overlays |
| `features/insights/` | `useInsights`, the signal/narrative types, headline renderer |
| `features/leagues/` | Create/join league forms, member listing, league settings |
| `features/pulse/` | Live dashboard hook with polling when games are live |
| `features/race-odds/` | Monte Carlo odds display hook |
| `features/rankings/` | Daily/season/playoff ranking tables |
| `features/skaters/` | Top skaters leaderboard |
| `features/teams/` | Fantasy team CRUD, add/remove players |

Each feature folder's `hooks/` directory contains React Query hooks. The per-page data-fetching map is in [`10-frontend-data-flow.md`](./10-frontend-data-flow.md).

## Components vs features

The distinction is not always crisp, but:

- `features/<name>/components/` - UI that only makes sense inside that feature's flow.
- `components/<name>/` - shared UI pieces used by multiple features or multiple pages.
- `components/ui/` - the low-level design-system primitives (buttons, badges, cards, tables).
- `components/layout/` - `Layout`, `LeagueShell`, `ProtectedRoute`, the top `NavBar`.

## Pages vs features

Pages are entry points for routes. They compose hooks from one or more features and render components. Most pages are thin - they pull data, pass it to components, and wire up event handlers. `DraftPage` is the largest because it orchestrates several feature hooks plus the WebSocket subscription.

## Environment configuration

From [`frontend/src/config.ts`](../frontend/src/config.ts):

| Env var | Default | Purpose |
| --- | --- | --- |
| `VITE_API_URL` | `https://api.fantasy-puck.ca/api` | Base URL for backend calls |
| `VITE_NHL_SEASON` | `20252026` | Displayed season identifier |
| `VITE_NHL_GAME_TYPE` | `3` (Playoffs) | Drives labels like "2025/2026 Playoffs" |

The WebSocket base URL is derived from `API_URL` at construction time in `WebSocketRealtimeService` ([`lib/realtime.ts:20-26`](../frontend/src/lib/realtime.ts)): `https → wss`, `http → ws`, trailing `/api` stripped.

## Where to start reading

- **"How does the nav bar know which routes to show?"** → `components/layout/LeagueShell.tsx` + `contexts/LeagueContext.tsx`.
- **"How does the draft page update live?"** → `pages/DraftPage.tsx` + `features/draft/hooks/use-draft-session.ts` + `lib/realtime.ts`. See also [`08-draft.md`](./08-draft.md).
- **"How does the Pulse page know when to poll?"** → `features/pulse/hooks/use-pulse.ts`.
- **"How do I add a new endpoint on the frontend?"** → `lib/api-client.ts` (shared fetch wrapper) + a new hook under `features/<name>/hooks/`.
