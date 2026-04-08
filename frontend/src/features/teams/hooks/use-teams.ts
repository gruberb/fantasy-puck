import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { api } from "@/api/client";

export function useTeams(leagueId: string | null) {
  const [searchTerm, setSearchTerm] = useState("");

  const {
    data: teams,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["teams", leagueId],
    queryFn: () => api.getTeams(leagueId!),
    enabled: !!leagueId,
  });

  const filteredTeams = useMemo(() => {
    if (!teams || !Array.isArray(teams)) return [];

    return teams.filter((team) =>
      team.name.toLowerCase().includes(searchTerm.toLowerCase()),
    );
  }, [teams, searchTerm]);

  return {
    teams: filteredTeams,
    isLoading,
    error,
    searchTerm,
    setSearchTerm,
  };
}
