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
