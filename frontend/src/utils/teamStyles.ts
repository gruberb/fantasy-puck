/**
 * Shared team styling utilities — gradient generation and NHL team colors.
 */

/** Generate a consistent gradient from a string (e.g. fantasy team name). */
export function getTeamGradient(name: string): {
  gradient: string;
  textColor: string;
} {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }

  const hue1 = Math.abs(hash % 360);
  const hue2 = (hue1 + 40) % 360;

  return {
    gradient: `linear-gradient(135deg, hsl(${hue1}, 70%, 60%), hsl(${hue2}, 80%, 45%))`,
    textColor: hue1 > 210 && hue1 < 340 ? "white" : "rgba(0,0,0,0.8)",
  };
}

const NHL_TEAM_COLORS: Record<string, string> = {
  ANA: "#F47A38",
  ARI: "#8C2633",
  BOS: "#FFB81C",
  BUF: "#002654",
  CGY: "#C8102E",
  CAR: "#CC0000",
  CHI: "#CF0A2C",
  COL: "#6F263D",
  CBJ: "#002654",
  DAL: "#006847",
  DET: "#CE1126",
  EDM: "#FF4C00",
  FLA: "#C8102E",
  LAK: "#111111",
  MIN: "#154734",
  MTL: "#AF1E2D",
  NSH: "#FFB81C",
  NJD: "#CE1126",
  NYI: "#00539B",
  NYR: "#0038A8",
  OTT: "#C52032",
  PHI: "#F74902",
  PIT: "#FFB81C",
  SJS: "#006D75",
  SEA: "#99D9D9",
  STL: "#002F87",
  TBL: "#002868",
  TOR: "#00205B",
  UTA: "#71AFE5",
  VAN: "#00205B",
  VGK: "#B4975A",
  WSH: "#C8102E",
  WPG: "#041E42",
};

const DEFAULT_NHL_COLOR = "#041E42";

/** Look up the primary colour for an NHL team abbreviation. */
export function getTeamPrimaryColor(teamAbbrev: string): string {
  // Direct match first
  if (NHL_TEAM_COLORS[teamAbbrev]) return NHL_TEAM_COLORS[teamAbbrev];

  // Partial match fallback
  for (const [key, color] of Object.entries(NHL_TEAM_COLORS)) {
    if (teamAbbrev.includes(key)) return color;
  }

  return DEFAULT_NHL_COLOR;
}
