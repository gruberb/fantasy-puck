export interface PlaotffNHLTeam {
  id: number;
  abbrev: string;
  wins: number;
}

export interface PlayoffSeries {
  seriesLetter: string;
  roundNumber: number;
  seriesLabel: string;
  bottomSeed: PlaotffNHLTeam;
  topSeed: PlaotffNHLTeam;
}

export interface PlayoffRound {
  roundNumber: number;
  roundLabel: string;
  roundAbbrev: string;
  series: PlayoffSeries[];
}

export interface PlayoffsResponse {
  currentRound: number;
  rounds: PlayoffRound[];
  eliminatedTeams: string[];
  teamsInPlayoffs: string[];
  advancedTeams: string[];
}
