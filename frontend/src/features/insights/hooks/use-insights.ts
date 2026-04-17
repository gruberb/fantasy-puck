import { useQuery } from "@tanstack/react-query";
import { API_URL } from "@/config";
import { useLeague } from "@/contexts/LeagueContext";

export interface HotPlayerSignal {
  name: string;
  nhlTeam: string;
  position: string;
  formGoals: number;
  formAssists: number;
  formPoints: number;
  formGames: number;
  playoffPoints: number;
  fantasyTeam: string | null;
  imageUrl: string;
  topSpeed: number | null;
  topShotSpeed: number | null;
}

export type SeriesStateCode =
  | "eliminated"
  | "facingElim"
  | "trailing"
  | "tied"
  | "leading"
  | "aboutToAdvance"
  | "advanced";

export interface ContenderSignal {
  teamAbbrev: string;
  seriesTitle: string;
  wins: number;
  opponentAbbrev: string;
  opponentWins: number;
  round: number;
  seriesState: SeriesStateCode;
  seriesLabel: string;
  oddsToAdvance: number;
  gamesRemaining: number;
}

export interface TeamSeriesProjection {
  teamAbbrev: string;
  teamName: string;
  opponentAbbrev: string;
  opponentName: string;
  round: number;
  wins: number;
  opponentWins: number;
  seriesState: SeriesStateCode;
  seriesLabel: string;
  oddsToAdvance: number;
  gamesRemaining: number;
}

export interface InjuryEntry {
  raw: string;
  playerName: string | null;
  status: string | null;
  fantasyTeam: string | null;
}

export interface RosteredPlayerTag {
  fantasyTeamName: string;
  count: number;
}

export interface PlayerLeader {
  name: string;
  position: string;
  value: number;
  headshot: string;
}

export interface GoalieStatsSignal {
  name: string;
  record: string;
  gaa: number;
  savePctg: number;
  shutouts: number;
}

export interface TodaysGameSignal {
  homeTeam: string;
  awayTeam: string;
  homeRecord: string;
  awayRecord: string;
  venue: string;
  startTime: string;
  seriesContext: string | null;
  isElimination: boolean;
  pointsLeaders: [PlayerLeader, PlayerLeader] | null;
  goalsLeaders: [PlayerLeader, PlayerLeader] | null;
  assistsLeaders: [PlayerLeader, PlayerLeader] | null;
  homeGoalie: GoalieStatsSignal | null;
  awayGoalie: GoalieStatsSignal | null;
  homeStreak: string | null;
  awayStreak: string | null;
  homeL10: string | null;
  awayL10: string | null;
  homeLastResult: string | null;
  awayLastResult: string | null;
  rosteredPlayerTags: RosteredPlayerTag[];
}

export interface FantasyRaceSignal {
  teamName: string;
  totalPoints: number;
  rank: number;
  playersActiveToday: number;
  sparkline: number[];
  deltaYesterday: number;
}

export interface SleeperAlertSignal {
  name: string;
  nhlTeam: string;
  fantasyTeam: string | null;
  points: number;
  goals: number;
  assists: number;
}

export interface InsightsNarratives {
  todaysWatch: string;
  gameNarratives: string[];
  hotPlayers: string;
  cupContenders: string;
  fantasyRace: string;
  sleeperWatch: string;
}

export interface InsightsSignals {
  hotPlayers: HotPlayerSignal[];
  coldHands: HotPlayerSignal[];
  cupContenders: ContenderSignal[];
  seriesProjections: TeamSeriesProjection[];
  todaysGames: TodaysGameSignal[];
  fantasyRace: FantasyRaceSignal[];
  sleeperAlerts: SleeperAlertSignal[];
  newsHeadlines: string[];
  injuryReport: InjuryEntry[];
}

export interface InsightsResponse {
  generatedAt: string;
  narratives: InsightsNarratives;
  signals: InsightsSignals;
}

async function fetchInsights(leagueId?: string): Promise<InsightsResponse> {
  let endpoint = `${API_URL}/insights`;
  if (leagueId) {
    endpoint += `?league_id=${leagueId}`;
  }
  const res = await fetch(endpoint);
  const json = await res.json();
  if (!json.success) throw new Error(json.error || "Failed to fetch insights");
  return json.data;
}

export function useInsights() {
  const { activeLeagueId } = useLeague();

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ["insights", activeLeagueId],
    queryFn: () => fetchInsights(activeLeagueId || undefined),
    staleTime: 15 * 60 * 1000, // 15 minutes
    gcTime: 60 * 60 * 1000, // 1 hour
    retry: 1,
  });

  return { insights: data, isLoading, error, refetch };
}
