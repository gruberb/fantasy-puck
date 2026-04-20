import { useParams } from "react-router-dom";
import { ErrorMessage, LiveIndicator, LoadingSpinner } from "@gruberb/fun-ui";
import DateHeader from "@/components/common/DateHeader";
import GameCard from "@/components/games/GameCard";
import { APP_CONFIG } from "@/config";
import { useGamesData } from "@/features/games";
import { dateStringToLocalDate, formatDisplayDate } from "@/utils/timezone";

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

  const formattedDate = formatDisplayDate(dateStringToLocalDate(selectedDate), {
    weekday: "long",
    month: "short",
    day: "numeric",
    year: "numeric",
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
            {hasLiveGames && <LiveIndicator label="Live · auto-updating" />}
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
