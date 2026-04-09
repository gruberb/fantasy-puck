// App configuration

// API URL
export const API_URL =
  import.meta.env.VITE_API_URL || "https://api.fantasy-puck.ca/api";

// App settings
export const APP_CONFIG = {
  APP_NAME: "Fantasy NHL Dashboard",
  DEFAULT_SEASON: import.meta.env.VITE_NHL_SEASON || "20252026",
  DEFAULT_GAME_TYPE: Number(import.meta.env.VITE_NHL_GAME_TYPE) || 3, // 3 = Playoffs
  FORM_GAMES: 5,
  SKATERS_LIMIT: 1000,
  HOME_SKATERS_LIMIT: 10,
};
