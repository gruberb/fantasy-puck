import { useQuery } from "@tanstack/react-query";
import { QUERY_INTERVALS } from "@/config";
import { fetchApi } from "@/lib/api-client";
import { useLeague } from "@/contexts/LeagueContext";
import type { PulseResponse } from "../types";

export function usePulse() {
  const { activeLeagueId } = useLeague();

  const query = useQuery({
    queryKey: ["pulse", activeLeagueId],
    queryFn: () =>
      fetchApi<PulseResponse>(`pulse?league_id=${activeLeagueId}`),
    enabled: !!activeLeagueId,
    staleTime: QUERY_INTERVALS.PULSE_STALE_MS,
  });

  return {
    pulse: query.data,
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
  };
}
