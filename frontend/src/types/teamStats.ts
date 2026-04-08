export interface PlayerStats {
  nhlId: number;
  name: string;
  points: number;
  nhlTeam: string;
  position: string;
  imageUrl?: string;
  teamLogo?: string;
}

export interface NHLTeamStats {
  nhlTeam: string;
  points: number;
  teamLogo?: string;
  teamName: string;
}

export interface TeamStats {
  teamId: number;
  teamName: string;
  totalPoints: number;
  dailyWins: number;
  dailyTopThree: number;
  winDates: string[];
  topThreeDates: string[];
  topPlayers: PlayerStats[];
  topNhlTeams: NHLTeamStats[];
}
