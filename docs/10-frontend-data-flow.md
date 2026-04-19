# Frontend data flow

What each page fetches, how long it stays fresh, which pages poll, and how the WebSocket fits in. Companion to [`09-frontend-architecture.md`](./09-frontend-architecture.md), which covers the structural side.

## React Query defaults

File: [`frontend/src/lib/react-query.ts`](../frontend/src/lib/react-query.ts).

```ts
const queryConfig: DefaultOptions = {
  queries: {
    refetchOnWindowFocus: false,
    retry: false,
    staleTime: QUERY_INTERVALS.DEFAULT_STALE_MS,
  },
};
```

- **`refetchOnWindowFocus: false`** - switching tabs does not trigger refetches. Most data in this app is slow-moving enough that a tab switch is noise.
- **`retry: false`** - do not hammer a failing backend. Individual hooks can override (most override to `retry: 1`).
- **`staleTime: DEFAULT_STALE_MS`** - 5 minutes. A cached query is considered fresh for five minutes; revisiting a page within that window renders from cache instantly.

## `QUERY_INTERVALS`

File: [`frontend/src/config.ts`](../frontend/src/config.ts). Every cadence the app uses lives here:

| Constant | Value | Where used |
| --- | --- | --- |
| `DEFAULT_STALE_MS` | 5 min | Global React Query default |
| `INSIGHTS_STALE_MS` | 15 min | `useInsights` staleTime |
| `RACE_ODDS_STALE_MS` | 15 min | `useRaceOdds` staleTime |
| `PULSE_STALE_MS` | 60 s | `usePulse` staleTime and live `refetchInterval` |
| `GAMES_LIVE_REFRESH_MS` | 30 s | `useGamesData` refetchInterval when a game is live; `useRankingsData` when viewing today |
| `DRAFT_ELAPSED_TICK_MS` | 1 s | Cosmetic elapsed-time ticker in DraftPage |

The rule of thumb used by the codebase: match the server's update cadence. The live poller writes `nhl_player_game_stats` every 60 s, so Pulse's `PULSE_STALE_MS = 60_000` is aligned. The Games page's `refetchInterval` of 30 s is half that, ensuring the client catches the next server write within one poller tick's worth of lag.

## The `fetchApi` helper

File: [`frontend/src/lib/api-client.ts`](../frontend/src/lib/api-client.ts).

Wraps `fetch` with three conveniences:

1. Prepends `API_URL` and normalises the leading slash.
2. Attaches `Authorization: Bearer <token>` if `authService.getToken()` is set. The token comes from `localStorage["auth_session"]`.
3. Unwraps the `{ success, data }` envelope: throws on `success=false`, returns `data` otherwise. Accepts an optional `fallback` for handlers that should degrade gracefully (for example a top-banner widget that fails quietly if the endpoint is down).

## Per-page data map

Each row: what the page calls, at what staleTime, and whether anything polls. "Uses WS" is only for pages that subscribe to the draft WebSocket.

| Page | Hooks | Endpoints | staleTime | refetchInterval | WS? |
| --- | --- | --- | --- | --- | --- |
| `LoginPage` | `useAuth` (context) | POST `/api/auth/login`, `/api/auth/register` | n/a | - | - |
| `LeaguePickerPage` | `LeagueContext` | GET `/api/leagues`, GET `/api/auth/memberships` | DEFAULT (5 min) | - | - |
| `HomePage` | Uses several dashboards (rankings, top skaters, upcoming games) | Multiple | DEFAULT | - | - |
| `FantasyTeamsPage` | `useFantasyTeams` | GET `/api/fantasy/teams?league_id=...` | DEFAULT | - | - |
| `FantasyTeamDetailPage` | `useFantasyTeam` | GET `/api/fantasy/teams/{id}` | DEFAULT | - | - |
| `RankingsPage` | `useRankingsData` | GET `/api/fantasy/rankings/daily`, GET `/api/fantasy/rankings/playoffs` | DEFAULT | 30 s when viewing today; off otherwise | - |
| `InsightsPage` | `useInsights`, `useRaceOdds` | GET `/api/insights`, GET `/api/race-odds` | 15 min each | - | - |
| `PulsePage` | `usePulse` | GET `/api/pulse?league_id=...` | 60 s | 60 s when `hasLiveGames`; off otherwise | - |
| `GamesPage` | `useGamesData` | GET `/api/nhl/games?date=&league_id=` | DEFAULT | 30 s when any game is LIVE/CRIT; off otherwise | - |
| `SkatersPage` | `useSkatersData` | GET `/api/nhl/skaters/top` | DEFAULT | - | - |
| `DraftPage` | `useDraftSession`, `usePlayerPool`, `useDraftPicks`, `useMakePick`, `useFinalizeDraft`, `useSleeperRound`, `useLeagueMembers` | GET `/api/leagues/{id}/draft`, GET `/api/draft/{id}`, POST `/api/draft/{id}/pick`, ... | DEFAULT | - | **Yes** |
| `AdminPage` | Admin action hooks | GET `/api/admin/*` | DEFAULT | - | - |
| `JoinLeaguePage` | League API | GET `/api/leagues`, POST `/api/leagues/{id}/join` | DEFAULT | - | - |
| `LeagueSettingsPage` | League API | PUT `/api/fantasy/teams/{id}`, DELETE member, DELETE league | DEFAULT | - | - |
| `SettingsPage` | Auth | PUT `/api/auth/profile`, DELETE `/api/auth/account` | n/a | - | - |

### How "poll only when something is live" works

Both `useGamesData` and `usePulse` use React Query's `refetchInterval` as a function of the last response:

```ts
refetchInterval: (query) => {
  const data = query.state.data as PulseResponse | undefined;
  return data?.hasLiveGames ? QUERY_INTERVALS.PULSE_STALE_MS : false;
},
```

When the server reports no live games, `false` tells React Query to stop polling. When the next manual refetch (or user navigation) picks up a live flag, polling resumes. No wasted requests on off-nights.

`useRankingsData` does the same thing but keys off "am I viewing today's date": if yes, poll at 30 s; if no, historical dates are frozen snapshots and a poll is wasted ([`use-rankings-data.ts:25-42`](../frontend/src/features/rankings/hooks/use-rankings-data.ts)).

## Draft WebSocket

File: [`frontend/src/features/draft/hooks/use-draft-session.ts`](../frontend/src/features/draft/hooks/use-draft-session.ts).

One subscription per draft session, established in `useDraftSession`. The effect runs when the session id becomes known, tears down on unmount, and the handlers map WebSocket events into React Query cache updates:

| Event | Handler behavior |
| --- | --- |
| `sessionUpdated` | `setQueryData` with the partial update; then `invalidateQueries` to force a full refetch so derived fields stay correct |
| `pickMade` | Append the pick to the picks cache (dedup by `id`); invalidate the session cache |
| `sleeperUpdated` | Invalidate the `eligibleSleepers` and `sleeperPicks` queries |
| `playerPoolUpdated` | Invalidate the `playerPool` query |

All of this runs in a single `useEffect`, and only one WebSocket connection exists at a time - the DraftPage does not multiplex.

### WebSocket URL derivation

From [`frontend/src/lib/realtime.ts:20-26`](../frontend/src/lib/realtime.ts):

```ts
function deriveWsUrl(apiUrl: string): string {
  return apiUrl
    .replace(/^https:\/\//, "wss://")
    .replace(/^http:\/\//, "ws://")
    .replace(/\/api\/?$/, "");
}
```

`https://api.fantasy-puck.ca/api` becomes `wss://api.fantasy-puck.ca`. The final URL appended with the session id and token is:

```
wss://api.fantasy-puck.ca/ws/draft/{session_id}?token={jwt}
```

### Reconnect

Exponential backoff with a 30-second cap ([`realtime.ts:38-92`](../frontend/src/lib/realtime.ts)):

```
backoff starts at 1000 ms
on onclose: setTimeout(connect, backoff); backoff = min(backoff * 2, 30_000)
on onopen:  backoff resets to 1000 ms
on explicit unsubscribe: `closed = true`, reconnect stops
```

The client does not try to catch up missed messages on reconnect. It relies on React Query's `invalidateQueries` (fired whenever the sesssion is reopened via a hook effect) to fetch the current state from HTTP. The WebSocket is an optimisation, not the source of truth.

## Auth header flow

The session lives in `localStorage["auth_session"]` (see [`features/auth/api/auth-service.ts`](../frontend/src/features/auth/api/auth-service.ts)). Every `fetchApi` call reads `authService.getToken()` and attaches `Authorization: Bearer <token>` when present ([`api-client.ts:9-12`](../frontend/src/lib/api-client.ts)).

The background `validateSession` on app boot calls `GET /api/auth/me` once:

- Response OK → session stays.
- Response 4xx → session is cleared and every subscriber is notified; UI re-renders as logged-out.
- Network error (offline) → session stays so the user can still navigate cached pages.

Cross-tab sync: a `storage` event listener picks up changes to the `auth_session` key made by another tab and re-reads the session into memory. Signing out in one tab signs out everywhere.

## Cache invalidation from the client side

React Query's invalidation primitive is `queryClient.invalidateQueries({ queryKey })`. The app uses it in three places:

1. **Draft WebSocket events** - as described above.
2. **Mutations** - draft pick, finalize, team name update, player add/remove. Each mutation's `onSuccess` invalidates the affected queries so the next render refetches.
3. **Explicit operator actions** - not yet used; admin prewarm and admin cache invalidation are server-side, not client.

## Query-key conventions

Keys are arrays. The first element is a namespace, subsequent elements are the relevant identifiers. Examples:

```
["insights", activeLeagueId]
["race-odds", leagueKey, myTeamId ?? null]
["pulse", activeLeagueId]
["games", selectedDate, activeLeagueId]
["dailyRankings", leagueId, selectedDate]
["playoffRankings", leagueId]
["draft", "session", leagueId]
["draft", "picks", sessionId]
["draft", "playerPool", sessionId]
["draft", "eligibleSleepers", sessionId]
["draft", "sleeperPicks", sessionId]
```

The convention lets a mutation invalidate a whole family with a prefix key: `invalidateQueries({ queryKey: ["draft"] })` would refetch every draft-scoped query, but in practice each hook invalidates only the keys it owns.

## Error handling

`fetchApi` accepts an optional `fallback`. A hook that wants a non-throwing quiet failure (for example a small card that should not crash the page) passes one. Without a fallback, errors propagate to React Query, which surfaces them via `error` on the hook return. Most pages render an `<ErrorState />` with the message and a retry button.

The backend's `{ success: false, error: "message" }` envelope is the source of the error text. See [`03-api.md`](./03-api.md).

## What the frontend does NOT poll

- Leagues list - once per session.
- Memberships - once per session, plus on explicit join/leave.
- Team rosters - stale on DEFAULT (5 min).
- Playoff bracket - stale on 15 min (via `useInsights` + `useRaceOdds`).
- Draft state - pushed via WebSocket; no polling fallback.

The combined effect: on an off-night with no games live, the app is effectively static after first load.
