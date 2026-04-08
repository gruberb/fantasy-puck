import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { getFixedAnalysisDateString, dateStringToLocalDate } from "@/utils/timezone";
import { PlayoffFantasyTeamRanking } from "@/types/rankings";
import { TeamStats } from "@/types/teamStats";

export function useRankingsData(leagueId: string | null) {
  const [selectedDate, setSelectedDate] = useState<string>(() => {
    return getFixedAnalysisDateString();
  });

  const enabled = !!leagueId;

  // Get daily rankings data
  const {
    data: dailyRankings,
    isLoading: dailyRankingsLoading,
    error: dailyRankingsError,
  } = useQuery({
    queryKey: ["dailyRankings", leagueId, selectedDate],
    queryFn: () => api.getDailyFantasySummary(leagueId!, selectedDate),
    retry: 1,
    enabled,
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
