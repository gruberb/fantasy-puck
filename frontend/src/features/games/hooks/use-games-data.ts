import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { api } from "@/api/client";
import { QUERY_INTERVALS, clampToSeasonWindow } from "@/config";
import { useLeague } from "@/contexts/use-league";
import { getHockeyDateToday } from "@/utils/timezone";
import { getTeamPrimaryColor } from "@/utils/teamStyles";
import { isLive } from "@/utils/gameState";
import type { Game } from "@/types/games";

export const gamesQueryKey = (date: string, leagueId: string | null) =>
  ["games", date, leagueId] as const;

interface GamesQueryOptions {
  /** Poll at `GAMES_LIVE_REFRESH_MS` while this returns true for the latest slate. */
  pollWhile: (games: Game[]) => boolean;
  enabled?: boolean;
  staleTime?: number;
}

/**
 * Single cache entry per (date, league) so the dashboard's live table and
 * the Games page share one request. React Query polls at the shortest
 * interval any mounted observer asks for.
 */
export function useGamesQuery(
  date: string,
  leagueId: string | null,
  { pollWhile, enabled = true, staleTime }: GamesQueryOptions,
) {
  return useQuery({
    queryKey: gamesQueryKey(date, leagueId),
    queryFn: () => api.getGames(date, leagueId ?? undefined),
    enabled,
    staleTime,
    retry: 1,
    refetchInterval: (query) =>
      pollWhile(query.state.data?.games ?? [])
        ? QUERY_INTERVALS.GAMES_LIVE_REFRESH_MS
        : false,
  });
}

export function useGamesData(dateParam?: string) {
  const navigate = useNavigate();
  const { activeLeagueId } = useLeague();

  const isValidDate = (dateString: string): boolean => {
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
    if (!dateRegex.test(dateString)) return false;
    return !isNaN(new Date(dateString).getTime());
  };

  const [selectedDate, setSelectedDate] = useState<string>(() => {
    if (dateParam && isValidDate(dateParam)) return clampToSeasonWindow(dateParam);
    return clampToSeasonWindow(getHockeyDateToday());
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
  } = useGamesQuery(selectedDate, activeLeagueId, {
    pollWhile: (games) => games.some((g) => isLive(g.gameState)),
  });

  const hasLiveGames = gamesData?.games?.some((g) => isLive(g.gameState)) ?? false;

  const isTodaySelected = selectedDate === getHockeyDateToday();

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
