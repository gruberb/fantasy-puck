import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { api } from "@/api/client";
import { useLeague } from "@/contexts/LeagueContext";
import {
  getFixedAnalysisDateString,
  dateStringToLocalDate,
  isSameLocalDay,
} from "@/utils/timezone";
import { getTeamPrimaryColor } from "@/utils/teamStyles";

export function useGamesData(dateParam?: string) {
  const navigate = useNavigate();
  const { activeLeagueId } = useLeague();

  const isValidDate = (dateString: string): boolean => {
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
    if (!dateRegex.test(dateString)) return false;
    return !isNaN(new Date(dateString).getTime());
  };

  const [selectedDate, setSelectedDate] = useState<string>(() => {
    if (dateParam && isValidDate(dateParam)) return dateParam;
    return getFixedAnalysisDateString();
  });

  const [expandedGames, setExpandedGames] = useState<Set<number>>(new Set());
  const [autoRefresh, setAutoRefresh] = useState<boolean>(false);

  const updateSelectedDate = (newDate: string) => {
    setSelectedDate(newDate);
    navigate(`/games/${newDate}`, { replace: true });
  };

  const toggleGameExpansion = (gameId: number) => {
    setExpandedGames((prev) => {
      const next = new Set(prev);
      if (next.has(gameId)) next.delete(gameId);
      else next.add(gameId);
      return next;
    });
  };

  const {
    data: gamesData,
    isLoading: gamesLoading,
    error: gamesError,
    refetch: refetchGames,
  } = useQuery({
    queryKey: ["games", selectedDate, activeLeagueId],
    queryFn: () => api.getGames(selectedDate, activeLeagueId ?? undefined),
    retry: 1,
  });

  const hasLiveGames =
    gamesData?.games?.some(
      (game) => {
        const state = (game.gameState || "").toUpperCase();
        return state === "LIVE" || state === "CRIT";
      },
    ) ?? false;

  useEffect(() => {
    if (!autoRefresh || !hasLiveGames) return;
    const id = setInterval(() => refetchGames(), 30000);
    return () => clearInterval(id);
  }, [autoRefresh, hasLiveGames, refetchGames]);

  useEffect(() => {
    if (autoRefresh && !hasLiveGames) setAutoRefresh(false);
  }, [selectedDate, hasLiveGames, autoRefresh]);

  const isTodaySelected = isSameLocalDay(
    dateStringToLocalDate(selectedDate),
    new Date(),
  );

  return {
    selectedDate,
    updateSelectedDate,
    gamesData,
    filteredGames: gamesData?.games ?? [],
    gamesLoading,
    gamesError,
    refetchGames,
    expandedGames,
    toggleGameExpansion,
    autoRefresh,
    setAutoRefresh,
    hasLiveGames,
    isTodaySelected,
    getTeamPrimaryColor,
  };
}
