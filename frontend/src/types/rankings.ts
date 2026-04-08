export interface Ranking {
  rank: number;
  teamId: number;
  teamName: string;
  goals: number;
  assists: number;
  totalPoints: number;
}

export interface SkaterHighlight {
  playerName: string;
  points: number;
  nhlTeam: string;
  imageUrl?: string;
  nhlId?: number;
}

export interface DailyRankingsResponse {
  date: string;
  rankings: RankingItem[];
}

export interface PlayoffFantasyTeamRanking {
  teamId: number;
  teamName: string;
  teamsInPlayoffs: number;
  totalTeams: number;
  playersInPlayoffs: number;
  totalPlayers: number;
  topTenPlayersCount: number;
  playoffScore: number;
  rank?: number;
  goals?: number;
  assists?: number;
  totalPoints?: number;
}

export interface RankingItem {
  rank: number;
  teamId: number;
  teamName: string;
  dailyPoints: number;
  dailyGoals: number;
  dailyAssists: number;
  playerHighlights: SkaterHighlight[];
}
