// Types mirror the Rust DTOs in backend/src/api/dtos/race_odds.rs and the
// domain types re-exported from backend/src/utils/race_sim.rs.

export type RaceOddsMode = "league" | "champion";

export interface TeamOdds {
  teamId: number;
  teamName: string;
  currentPoints: number;
  projectedFinalMean: number;
  projectedFinalMedian: number;
  p10: number;
  p90: number;
  winProb: number;
  top3Prob: number;
  /** Exact MC pairwise probability: P(this team > opponent team). */
  headToHead: Record<string, number>;
}

export interface NhlTeamOdds {
  abbrev: string;
  advanceRound1Prob: number;
  conferenceFinalsProb: number;
  cupFinalsProb: number;
  cupWinProb: number;
  expectedGames: number;
}

export interface PlayerOdds {
  nhlId: number;
  name: string;
  nhlTeam: string;
  position: string;
  currentPoints: number;
  projectedFinalMean: number;
  projectedFinalMedian: number;
  p10: number;
  p90: number;
  imageUrl: string | null;
}

export interface RivalryCard {
  myTeamName: string;
  rivalTeamName: string;
  myWinProb: number;
  rivalWinProb: number;
  myHeadToHeadProb: number;
  myProjectedMean: number;
  rivalProjectedMean: number;
}

export interface RaceOddsResponse {
  generatedAt: string;
  mode: RaceOddsMode;
  trials: number;
  kFactor: number;
  teamOdds: TeamOdds[];
  championLeaderboard: PlayerOdds[];
  nhlTeams: NhlTeamOdds[];
  rivalry: RivalryCard | null;
}
