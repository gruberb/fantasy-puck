/**
 * Format a season string like "20252026" into "2025/2026".
 * Falls back to the raw string if it doesn't match the expected pattern.
 */
export function formatSeason(season: string): string {
  if (season.length === 8) {
    return `${season.slice(0, 4)}/${season.slice(4)}`;
  }
  return season;
}

/** Seconds of ice time as "m:ss". */
export function formatToi(seconds: number): string {
  const total = Math.round(seconds);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
