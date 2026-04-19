import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { QUERY_INTERVALS } from "@/config";
import {
  getMostRecentRankingsDate,
  dateStringToLocalDate,
  isSameLocalDay,
} from "@/utils/timezone";
import { PlayoffFantasyTeamRanking } from "@/types/rankings";
import { TeamStats } from "@/types/teamStats";

export function useRankingsData(leagueId: string | null) {
  const [selectedDate, setSelectedDate] = useState<string>(() => {
    return getMostRecentRankingsDate();
  });

  const enabled = !!leagueId;

  // Auto-refresh the daily rankings on whichever date the user is
  // currently viewing if it's today — live-poller-driven scoring
  // writes hit `v_daily_fantasy_totals` within 60 s, so matching the
  // cadence here picks up live in-game points without a manual
  // refresh. Historical dates are pure DB snapshots; no need to poll.
  const isViewingToday = isSameLocalDay(
    dateStringToLocalDate(selectedDate),
    new Date(),
  );

  const {
    data: dailyRankings,
    isLoading: dailyRankingsLoading,
    error: dailyRankingsError,
  } = useQuery({
    queryKey: ["dailyRankings", leagueId, selectedDate],
    queryFn: () => api.getDailyFantasySummary(leagueId!, selectedDate),
    retry: 1,
    enabled,
    refetchInterval: isViewingToday
      ? QUERY_INTERVALS.GAMES_LIVE_REFRESH_MS
      : false,
  });

  // Playoff rankings — computed server-side (eliminates N+1 queries)
  const {
    data: playoffRankings = [] as PlayoffFantasyTeamRanking[],
    isLoading: playoffRankingsLoading,
  } = useQuery({
    queryKey: ["playoffRankings", leagueId],
    queryFn: () => api.getPlayoffRankings(leagueId!),
    enabled,
  });

  // Fetch team stats
  const {
    data: teamStats,
    isLoading: teamStatsLoading,
    error: teamStatsError,
  } = useQuery<TeamStats[]>({
    queryKey: ["teamStats", leagueId],
    queryFn: () => api.getTeamStats(leagueId!),
    enabled,
  });

  const displayDate = dateStringToLocalDate(selectedDate);

  return {
    selectedDate,
    setSelectedDate,
    displayDate,
    dailyRankings,
    dailyRankingsLoading,
    dailyRankingsError,
    playoffRankings,
    playoffRankingsLoading,
    teamStats,
    teamStatsLoading,
    teamStatsError,
  };
}
