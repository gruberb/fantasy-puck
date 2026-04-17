import { useQuery } from "@tanstack/react-query";

import { fetchApi } from "@/lib/api-client";
import { useLeague } from "@/contexts/LeagueContext";

import type { RaceOddsResponse } from "../types";

interface UseRaceOddsOptions {
  /**
   * Fantasy team id of the current user inside the active league. When
   * provided in league mode, the response's `rivalry` card is populated
   * against this team's closest rival by projected mean.
   */
  myTeamId?: number | null;
}

/**
 * React Query hook returning Monte Carlo race odds for the active league,
 * or the global Fantasy Champion leaderboard when no league is active.
 *
 * Query key mirrors the backend cache-key shape: `(leagueId, myTeamId)`.
 * The backend caches the heavy sim once per day; changing `myTeamId` only
 * reshapes the rivalry card which is cheap to recompute.
 */
export function useRaceOdds({ myTeamId }: UseRaceOddsOptions = {}) {
  const { activeLeagueId } = useLeague();
  const leagueKey = activeLeagueId ?? "global";

  return useQuery({
    queryKey: ["race-odds", leagueKey, myTeamId ?? null],
    queryFn: () => fetchApi<RaceOddsResponse>(buildEndpoint(activeLeagueId, myTeamId)),
    staleTime: 15 * 60 * 1000,
    gcTime: 60 * 60 * 1000,
    retry: 1,
  });
}

function buildEndpoint(
  leagueId: string | null,
  myTeamId: number | null | undefined,
): string {
  const params = new URLSearchParams();
  if (leagueId) params.set("league_id", leagueId);
  if (myTeamId != null) params.set("my_team_id", String(myTeamId));
  const qs = params.toString();
  return qs ? `race-odds?${qs}` : "race-odds";
}
