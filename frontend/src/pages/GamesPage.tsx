import { useParams } from "react-router-dom";
import DateHeader from "@/components/common/DateHeader";
import GameCard from "@/components/games/GameCard";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import { APP_CONFIG } from "@/config";
import { useGamesData } from "@/features/games";

const GamesPage = () => {
  const { date: dateParam } = useParams<{ date?: string }>();

  const {
    selectedDate,
    updateSelectedDate,
    filteredGames,
    gamesLoading,
    gamesError,
    refetchGames,
    expandedGames,
    toggleGameExpansion,
    hasLiveGames,
    getTeamPrimaryColor,
  } = useGamesData(dateParam);

  const formattedDate = new Date(selectedDate).toLocaleDateString("en-US", {
    weekday: "long",
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });

  return (
    <div className="space-y-4">
      <DateHeader
        selectedDate={selectedDate}
        onDateChange={updateSelectedDate}
        isFloating={true}
        minDate={APP_CONFIG.PLAYOFF_START}
        maxDate={APP_CONFIG.SEASON_END}
      />

      {gamesLoading ? (
        <div className="flex items-center justify-center min-h-[300px] bg-white rounded-none p-8 border border-gray-200">
          <LoadingSpinner size="large" message="Loading games data..." />
        </div>
      ) : gamesError ? (
        <div className="bg-white rounded-none p-8 border border-gray-200">
          <ErrorMessage
            message="Failed to load games data. Please try again."
            onRetry={refetchGames}
          />
        </div>
      ) : !filteredGames || filteredGames.length === 0 ? (
        <div className="bg-white rounded-none p-8 text-center border border-gray-200">
          <p className="text-gray-500">No game data available for the selected date.</p>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <span className="inline-block px-2 py-0.5 text-xs uppercase tracking-wider bg-[#FACC15]">{formattedDate}</span>
            <div className="flex items-center gap-2">
              {hasLiveGames && (
                <div className="flex items-center gap-1.5 bg-[#1A1A1A]/5 px-3 py-1.5 rounded-none text-sm border border-[#1A1A1A]/20 text-[#1A1A1A]">
                  <span className="h-2 w-2 rounded-full bg-red-500 animate-pulse" aria-hidden="true"></span>
                  Live — auto-updating
                </div>
              )}
              <button
                onClick={refetchGames}
                className="px-3 py-1.5 bg-[#1A1A1A]/5 text-[#1A1A1A] rounded-none hover:bg-[#1A1A1A]/10 flex items-center text-sm transition-colors border border-[#1A1A1A]/20"
              >
                <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                Refresh
              </button>
            </div>
          </div>

          <div className="space-y-4">
            {filteredGames.map((game) => (
              <GameCard
                key={game.id}
                game={game}
                isExpanded={expandedGames.has(game.id)}
                onToggleExpand={() => toggleGameExpansion(game.id)}
                getTeamPrimaryColor={getTeamPrimaryColor}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default GamesPage;
