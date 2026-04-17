// App configuration

// API URL
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

// App settings
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
