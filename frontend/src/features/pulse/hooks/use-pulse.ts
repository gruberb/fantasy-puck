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
    // Live auto-refresh when the server reports any game in flight.
    // Off-night / no-live-games case returns `false`, so polling stops
    // automatically and the page is as quiet as a static read.
    refetchInterval: (query) => {
      const data = query.state.data as PulseResponse | undefined;
      return data?.hasLiveGames ? QUERY_INTERVALS.PULSE_STALE_MS : false;
    },
  });

  return {
    pulse: query.data,
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
  };
}
