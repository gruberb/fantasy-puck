import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { APP_CONFIG } from "@/config";
import { getFixedAnalysisDateString, dateStringToLocalDate } from "@/utils/timezone";

export function useHomePageData(leagueId: string | null) {
  // Yesterday's date for rankings
  const analysisDateString = getFixedAnalysisDateString();
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

  // Top skaters query
  const {
    data: topSkatersData,
    isLoading: topSkatersLoading,
    error: topSkatersError,
  } = useQuery({
    queryKey: ["topSkaters", leagueId],
    queryFn: () => api.getTopSkaters(APP_CONFIG.HOME_SKATERS_LIMIT, parseInt(APP_CONFIG.DEFAULT_SEASON), APP_CONFIG.DEFAULT_GAME_TYPE, APP_CONFIG.FORM_GAMES),
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
    topSkatersData,
    topSkatersLoading,
    topSkatersError,
    yesterdayRankings: analysisDateRankings,
    yesterdayRankingsLoading: analysisDateRankingsLoading,
    yesterdayRankingsError: analysisDateRankingsError,
    yesterdayString: analysisDateString,
    sleepersData,
    sleepersLoading,
    sleepersError,
  };
}
