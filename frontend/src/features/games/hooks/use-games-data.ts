import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { api } from "@/api/client";
import { QUERY_INTERVALS } from "@/config";
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

  // Two-pass query pattern: the first pass discovers `hasLiveGames` from
  // the current data, the second pass (via `refetchInterval`) keeps the
  // page live-updating only while the mirror says something is live.
  // The server-side live poller updates `nhl_games` + `nhl_player_game_stats`
  // every 60 s, so aligning the client at 30 s catches the next write
  // within one boxscore tick's worth of lag. When the slate is done
  // `refetchInterval` returns `false` and polling stops automatically.
  const {
    data: gamesData,
    isLoading: gamesLoading,
    error: gamesError,
    refetch: refetchGames,
  } = useQuery({
    queryKey: ["games", selectedDate, activeLeagueId],
    queryFn: () => api.getGames(selectedDate, activeLeagueId ?? undefined),
    retry: 1,
    refetchInterval: (query) => {
      const state = (query.state.data as typeof gamesData | undefined)?.games ?? [];
      const anyLive = state.some((g) => {
        const s = (g.gameState || "").toUpperCase();
        return s === "LIVE" || s === "CRIT";
      });
      return anyLive ? QUERY_INTERVALS.GAMES_LIVE_REFRESH_MS : false;
    },
  });

  const hasLiveGames =
    gamesData?.games?.some((game) => {
      const state = (game.gameState || "").toUpperCase();
      return state === "LIVE" || state === "CRIT";
    }) ?? false;

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
    hasLiveGames,
    isTodaySelected,
    getTeamPrimaryColor,
  };
}
