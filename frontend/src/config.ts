// Frontend configuration.
//
// Two concerns live here:
//
//   1. `APP_CONFIG` — product metadata (app name, current season, game
//      type, display labels). Derived from Vite env vars at build time,
//      with sensible defaults for local dev.
//
//   2. `QUERY_INTERVALS` — every poll interval, React Query staleTime,
//      and client-side timer the app uses. Centralising these lets a
//      maintainer change refresh cadence in one place without grepping
//      through hooks.
//
// Anything added to these objects should get a comment explaining why
// that cadence — the scanning pattern is "find the right key, read the
// comment, decide whether to change".

// API URL. The backend runs at `api.fantasy-puck.ca` in production,
// and at `http://localhost:3000` when running `make run` locally
// (Vite's dev server proxies to VITE_API_URL).
export const API_URL =
  import.meta.env.VITE_API_URL || "https://api.fantasy-puck.ca/api";

const GAME_TYPE_LABELS: Record<number, string> = {
  1: "Preseason",
  2: "Regular Season",
  3: "Playoffs",
};

const DEFAULT_SEASON = import.meta.env.VITE_NHL_SEASON || "20252026";
const DEFAULT_GAME_TYPE = Number(import.meta.env.VITE_NHL_GAME_TYPE) || 3;
const GAME_TYPE_LABEL = GAME_TYPE_LABELS[DEFAULT_GAME_TYPE] ?? "Playoffs";

function formatSeason(s: string): string {
  return s.length === 8 ? `${s.slice(0, 4)}/${s.slice(4)}` : s;
}

// App settings derived from env vars at build time.
export const APP_CONFIG = {
  APP_NAME: "Fantasy NHL Dashboard",
  DEFAULT_SEASON,
  DEFAULT_GAME_TYPE,
  GAME_TYPE_LABEL,
  // e.g. "2025/2026 Playoffs"
  SEASON_LABEL: `${formatSeason(DEFAULT_SEASON)} ${GAME_TYPE_LABEL}`,
  // e.g. "NHL 2026" (last four digits of the season)
  BRAND_LABEL: `NHL ${DEFAULT_SEASON.slice(4)}`,
  FORM_GAMES: 5,
  SKATERS_LIMIT: 1000,
  HOME_SKATERS_LIMIT: 10,
};

// React Query staleTime / refetch intervals, in milliseconds.
//
// `staleTime` controls how long React Query considers a cache entry
// fresh. Within that window, navigating back to a page does not
// refetch; after it, the next render triggers a background refetch.
//
// `REFRESH` values drive explicit `setInterval`-style polling (today
// only the Games page, gated behind an opt-in checkbox).
//
// Units: milliseconds. Values are given as `N * 60 * 1000` so the
// minute count is legible at the call site.
export const QUERY_INTERVALS = {
  // Global default for React Query (see `frontend/src/lib/react-query.ts`).
  // Five minutes balances UI liveness against backend load for the
  // majority of pages, which are not time-sensitive.
  DEFAULT_STALE_MS: 5 * 60 * 1000,

  // Insights is a once-per-day narrative. Keep it stale for 15 min so
  // repeated navigations don't regenerate the Claude call.
  INSIGHTS_STALE_MS: 15 * 60 * 1000,

  // Race-Odds is a heavy Monte Carlo payload. Same cadence as Insights.
  RACE_ODDS_STALE_MS: 15 * 60 * 1000,

  // Pulse reflects the caller's personal team. We want it to pick up
  // live in-game points quickly; 60 s matches the server-side live
  // poller cadence once the NHL mirror lands.
  PULSE_STALE_MS: 60 * 1000,

  // Games-page auto-refresh. Only fires when the user opts in AND the
  // current date has at least one live game. Matches the server's
  // live-boxscore TTL so we never hit the cache mid-refresh and get
  // the same response twice.
  GAMES_LIVE_REFRESH_MS: 30 * 1000,

  // Draft-room elapsed-time ticker. Cosmetic; updates the "N seconds
  // since current pick" display.
  DRAFT_ELAPSED_TICK_MS: 1000,
} as const;
