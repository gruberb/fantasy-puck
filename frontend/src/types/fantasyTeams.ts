import { SkaterStats } from "./skaters";
import { TeamDiagnosis } from "./team-diagnosis";

export interface FantasyTeam {
  teamId: number;
  teamName: string;
}

export interface FantasyTeamPoints {
  teamId: number;
  teamName: string;
  players: SkaterStats[];
  teamTotals: {
    goals: number;
    assists: number;
    totalPoints: number;
  };
  diagnosis?: TeamDiagnosis;
}

export interface NHLTeamBet {
  nhlTeam: string;
  nhlTeamName: string;
  numPlayers: number;
  teamLogo?: string;
}

export interface NHLTeamBetsResponse {
  teamId: number;
  teamName: string;
  bets: NHLTeamBet[];
}

export interface NHLTeam {
  id: number;
  name: string;
  abbreviation?: string;
  teamLogo?: string;
}
