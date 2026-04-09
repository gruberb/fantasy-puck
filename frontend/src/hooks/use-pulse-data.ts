import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { useLeague } from "@/contexts/LeagueContext";
import { getFixedAnalysisDateString } from "@/utils/timezone";
import type { Ranking } from "@/types/rankings";
import type { FantasyTeamInAction, PlayerInAction } from "@/types/matchDay";
import type { Game } from "@/types/games";

interface PulseTeam {
  teamId: number;
  teamName: string;
  rank: number;
  totalPoints: number;
  playersActiveTonight: number;
  pointsToday: number;
  players: PlayerInAction[];
}

export interface PulseData {
  myTeam: PulseTeam | null;
  opponents: PulseTeam[];
  todayDate: string;
  hasGamesToday: boolean;
  games: Game[];
}

export function usePulseData() {
  const { activeLeagueId, myMemberships } = useLeague();

  const myMembership = myMemberships.find(
    (m) => m.league_id === activeLeagueId,
  );
  const myTeamId = myMembership?.fantasy_team_id ?? null;
  const todayDate = getFixedAnalysisDateString();

  const rankingsQuery = useQuery({
    queryKey: ["rankings", activeLeagueId],
    queryFn: () => api.getRankings(activeLeagueId!),
    enabled: !!activeLeagueId,
  });

  const gamesQuery = useQuery({
    queryKey: ["pulse-games", todayDate, activeLeagueId],
    queryFn: () => api.getGames(todayDate, activeLeagueId || undefined),
    enabled: !!activeLeagueId,
  });

  const rankings: Ranking[] = rankingsQuery.data ?? [];
  const fantasyTeams: FantasyTeamInAction[] = gamesQuery.data?.fantasyTeams ?? [];
  const games: Game[] = gamesQuery.data?.games ?? [];
  const hasGamesToday = games.length > 0;

  // Merge rankings with today's game data
  const buildTeam = (ranking: Ranking): PulseTeam => {
    const todayData = fantasyTeams.find((ft) => ft.teamId === ranking.teamId);
    const players = todayData?.playersInAction ?? [];
    return {
      teamId: ranking.teamId,
      teamName: ranking.teamName,
      rank: ranking.rank,
      totalPoints: ranking.totalPoints,
      playersActiveTonight: todayData?.totalPlayersToday ?? 0,
      pointsToday: players.reduce((sum, p) => sum + (p.points || 0), 0),
      players,
    };
  };

  const allTeams = rankings.map(buildTeam);
  const myTeam = allTeams.find((t) => t.teamId === myTeamId) ?? null;
  const opponents = allTeams.filter((t) => t.teamId !== myTeamId);

  return {
    pulse: { myTeam, opponents, todayDate, hasGamesToday, games } as PulseData,
    isLoading: rankingsQuery.isLoading || gamesQuery.isLoading,
    error: rankingsQuery.error || gamesQuery.error,
  };
}
