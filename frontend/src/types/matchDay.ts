import { Game } from "./games";

export interface PlayerInAction {
  fantasyTeam: string;
  fantasyTeamId: number;
  playerName: string;
  position: string;
  nhlId: number;
  imageUrl: string;
  teamLogo: string;
  nhlTeam: string;
  goals: number;
  assists: number;
  points: number;
  playoffGoals: number;
  playoffAssists: number;
  playoffPoints: number;
  playoffGames: number;
  form?: {
    games: number;
    goals: number;
    assists: number;
    points: number;
  };
  timeOnIce: string | null;
}

export interface FantasyTeamInAction {
  teamId: number;
  teamName: string;
  playersInAction: PlayerInAction[];
  totalPlayersToday: number;
}

export interface TeamPlayerCount {
  nhlTeam: string;
  playerCount: number;
}

export interface MatchDaySummary {
  totalGames: number;
  totalTeamsPlaying: number;
  teamPlayersCount: TeamPlayerCount[];
}

export interface MatchDayResponse {
  date: string;
  games: Game[];
  fantasyTeams: FantasyTeamInAction[];
  summary: MatchDaySummary;
}
