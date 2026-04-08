import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { api } from "@/api/client";
import {
  getFixedAnalysisDateString,
  dateStringToLocalDate,
  isSameLocalDay,
} from "@/utils/timezone";
import { useLeague } from "@/contexts/LeagueContext";
import { getTeamPrimaryColor } from "@/utils/teamStyles";

export function useGamesData(dateParam?: string) {
  const navigate = useNavigate();
  const { activeLeagueId } = useLeague();

  // Helper to validate date format
  const isValidDate = (dateString: string): boolean => {
    // Check if the date string matches format YYYY-MM-DD
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
    if (!dateRegex.test(dateString)) return false;

    // Check if it's a valid date
    const date = new Date(dateString);
    return !isNaN(date.getTime());
  };

  // State for date selector - initialized from URL parameter if valid, otherwise use fixed analysis date
  const [selectedDate, setSelectedDate] = useState<string>(() => {
    // If there's a valid date in the URL, use it
    if (dateParam && isValidDate(dateParam)) {
      return dateParam;
    }
    // Otherwise, use the fixed analysis date (June 17, 2024)
    return getFixedAnalysisDateString();
  });

  // State for tabs and game expansions
  const [activeTab, setActiveTab] = useState("games");
  const [expandedGames, setExpandedGames] = useState<Set<number>>(new Set());
  const [autoRefresh, setAutoRefresh] = useState<boolean>(false);

  // Update URL when date changes
  const updateSelectedDate = (newDate: string) => {
    setSelectedDate(newDate);
    // Update URL without full reload
    navigate(`/games/${newDate}`, { replace: true });
  };

  // Toggle game expansion state
  const toggleGameExpansion = (gameId: number) => {
    setExpandedGames((prevExpandedGames) => {
      const newExpandedGames = new Set(prevExpandedGames);
      if (newExpandedGames.has(gameId)) {
        newExpandedGames.delete(gameId);
      } else {
        newExpandedGames.add(gameId);
      }
      return newExpandedGames;
    });
  };

  // Fetch data for the selected date (with extended fantasy data when league is active)
  const {
    data: gamesData,
    isLoading: gamesLoading,
    error: gamesError,
    refetch: refetchGames,
  } = useQuery({
    queryKey: ["games", selectedDate, activeLeagueId],
    queryFn: () => api.getGames(selectedDate, activeLeagueId || undefined),
    retry: 1,
  });

  // Check if there are live games on the selected date
  const hasLiveGames =
    (gamesData?.games &&
      gamesData.games.length > 0 &&
      gamesData.games.some(
        (game) =>
          (game.gameState || "").toUpperCase() === "LIVE" ||
          (game.gameState || "").toUpperCase() === "CRIT",
      )) ||
    false;

  // Auto-refresh for live games - Updated Logic
  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval> | null = null;

    if (autoRefresh && hasLiveGames) {
      intervalId = setInterval(() => {
        // Always refetch if there are live games, regardless of selected date
        refetchGames();
      }, 30000); // Refresh every 30 seconds
    }

    return () => {
      if (intervalId) clearInterval(intervalId);
    };
  }, [autoRefresh, hasLiveGames, refetchGames]);

  // Reset autoRefresh when date changes to prevent stale auto-refresh
  useEffect(() => {
    // When the selected date changes, check if we should keep autoRefresh on
    // If there are no live games on the new date, turn off autoRefresh
    if (autoRefresh && !hasLiveGames) {
      setAutoRefresh(false);
    }
  }, [selectedDate, hasLiveGames, autoRefresh]);

  // Check if selected date is today
  const isTodaySelected = isSameLocalDay(
    dateStringToLocalDate(selectedDate),
    new Date(),
  );

  const filteredGames = gamesData?.games ?? [];

  // Extended fantasy team data (only present when league is active)
  const fantasyTeams = gamesData?.fantasyTeams ?? [];

  return {
    selectedDate,
    updateSelectedDate,
    activeTab,
    setActiveTab,
    gamesData,
    filteredGames,
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
    fantasyTeams,
  };
}
