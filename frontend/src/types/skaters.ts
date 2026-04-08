import { FantasyTeam } from "./fantasyTeams";

export interface SkaterWithPoints {
  fantasyTeam: string;
  fantasyTeamId: number;
  playerName?: string;
  position: string;
  nhlId?: number;
  imageUrl?: string;
  goals?: number;
  assists?: number;
  points?: number;
}

export interface Skater {
  id?: number;
  nhlId?: number;
  name?: string;
  playerName?: string;
  position: string;
  points?: number;
  jerseyNumber?: number;
  nhlTeam?: string;
  fantasyTeam?: string;
  imageUrl?: string;
  teamLogo?: string;
  goals?: number;
  assists?: number;
}

export interface SkaterStats {
  name: string;
  nhlTeam: string;
  nhlId: number;
  position: string;
  goals: number;
  assists: number;
  totalPoints: number;
  imageUrl?: string;
  teamLogo?: string;
  nhlTeamUrlSlug?: string;
}

export interface TopSkater {
  id: number;
  firstName: string;
  lastName: string;
  sweaterNumber?: number;
  headshot: string;
  teamAbbrev: string;
  teamName: string;
  teamLogo: string;
  position: string;
  stats: {
    points: number;
    goals: number;
    assists: number;
    plusMinus?: number;
    penaltyMins?: number;
    goalsPp?: number;
    goalsSh?: number;
    faceoffPct?: number;
    toi?: number;
  };
  fantasyTeam?: FantasyTeam;
}

export interface SkaterWithTeam extends SkaterStats {
  teamName?: string;
  teamId?: number;
  teamAbbreviation?: string;
  nhlTeamUrlSlug?: string;
}
