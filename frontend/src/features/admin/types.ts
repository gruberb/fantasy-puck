// Request/response shapes for every admin endpoint. These mirror the
// Rust DTOs on the backend; when a backend shape changes, update here.

export interface InvalidateCacheResult {
  message: string;
}

export interface ProcessRankingsResult {
  message: string;
}

export interface PrewarmResult {
  message: string;
  /** Jobs queued by the prewarm handler. */
  jobs?: string[];
}

export interface RehydrateResult {
  /** Row counts written per table during the rehydrate pass. */
  tables: Record<string, number>;
  elapsed_ms: number;
}

export interface ClubStatsRefreshResult {
  teams_fetched: number;
  skaters_upserted: number;
  errors: string[];
}

export interface BackfillHistoricalResult {
  games_ingested: number;
  stats_rows: number;
  range: { start: string; end: string };
}

export interface RebackfillCarouselResult {
  season: number;
  games_ingested: number;
}

export interface CalibrationRoundReport {
  round: number;
  brier: number;
  log_loss: number;
  games_scored: number;
}

export interface CalibrationTeamDelta {
  team: string;
  predicted: number;
  actual: number;
  delta: number;
}

export interface CalibrationReport {
  season: number;
  overall_brier: number;
  overall_log_loss: number;
  rounds: CalibrationRoundReport[];
  team_deltas: CalibrationTeamDelta[];
}

export interface SweepReport {
  season: number;
  grid_size: number;
  best: {
    params: Record<string, number>;
    brier: number;
    log_loss: number;
  };
  results: Array<{
    params: Record<string, number>;
    brier: number;
    log_loss: number;
  }>;
}

/** Invalidation scope used by the Cache panel. */
export type CacheScope = "all" | "today" | string;

/** Optional grid overrides for calibrate-sweep. Each string is a
 *  comma-separated list of numbers; empty / undefined leaves the
 *  default grid on the backend. */
export interface SweepParams {
  points_scale?: string;
  shrinkage?: string;
  k_factor?: string;
  home_ice_elo?: string;
  trials?: string;
}
