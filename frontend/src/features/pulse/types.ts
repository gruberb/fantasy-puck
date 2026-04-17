export type SeriesStateCode =
  | "eliminated"
  | "facingElim"
  | "trailing"
  | "tied"
  | "leading"
  | "aboutToAdvance"
  | "advanced";

export interface PlayerForecastCell {
  nhlId: number;
  playerName: string;
  position: string;
  nhlTeam: string;
  nhlTeamName: string;
  opponentAbbrev: string | null;
  opponentName: string | null;
  seriesState: SeriesStateCode;
  seriesLabel: string;
  wins: number;
  opponentWins: number;
  oddsToAdvance: number;
  gamesRemaining: number;
  headshotUrl: string;
}

export interface FantasyTeamForecast {
  teamId: number;
  teamName: string;
  totalPlayers: number;
  playersEliminated: number;
  playersFacingElimination: number;
  playersTrailing: number;
  /** Players whose series is currently tied. Separate from `playersTrailing`. */
  playersTied: number;
  playersLeading: number;
  playersAdvanced: number;
  cells: PlayerForecastCell[];
}

export interface MyTeamStatus {
  teamId: number;
  teamName: string;
  rank: number;
  totalPoints: number;
  pointsToday: number;
  playersActiveToday: number;
  totalRosterSize: number;
}

export interface MyPlayerInGame {
  nhlId: number;
  name: string;
  position: string;
  nhlTeam: string;
  headshotUrl: string;
  goals: number;
  assists: number;
  points: number;
}

export interface MyGameTonight {
  gameId: number;
  homeTeam: string;
  homeTeamName: string;
  homeTeamLogo: string;
  awayTeam: string;
  awayTeamName: string;
  awayTeamLogo: string;
  startTimeUtc: string;
  venue: string;
  gameState: string;
  homeScore: number | null;
  awayScore: number | null;
  period: string | null;
  seriesContext: string | null;
  isElimination: boolean;
  myPlayers: MyPlayerInGame[];
}

export interface LeagueBoardEntry {
  rank: number;
  teamId: number;
  teamName: string;
  totalPoints: number;
  pointsToday: number;
  playersActiveToday: number;
  sparkline: number[];
  isMyTeam: boolean;
}

export interface PulseResponse {
  generatedAt: string;
  myTeam: MyTeamStatus | null;
  seriesForecast: FantasyTeamForecast[];
  myGamesTonight: MyGameTonight[];
  leagueBoard: LeagueBoardEntry[];
  hasGamesToday: boolean;
  hasLiveGames: boolean;
  /** Personal narrative from Claude Sonnet 4.6, or null if unavailable. */
  narrative: string | null;
}
