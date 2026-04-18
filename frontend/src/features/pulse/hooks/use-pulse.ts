import { useQuery } from "@tanstack/react-query";
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
    staleTime: 60_000,
  });

  return {
    pulse: query.data,
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
  };
}
