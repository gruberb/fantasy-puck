import { useQuery } from "@tanstack/react-query";

import { api } from "@/api/client";
import type { LeagueStatsResponse } from "@/types/leagueStats";

export function useLeagueStats(leagueId: string | null) {
  return useQuery<LeagueStatsResponse>({
    queryKey: ["leagueStats", leagueId],
    queryFn: () => api.getLeagueStats(leagueId!),
    enabled: !!leagueId,
    retry: 1,
  });
}
