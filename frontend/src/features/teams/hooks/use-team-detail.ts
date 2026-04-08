import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "@/api/client";
import { usePlayoffsData } from "@/features/rankings/hooks/use-playoffs-data";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { SkaterStats } from "@/types/skaters";
import { NHLTeamBet } from "@/types/fantasyTeams";

export function useTeamDetail(leagueId: string | null, teamId: number) {
  // ALWAYS call all hooks at the top level, unconditionally
  const { isTeamInPlayoffs, isLoading: playoffsLoading } = usePlayoffsData();

  const enabled = !!leagueId && !!teamId;

  // Fetch team data
  const { data: teams, isLoading: teamsLoading } = useQuery({
    queryKey: ["teams", leagueId],
    queryFn: () => api.getTeams(leagueId!),
    enabled: !!leagueId,
  });

  // Fetch team points
  const { data: teamPoints, isLoading: pointsLoading } = useQuery({
    queryKey: ["teamPoints", leagueId, teamId],
    queryFn: () => api.getTeamPoints(leagueId!, teamId),
    enabled,
  });

  // Fetch team bets
  const { data: teamBets, isLoading: betsLoading } = useQuery({
    queryKey: ["teamBets", leagueId],
    queryFn: () => api.getTeamBets(leagueId!),
    enabled: !!leagueId,
  });

  // Fetch sleepers for this league
  const { data: allSleepers } = useQuery({
    queryKey: ["sleepers", leagueId],
    queryFn: () => api.getSleepers(leagueId!),
    enabled: !!leagueId,
  });

  // Calculate playoff-related data using useMemo
  const playoffStats = useMemo(() => {
    // Default values when data isn't loaded yet
    const defaultStats = {
      teamsInPlayoffs: [] as NHLTeamBet[],
      playersInPlayoffs: [] as SkaterStats[],
    };

    // Only calculate if all dependencies are available
    if (!teamPoints || !teamBets || playoffsLoading || !isTeamInPlayoffs) {
      return defaultStats;
    }

    // Find the team's bets
    const currentTeamBets =
      teamBets.find((tb) => tb.teamId === teamId)?.bets || [];

    // Check if players array exists before accessing it
    const players = teamPoints?.players || [];

    // Filter for teams and players in playoffs
    const teamsInPlayoffs = currentTeamBets.filter((bet) =>
      isTeamInPlayoffs(bet.nhlTeam),
    );

    const playersInPlayoffs = players
      .filter((player) => isTeamInPlayoffs(player.nhlTeam || ""))
      .sort((a, b) => b.totalPoints - a.totalPoints);

    return {
      teamsInPlayoffs,
      playersInPlayoffs,
    };
  }, [teamId, teamPoints, teamBets, playoffsLoading, isTeamInPlayoffs]);

  // Find the team in the teams array
  const team = useMemo(() => {
    return teams?.find((t) => t.id === teamId);
  }, [teams, teamId]);

  // Process players to add URL slugs
  const processedPlayers = useMemo(() => {
    if (!teamPoints?.players) {
      return [];
    }

    return teamPoints.players.map((player) => ({
      ...player,
      nhlTeamUrlSlug: getNHLTeamUrlSlug(player.nhlTeam || ""),
    })).sort((a, b) => b.totalPoints - a.totalPoints);
  }, [teamPoints?.players]);

  // Get team bets
  const currentTeamBets = useMemo(() => {
    return teamBets?.find((tb) => tb.teamId === teamId)?.bets || [];
  }, [teamBets, teamId]);

  // Get this team's sleeper pick
  const teamSleeper = useMemo(() => {
    if (!allSleepers) return null;
    return (allSleepers as any[]).find((s: any) => {
      const tid = s.fantasyTeamId ?? s.fantasy_team_id;
      return tid === teamId;
    }) ?? null;
  }, [allSleepers, teamId]);

  // Loading state
  const isLoading =
    teamsLoading || pointsLoading || betsLoading || playoffsLoading;

  // Error state
  const hasError = !team || !teamPoints;

  return {
    team,
    teamPoints,
    processedPlayers,
    currentTeamBets,
    playoffStats,
    teamSleeper,
    isLoading,
    hasError,
  };
}
