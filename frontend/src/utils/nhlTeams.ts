export interface NHLTeamInfo {
  fullName: string; // Full official team name
  shortName: string; // Common/short name
  urlSlug: string; // URL path used on nhl.com
  abbreviation: string; // Official abbreviation
}

/** Build a link to an NHL player's public profile page. */
export function nhlPlayerProfileUrl(nhlId: number | string): string {
  return `https://www.nhl.com/player/${nhlId}`;
}

// Map from team abbreviation to team information
export const NHL_TEAMS_BY_ABBREV: Record<string, NHLTeamInfo> = {
  ANA: {
    fullName: "Anaheim Ducks",
    shortName: "Ducks",
    urlSlug: "ducks",
    abbreviation: "ANA",
  },
  ARI: {
    fullName: "Arizona Coyotes",
    shortName: "Coyotes",
    urlSlug: "coyotes",
    abbreviation: "ARI",
  },
  BOS: {
    fullName: "Boston Bruins",
    shortName: "Bruins",
    urlSlug: "bruins",
    abbreviation: "BOS",
  },
  BUF: {
    fullName: "Buffalo Sabres",
    shortName: "Sabres",
    urlSlug: "sabres",
    abbreviation: "BUF",
  },
  CGY: {
    fullName: "Calgary Flames",
    shortName: "Flames",
    urlSlug: "flames",
    abbreviation: "CGY",
  },
  CAR: {
    fullName: "Carolina Hurricanes",
    shortName: "Hurricanes",
    urlSlug: "hurricanes",
    abbreviation: "CAR",
  },
  CHI: {
    fullName: "Chicago Blackhawks",
    shortName: "Blackhawks",
    urlSlug: "blackhawks",
    abbreviation: "CHI",
  },
  COL: {
    fullName: "Colorado Avalanche",
    shortName: "Avalanche",
    urlSlug: "avalanche",
    abbreviation: "COL",
  },
  CBJ: {
    fullName: "Columbus Blue Jackets",
    shortName: "Blue Jackets",
    urlSlug: "bluejackets",
    abbreviation: "CBJ",
  },
  DAL: {
    fullName: "Dallas Stars",
    shortName: "Stars",
    urlSlug: "stars",
    abbreviation: "DAL",
  },
  DET: {
    fullName: "Detroit Red Wings",
    shortName: "Red Wings",
    urlSlug: "redwings",
    abbreviation: "DET",
  },
  EDM: {
    fullName: "Edmonton Oilers",
    shortName: "Oilers",
    urlSlug: "oilers",
    abbreviation: "EDM",
  },
  FLA: {
    fullName: "Florida Panthers",
    shortName: "Panthers",
    urlSlug: "panthers",
    abbreviation: "FLA",
  },
  LAK: {
    fullName: "Los Angeles Kings",
    shortName: "Kings",
    urlSlug: "kings",
    abbreviation: "LAK",
  },
  MIN: {
    fullName: "Minnesota Wild",
    shortName: "Wild",
    urlSlug: "wild",
    abbreviation: "MIN",
  },
  MTL: {
    fullName: "Montreal Canadiens",
    shortName: "Canadiens",
    urlSlug: "canadiens",
    abbreviation: "MTL",
  },
  NSH: {
    fullName: "Nashville Predators",
    shortName: "Predators",
    urlSlug: "predators",
    abbreviation: "NSH",
  },
  NJD: {
    fullName: "New Jersey Devils",
    shortName: "Devils",
    urlSlug: "devils",
    abbreviation: "NJD",
  },
  NYI: {
    fullName: "New York Islanders",
    shortName: "Islanders",
    urlSlug: "islanders",
    abbreviation: "NYI",
  },
  NYR: {
    fullName: "New York Rangers",
    shortName: "Rangers",
    urlSlug: "rangers",
    abbreviation: "NYR",
  },
  OTT: {
    fullName: "Ottawa Senators",
    shortName: "Senators",
    urlSlug: "senators",
    abbreviation: "OTT",
  },
  PHI: {
    fullName: "Philadelphia Flyers",
    shortName: "Flyers",
    urlSlug: "flyers",
    abbreviation: "PHI",
  },
  PIT: {
    fullName: "Pittsburgh Penguins",
    shortName: "Penguins",
    urlSlug: "penguins",
    abbreviation: "PIT",
  },
  SEA: {
    fullName: "Seattle Kraken",
    shortName: "Kraken",
    urlSlug: "kraken",
    abbreviation: "SEA",
  },
  SJS: {
    fullName: "San Jose Sharks",
    shortName: "Sharks",
    urlSlug: "sharks",
    abbreviation: "SJS",
  },
  STL: {
    fullName: "St. Louis Blues",
    shortName: "Blues",
    urlSlug: "blues",
    abbreviation: "STL",
  },
  TBL: {
    fullName: "Tampa Bay Lightning",
    shortName: "Lightning",
    urlSlug: "lightning",
    abbreviation: "TBL",
  },
  TOR: {
    fullName: "Toronto Maple Leafs",
    shortName: "Maple Leafs",
    urlSlug: "mapleleafs",
    abbreviation: "TOR",
  },
  VAN: {
    fullName: "Vancouver Canucks",
    shortName: "Canucks",
    urlSlug: "canucks",
    abbreviation: "VAN",
  },
  VGK: {
    fullName: "Vegas Golden Knights",
    shortName: "Golden Knights",
    urlSlug: "goldenknights",
    abbreviation: "VGK",
  },
  WSH: {
    fullName: "Washington Capitals",
    shortName: "Capitals",
    urlSlug: "capitals",
    abbreviation: "WSH",
  },
  WPG: {
    fullName: "Winnipeg Jets",
    shortName: "Jets",
    urlSlug: "jets",
    abbreviation: "WPG",
  },
  UTA: {
    fullName: "Utah Hockey Club",
    shortName: "Utah HC",
    urlSlug: "utahhc",
    abbreviation: "UTA",
  },
};

// Map from full team name to team information
export const NHL_TEAMS_BY_FULL_NAME: Record<string, NHLTeamInfo> =
  Object.values(NHL_TEAMS_BY_ABBREV).reduce(
    (acc, team) => {
      acc[team.fullName] = team;
      return acc;
    },
    {} as Record<string, NHLTeamInfo>,
  );

// Map from short name to team information
export const NHL_TEAMS_BY_SHORT_NAME: Record<string, NHLTeamInfo> =
  Object.values(NHL_TEAMS_BY_ABBREV).reduce(
    (acc, team) => {
      acc[team.shortName] = team;
      return acc;
    },
    {} as Record<string, NHLTeamInfo>,
  );

/**
 * Gets the URL slug for an NHL team given its abbreviation, full name, or short name
 * @param teamIdentifier The team abbreviation, full name, or short name
 * @returns The URL slug for the team, or a sanitized version of the input if not found
 */
export function getNHLTeamUrlSlug(teamIdentifier: string): string {
  if (!teamIdentifier) {
    return "";
  }

  // Check if it's an abbreviation
  if (NHL_TEAMS_BY_ABBREV[teamIdentifier]) {
    return NHL_TEAMS_BY_ABBREV[teamIdentifier].urlSlug;
  }

  // Check if it's a full name
  if (NHL_TEAMS_BY_FULL_NAME[teamIdentifier]) {
    return NHL_TEAMS_BY_FULL_NAME[teamIdentifier].urlSlug;
  }

  // Check if it's a short name
  if (NHL_TEAMS_BY_SHORT_NAME[teamIdentifier]) {
    return NHL_TEAMS_BY_SHORT_NAME[teamIdentifier].urlSlug;
  }

  // If we don't have a mapping, sanitize the input as a fallback
  return teamIdentifier.toLowerCase().replace(/\s+/g, "");
}

/**
 * Gets the common/short name for an NHL team given its abbreviation or full name
 * @param teamIdentifier The team abbreviation or full name
 * @returns The short name for the team, or the input if not found
 */
/**
 * Gets the logo URL for an NHL team given its abbreviation
 */
export function getNHLTeamLogoUrl(abbrev: string): string {
  return `https://assets.nhle.com/logos/nhl/svg/${abbrev}_light.svg`;
}

/**
 * Gets the full name for an NHL team given its abbreviation
 * @param abbrev The team abbreviation (e.g. "TBL")
 * @returns The full name (e.g. "Tampa Bay Lightning"), or the abbreviation if not found
 */
export function getNHLTeamFullName(abbrev: string): string {
  return NHL_TEAMS_BY_ABBREV[abbrev]?.fullName ?? abbrev;
}

export function getNHLTeamShortName(teamIdentifier: string): string {
  if (!teamIdentifier) {
    return "";
  }

  // Check if it's an abbreviation
  if (NHL_TEAMS_BY_ABBREV[teamIdentifier]) {
    return NHL_TEAMS_BY_ABBREV[teamIdentifier].shortName;
  }

  // Check if it's a full name
  if (NHL_TEAMS_BY_FULL_NAME[teamIdentifier]) {
    return NHL_TEAMS_BY_FULL_NAME[teamIdentifier].shortName;
  }

  return teamIdentifier;
}

