import { useQuery } from "@tanstack/react-query";
import { API_URL, QUERY_INTERVALS } from "@/config";
import { useLeague } from "@/contexts/LeagueContext";

export interface HotPlayerSignal {
  nhlId: number;
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
  teamRating: number | null;
  opponentRating: number | null;
  rosteredTags: RosteredPlayerTag[];
}

export interface RosteredPlayerTag {
  fantasyTeamId: number;
  fantasyTeamName: string;
  count: number;
}

export interface PlayerLeader {
  /** NHL player id — optional because the pre-game landing response
   *  occasionally omits it. When present, renderers link the leader's
   *  name to `nhl.com/player/{id}`. */
  playerId?: number;
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

export interface InsightsNarratives {
  todaysWatch: string;
  gameNarratives: string[];
  hotPlayers: string;
  bracket: string;
  /** Daily Faceoff-style recap of the previous hockey-date's games.
   *  Contains one `### Sub-heading` per covered game, followed by a
   *  short paragraph. Empty when no games were played yesterday. */
  lastNight: string;
}

export interface LastNightScorer {
  name: string;
  team: string;
  goals: number;
  assists: number;
  points: number;
}

export interface LastNightGame {
  homeTeam: string;
  awayTeam: string;
  homeScore: number;
  awayScore: number;
  headline: string;
  seriesAfter: string | null;
  topScorers: LastNightScorer[];
}

export interface InsightsSignals {
  hotPlayers: HotPlayerSignal[];
  coldHands: HotPlayerSignal[];
  seriesProjections: TeamSeriesProjection[];
  todaysGames: TodaysGameSignal[];
  newsHeadlines: string[];
  lastNight: LastNightGame[];
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
    staleTime: QUERY_INTERVALS.INSIGHTS_STALE_MS,
    gcTime: 60 * 60 * 1000, // 1 hour
    retry: 1,
  });

  return { insights: data, isLoading, error, refetch };
}
