import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { useMemo, useCallback } from "react";

export function usePlayoffsData() {
  const {
    data: playoffsData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["playoffs"],
    queryFn: () => api.getPlayoffs(),
  });

  // Use backend-computed sets (converted to Sets for O(1) lookups)
  const eliminatedTeams = useMemo(
    () => new Set(playoffsData?.eliminatedTeams ?? []),
    [playoffsData],
  );

  const teamsInPlayoffs = useMemo(
    () => new Set(playoffsData?.teamsInPlayoffs ?? []),
    [playoffsData],
  );

  const advancedTeams = useMemo(
    () => new Set(playoffsData?.advancedTeams ?? []),
    [playoffsData],
  );

  const isTeamInPlayoffs = useCallback(
    (teamAbbrev: string) => teamsInPlayoffs.has(teamAbbrev),
    [teamsInPlayoffs],
  );

  const playoffTeamsArray = useMemo(
    () => Array.from(teamsInPlayoffs),
    [teamsInPlayoffs],
  );

  // Teams in current round: active teams from the current round's series
  const teamsInCurrentRound = useMemo(() => {
    if (!playoffsData?.rounds) return new Set<string>();

    const teamSet = new Set<string>();
    const currentRound = playoffsData.rounds.find(
      (r) => r.roundNumber === playoffsData.currentRound,
    );

    if (currentRound) {
      currentRound.series.forEach((series) => {
        if (series.topSeed.abbrev && series.topSeed.abbrev !== "TBD" && !eliminatedTeams.has(series.topSeed.abbrev)) {
          teamSet.add(series.topSeed.abbrev);
        }
        if (series.bottomSeed.abbrev && series.bottomSeed.abbrev !== "TBD" && !eliminatedTeams.has(series.bottomSeed.abbrev)) {
          teamSet.add(series.bottomSeed.abbrev);
        }
      });
    }

    return teamSet;
  }, [playoffsData, eliminatedTeams]);

  return {
    playoffsData,
    isLoading,
    error,
    teamsInPlayoffs,
    isTeamInPlayoffs,
    playoffTeamsArray,
    advancedTeams,
    teamsInCurrentRound,
    eliminatedTeams,
    hasTeamAdvanced: useCallback(
      (teamAbbrev: string) => advancedTeams.has(teamAbbrev),
      [advancedTeams],
    ),
    isTeamEliminated: useCallback(
      (teamAbbrev: string) => eliminatedTeams.has(teamAbbrev),
      [eliminatedTeams],
    ),
    isTeamInCurrentRound: useCallback(
      (teamAbbrev: string) => teamsInCurrentRound.has(teamAbbrev),
      [teamsInCurrentRound],
    ),
  };
}
