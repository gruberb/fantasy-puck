import { SkaterWithPoints } from "./skaters";

export interface FantasyTeamCount {
  teamId: number;
  teamName: string;
  teamLogo?: string;
  playerCount: number;
  players: SkaterWithPoints[];
  totalPoints: number;
}
