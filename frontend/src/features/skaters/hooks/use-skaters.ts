import { useState, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import { APP_CONFIG } from "@/config";
import { usePlayoffsData } from "@/features/rankings/hooks/use-playoffs-data";
import { useLeague } from "@/contexts/LeagueContext";

export function useSkaters() {
  const { isTeamInPlayoffs } = usePlayoffsData();
  const { activeLeagueId } = useLeague();

  // Filter states
  const [searchTerm, setSearchTerm] = useState("");
  const [positionFilter, setPositionFilter] = useState("all");
  const [inPlayoffsFilter, setInPlayoffsFilter] = useState("all");

  // Fetch top skaters data
  const {
    data: skaters,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ["topSkaters", APP_CONFIG.SKATERS_LIMIT, activeLeagueId],
    queryFn: () => api.getTopSkaters(
      APP_CONFIG.SKATERS_LIMIT,
      parseInt(APP_CONFIG.DEFAULT_SEASON),
      APP_CONFIG.DEFAULT_GAME_TYPE,
      APP_CONFIG.FORM_GAMES,
      activeLeagueId,
    ),
  });

  // Make sure we have an array of skaters
  const allSkaters = useMemo(() => {
    return skaters || [];
  }, [skaters]);

  // Get unique positions from skaters - with null check
  const positions = useMemo(() => {
    if (!allSkaters || !Array.isArray(allSkaters) || allSkaters.length === 0) {
      return [];
    }

    return [...new Set(allSkaters.map(skater => skater.position))].sort();
  }, [allSkaters]);

  // Count skaters by position for statistics display
  const positionCounts = useMemo(() => {
    if (!allSkaters || !Array.isArray(allSkaters) || allSkaters.length === 0) {
      return [];
    }

    const counts: Record<string, number> = {};
    allSkaters.forEach((skater) => {
      counts[skater.position] = (counts[skater.position] || 0) + 1;
    });
    return Object.entries(counts).map(([position, count]) => ({
      position,
      count,
    }));
  }, [allSkaters]);

  // Filter skaters based on search term and filters
  const filteredSkaters = useMemo(() => {
    if (!allSkaters || !Array.isArray(allSkaters) || allSkaters.length === 0) {
      return [];
    }

    return allSkaters.filter(skater => {
      const matchesSearch = searchTerm === "" ||
        `${skater.firstName} ${skater.lastName}`.toLowerCase().includes(searchTerm.toLowerCase()) ||
        skater.teamAbbrev.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (skater.fantasyTeam?.teamName || "").toLowerCase().includes(searchTerm.toLowerCase());

      const matchesPosition = positionFilter === "all" || skater.position === positionFilter;

      const matchesPlayoff =
        inPlayoffsFilter === "all" ||
        (inPlayoffsFilter === "in"
          ? isTeamInPlayoffs(skater.teamAbbrev)
          : !isTeamInPlayoffs(skater.teamAbbrev));

      return matchesSearch && matchesPosition && matchesPlayoff;
    });
  }, [allSkaters, searchTerm, positionFilter, inPlayoffsFilter, isTeamInPlayoffs]);

  return {
    allSkaters,
    filteredSkaters,
    positions,
    positionCounts,
    isLoading,
    error,
    refetch,

    // Filter state and setters
    searchTerm,
    setSearchTerm,
    positionFilter,
    setPositionFilter,
    inPlayoffsFilter,
    setInPlayoffsFilter
  };
}
