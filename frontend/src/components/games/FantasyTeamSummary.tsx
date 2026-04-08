import { useState, useEffect, useRef } from "react";
import { Link } from "react-router-dom";
import { useFantasyTeams } from "@/features/games";
import { useLeague } from "@/contexts/LeagueContext";
import { dateStringToLocalDate, formatDisplayDate } from "@/utils/timezone";
import PlayerCard from "@/components/common/PlayerCard";
import type { SkaterWithPoints } from "@/types/skaters";

interface FantasyTeamSummaryProps {
  selectedDate: string;
  onRefresh?: () => void;
}

const FantasyTeamSummary = ({
  selectedDate,
  onRefresh,
}: FantasyTeamSummaryProps) => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const { gamesData, fantasyTeamCounts, isLoading } =
    useFantasyTeams(selectedDate);

  // Convert selectedDate to Date object
  const selectedDateObj = dateStringToLocalDate(selectedDate);
  const formattedDate = formatDisplayDate(selectedDateObj, {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });

  // State for sorting and collapsed teams
  const [sortBy, setSortBy] = useState<"playerCount" | "totalPoints">(
    "totalPoints",
  );
  const [collapsedTeams, setCollapsedTeams] = useState<Set<number>>(new Set());
  const hasInitialized = useRef(false);

  // run *only once* when fantasyTeamCounts goes from [] → [ … ]
  useEffect(() => {
    if (!hasInitialized.current && fantasyTeamCounts.length > 0) {
      setCollapsedTeams(new Set(fantasyTeamCounts.map((t) => t.teamId)));
      hasInitialized.current = true;
    }
  }, [fantasyTeamCounts]);

  // Toggle team collapse state
  const toggleTeamCollapse = (teamId: number) => {
    setCollapsedTeams((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(teamId)) {
        newSet.delete(teamId);
      } else {
        newSet.add(teamId);
      }
      return newSet;
    });
  };

  // Collapse or expand all teams
  const toggleAllTeams = () => {
    if (collapsedTeams.size === fantasyTeamCounts.length) {
      // If all teams are collapsed, expand all
      setCollapsedTeams(new Set());
    } else {
      // Otherwise collapse all
      setCollapsedTeams(new Set(fantasyTeamCounts.map((t) => t.teamId)));
    }
  };

  // Sorted teams
  const sortedTeams = [...fantasyTeamCounts].sort((a, b) => {
    if (sortBy === "totalPoints") return b.totalPoints - a.totalPoints;
    return b.playerCount - a.playerCount;
  });

  // Check if all teams are collapsed
  const allTeamsCollapsed = collapsedTeams.size === fantasyTeamCounts.length;

  // Function to render loading state
  const renderLoadingState = () => {
    return (
      <div className="bg-white rounded-none p-4 animate-pulse">
        <div className="h-6 bg-gray-200 rounded w-1/4 mb-4"></div>
        <div className="space-y-2">
          <div className="h-12 bg-gray-100 rounded"></div>
          <div className="h-12 bg-gray-100 rounded"></div>
          <div className="h-12 bg-gray-100 rounded"></div>
        </div>
      </div>
    );
  };

  // Function to render empty state
  const renderEmptyState = () => {
    return (
      <div className="bg-white rounded-none p-6 text-center border border-gray-200">
        <div className="bg-[#2563EB]/5 w-16 h-16 mx-auto rounded-none flex items-center justify-center mb-4">
          <svg
            className="w-8 h-8 text-[#2563EB]/40"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M9.172 16.172a4 4 0 015.656 0M12 14a2 2 0 100-4 2 2 0 000 4z"
            />
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
        </div>
        <h3 className="text-lg font-bold text-gray-700 mb-2">
          No Fantasy Teams
        </h3>
        <p className="text-gray-500 mb-4">
          No fantasy teams have players in today's games.
        </p>

        <div className="p-4 bg-gray-50 rounded-none text-left text-sm text-gray-500 max-w-md mx-auto">
          <p className="flex justify-between mb-1">
            <span>Games loaded:</span>
            <span className="font-medium">{gamesData?.games?.length || 0}</span>
          </p>
          {gamesData?.games && gamesData.games.length > 0 && (
            <>
              <p className="flex justify-between">
                <span>Total players:</span>
                <span className="font-medium">
                  {gamesData.games.reduce(
                    (total, game) =>
                      total +
                      (game.homeTeamPlayers?.length || 0) +
                      (game.awayTeamPlayers?.length || 0),
                    0,
                  )}
                </span>
              </p>
            </>
          )}
        </div>
      </div>
    );
  };

  if (isLoading) {
    return renderLoadingState();
  }

  if (fantasyTeamCounts.length === 0) {
    return renderEmptyState();
  }

  return (
    <div className="space-y-4">
      {/* Header with gradient background - matching the RankingTable style */}
      <div className="ranking-table-container overflow-hidden transition-all duration-300">
        <div className="ranking-table-header p-5 border-b border-gray-100">
          <div className="flex flex-col sm:flex-row justify-between sm:items-center gap-3">
            <div>
              <h2 className="text-2xl font-bold mb-1">
                Fantasy Teams Standings
              </h2>
              <div className="flex items-center">
                <span className="bg-[#FACC15] text-[#1A1A1A] text-xs px-3 py-1 rounded-none font-medium">
                  {formattedDate}
                </span>
              </div>
            </div>

            <div className="flex items-center gap-2">
              {/* Sort buttons */}
              <div className="flex border border-[#1A1A1A]/20 rounded-none overflow-hidden">
                <button
                  onClick={() => setSortBy("playerCount")}
                  className={`px-3 py-1.5 text-sm font-medium transition-colors ${
                    sortBy === "playerCount"
                      ? "bg-[#1A1A1A]/10 text-[#1A1A1A]"
                      : "hover:bg-[#1A1A1A]/5 text-[#1A1A1A]/70"
                  }`}
                >
                  Players
                </button>
                <button
                  onClick={() => setSortBy("totalPoints")}
                  className={`px-3 py-1.5 text-sm font-medium transition-colors ${
                    sortBy === "totalPoints"
                      ? "bg-[#1A1A1A]/10 text-[#1A1A1A]"
                      : "hover:bg-[#1A1A1A]/5 text-[#1A1A1A]/70"
                  }`}
                >
                  Points
                </button>
              </div>

              {/* Collapse All button */}
              <button
                onClick={toggleAllTeams}
                className="px-3 py-1.5 bg-[#1A1A1A]/5 text-[#1A1A1A] rounded-none hover:bg-[#1A1A1A]/10 text-sm font-medium flex items-center border border-[#1A1A1A]/20"
              >
                <svg
                  className={`w-4 h-4 mr-1 transition-transform ${
                    allTeamsCollapsed ? "" : "rotate-180"
                  }`}
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M19 9l-7 7-7-7"
                  />
                </svg>
                {allTeamsCollapsed ? "Expand" : "Collapse"}
              </button>

              {/* Refresh button */}
              {onRefresh && (
                <button
                  onClick={onRefresh}
                  className="px-3 py-1.5 bg-[#1A1A1A]/5 text-[#1A1A1A] rounded-none hover:bg-[#1A1A1A]/10 flex items-center text-sm transition-colors border border-[#1A1A1A]/20"
                >
                  <svg
                    className="w-4 h-4 mr-1"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                    />
                  </svg>
                  Refresh
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Teams List - improved layout with shadows and borders */}
        <div className="ranking-table-body bg-white">
          <div className="divide-y divide-gray-200">
            {sortedTeams.map((team, index) => (
              <TeamRow
                key={team.teamId}
                team={team}
                index={index}
                isCollapsed={collapsedTeams.has(team.teamId)}
                onToggle={() => toggleTeamCollapse(team.teamId)}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

// Separate component for team row
interface TeamRowProps {
  team: {
    teamId: number;
    teamName: string;
    teamLogo?: string;
    playerCount: number;
    players: SkaterWithPoints[];
    totalPoints: number;
  };
  index: number;
  isCollapsed: boolean;
  onToggle: () => void;
}

const TeamRow = ({ team, index, isCollapsed, onToggle }: TeamRowProps) => {
  // Function to get rank indicator style based on position
  const getRankStyle = (position: number) => {
    switch (position) {
      case 0: // First place
        return "w-8 h-8 bg-[#FACC15] text-[#1A1A1A] border-2 border-[#1A1A1A]";
      case 1: // Second place
        return "w-8 h-8 bg-gray-300 text-gray-800 border-2 border-[#1A1A1A]";
      case 2: // Third place
        return "w-8 h-8 bg-amber-600 text-white border-2 border-[#1A1A1A]";
      default: // All other positions
        return "w-8 h-8 bg-white border border-gray-200 text-gray-700";
    }
  };

  return (
    <div className="border-gray-200 hover:bg-gray-50 transition-colors">
      {/* Team header */}
      <div className="px-6 py-4 flex items-center">
        {/* Rank indicator */}
        <div
          className={`${getRankStyle(index)} rounded-none flex items-center justify-center font-bold text-sm mr-4 flex-shrink-0`}
        >
          {index + 1}
        </div>

        <div className="flex-1">
          <Link
            to={`${lp}/teams/${team.teamId}`}
            className="text-sm font-medium text-gray-900 hover:text-[#2563EB] hover:underline"
          >
            {team.teamName}
          </Link>
        </div>

        <div className="flex items-center space-x-8">
          {/* Points badge */}
          <div className="flex items-center">
            <span
              className={`inline-flex items-center justify-center px-3 py-1 rounded-none text-xs font-bold ${
                team.totalPoints > 0
                  ? "bg-green-100 text-green-800"
                  : "bg-gray-100 text-gray-800"
              }`}
            >
              {team.totalPoints} pts
            </span>
          </div>

          {/* Players count badge */}
          <div className="flex items-center">
            <span className="inline-flex items-center justify-center px-3 py-1 rounded-none text-xs font-medium bg-blue-100 text-blue-800">
              {team.playerCount} ⛸️
            </span>
          </div>

          {/* Toggle button with improved styling */}
          <button
            onClick={onToggle}
            className="flex items-center px-3 py-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded-none text-sm font-medium transition-colors border border-gray-200"
          >
            {/* <span className="mr-1">{isCollapsed ? "Show" : "Hide"}</span> */}
            <svg
              className={`w-4 h-4 transform transition-transform ${
                isCollapsed ? "" : "rotate-180"
              }`}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 9l-7 7-7-7"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* Player details - conditionally rendered based on collapsed state */}
      {!isCollapsed && (
        <div className="border-t border-gray-200 bg-gray-50 p-6">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            {team.players.map((player, idx) => (
              <PlayerCard
                key={idx}
                player={player}
                showFantasyTeam={false}
                showPoints={true}
                valueLabel="pts"
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default FantasyTeamSummary;
