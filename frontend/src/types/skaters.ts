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
  breakdown?: PlayerBreakdown;
}

export type PlayerGrade = "a" | "b" | "c" | "d" | "f" | "notEnoughData";

export type PlayerBucket =
  | "tooEarly"
  | "keepFaith"
  | "onPace"
  | "outperforming"
  | "fineButFragile"
  | "needMiracle"
  | "problemAsset"
  | "teamEliminated";

export type SeriesStateCode =
  | "eliminated"
  | "facingElim"
  | "trailing"
  | "tied"
  | "leading"
  | "aboutToAdvance"
  | "advanced";

export interface GradeReport {
  grade: PlayerGrade;
  zScore: number;
  expectedPoints: number;
  actualPoints: number;
  gamesPlayed: number;
}

export interface RemainingImpact {
  expectedRemainingGames: number;
  expectedRemainingPoints: number;
  nhlTeamEliminated: boolean;
}

export interface PlayerRecentGameCell {
  gameDate: string;
  opponent: string;
  toiSeconds?: number | null;
  goals: number;
  assists: number;
  points: number;
}

export interface PlayerBreakdown {
  gamesPlayed: number;
  sog: number;
  pim: number;
  plusMinus: number;
  hits: number;
  toiSecondsPerGame: number;
  projectedPpg: number;
  activeProb: number;
  toiMultiplier: number;
  grade: GradeReport;
  remainingImpact: RemainingImpact;
  seriesState: SeriesStateCode;
  bucket: PlayerBucket;
  recentGames: PlayerRecentGameCell[];
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
