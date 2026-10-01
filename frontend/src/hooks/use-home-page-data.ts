import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { clampToSeasonWindow } from "@/config";
import { getHockeyDateYesterday, dateStringToLocalDate } from "@/utils/timezone";

export function useHomePageData(leagueId: string | null) {
  // Yesterday's date for rankings — rankings table is populated
  // post-completion, so today returns empty during live slates. Clamped to
  // the season window so once the season is over this pins to the last game
  // day instead of chasing empty future dates.
  const analysisDateString = clampToSeasonWindow(getHockeyDateYesterday());
  const analysisDate = dateStringToLocalDate(analysisDateString);

  const enabled = !!leagueId;

  // Rankings query
  const {
    data: rankings,
    isLoading: rankingsLoading,
    error: rankingsError,
  } = useQuery({
    queryKey: ["rankings", leagueId],
    queryFn: () => api.getRankings(leagueId!),
    enabled,
  });

  // Sleepers query
  const {
    data: sleepersData,
    isLoading: sleepersLoading,
    error: sleepersError,
  } = useQuery({
    queryKey: ["sleepers", leagueId],
    queryFn: () => api.getSleepers(leagueId!),
    enabled,
  });

  // Analysis date rankings query
  const {
    data: analysisDateRankings,
    isLoading: analysisDateRankingsLoading,
    error: analysisDateRankingsError,
  } = useQuery({
    queryKey: ["dailyRankings", leagueId, analysisDateString],
    queryFn: () => api.getDailyFantasySummary(leagueId!, analysisDateString),
    retry: 1,
    enabled,
  });

  return {
    yesterdayDate: analysisDate,
    rankings,
    rankingsLoading,
    rankingsError,
    yesterdayRankings: analysisDateRankings,
    yesterdayRankingsLoading: analysisDateRankingsLoading,
    yesterdayRankingsError: analysisDateRankingsError,
    yesterdayString: analysisDateString,
    sleepersData,
    sleepersLoading,
    sleepersError,
  };
}
