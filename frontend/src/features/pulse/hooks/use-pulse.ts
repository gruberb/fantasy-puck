import { useEffect } from "react";
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
    staleTime: 15_000,
  });

  const hasLive = query.data?.hasLiveGames ?? false;

  // Auto-refresh every 30s when there are live games. Mirrors the pattern
  // in features/games/hooks/use-games-data.ts.
  useEffect(() => {
    if (!hasLive) return;
    const id = setInterval(() => {
      void query.refetch();
    }, 30_000);
    return () => clearInterval(id);
  }, [hasLive, query]);

  return {
    pulse: query.data,
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
    hasLive,
  };
}
