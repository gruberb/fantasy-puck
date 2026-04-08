// App configuration

// API URL
export const API_URL =
  import.meta.env.VITE_API_URL || "https://api.fantasy-puck.ca/api";

// App settings
export const APP_CONFIG = {
  APP_NAME: "Fantasy NHL Dashboard",
  DEFAULT_SEASON: "20252026",
  DEFAULT_GAME_TYPE: 3, // 3 = Playoffs
  FORM_GAMES: 5,
  SKATERS_LIMIT: 1000,
  HOME_SKATERS_LIMIT: 10,
  STALE_TIME: 1000 * 60 * 5, // 5 minutes
  CACHE_TIME: 1000 * 60 * 30, // 30 minutes
};

// Default query options for React Query
export const DEFAULT_QUERY_OPTIONS = {
  staleTime: APP_CONFIG.STALE_TIME,
  cacheTime: APP_CONFIG.CACHE_TIME,
  retry: false as const,
  refetchOnWindowFocus: false,
};
