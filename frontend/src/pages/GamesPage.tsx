import { useParams, useSearchParams } from "react-router-dom";
import { useEffect } from "react";
import DateHeader from "@/components/common/DateHeader";
import GameTabs from "@/components/games/GameTabs";
import GameCard from "@/components/games/GameCard";
import FantasyTeamSummary from "@/components/games/FantasyTeamSummary";
import FantasySummaryCards from "@/components/games/FantasySummary";
import FantasyTeamCard from "@/components/games/FantasyTeamCard";
import PlayerComparison from "@/components/games/PlayerComparison";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import { useGamesData } from "@/features/games";
import { useLeague } from "@/contexts/LeagueContext";

const GamesPage = () => {
  const { date: dateParam } = useParams<{ date?: string }>();
  const [searchParams, setSearchParams] = useSearchParams();

  const {
    selectedDate,
    updateSelectedDate,
    activeTab,
    setActiveTab,
    filteredGames,
    gamesLoading,
    gamesError,
    refetchGames,
    expandedGames,
    toggleGameExpansion,
    autoRefresh,
    setAutoRefresh,
    hasLiveGames,
    getTeamPrimaryColor,
    fantasyTeams,
  } = useGamesData(dateParam);

  const { activeLeagueId } = useLeague();
  const hasExtendedData = fantasyTeams.length > 0;
  // Show Fantasy/Matchups tabs whenever a league is active, even if no players in today's games
  const hasLeagueContext = !!activeLeagueId;

  // Set active tab from URL parameter
  useEffect(() => {
    const tabParam = searchParams.get("tab");
    if (tabParam === "fantasy" || tabParam === "games" || tabParam === "matchups") {
      setActiveTab(tabParam);
    }
  }, [searchParams, setActiveTab]);

  // Update URL when tab changes
  const handleTabChange = (tab: string) => {
    setActiveTab(tab);
    searchParams.set("tab", tab);
    setSearchParams(searchParams);
  };

  return (
    <div className="space-y-4">
      <DateHeader
        selectedDate={selectedDate}
        onDateChange={updateSelectedDate}
        isFloating={true}
      />

      <GameTabs
        activeTab={activeTab}
        setActiveTab={handleTabChange}
        hasFantasyTeams={hasExtendedData || hasLeagueContext}
        hasExtendedData={hasExtendedData || hasLeagueContext}
      />

      {activeTab === "games" ? (
        <GamesContent
          filteredGames={filteredGames}
          isLoading={gamesLoading}
          error={gamesError}
          expandedGames={expandedGames}
          toggleGameExpansion={toggleGameExpansion}
          onRefresh={refetchGames}
          getTeamPrimaryColor={getTeamPrimaryColor}
          hasLiveGames={hasLiveGames}
          autoRefresh={autoRefresh}
          setAutoRefresh={setAutoRefresh}
          selectedDate={selectedDate}
        />
      ) : activeTab === "matchups" ? (
        <MatchupsContent
          filteredGames={filteredGames}
          fantasyTeams={fantasyTeams}
          isLoading={gamesLoading}
          getTeamPrimaryColor={getTeamPrimaryColor}
        />
      ) : hasLeagueContext ? (
        <FantasyContent
          fantasyTeams={fantasyTeams}
          isLoading={gamesLoading}
          getTeamPrimaryColor={getTeamPrimaryColor}
        />
      ) : (
        <FantasyTeamSummary
          selectedDate={selectedDate}
          onRefresh={refetchGames}
        />
      )}
    </div>
  );
};

// ── Games tab content ──────────────────────────────────────────────────────

interface GamesContentProps {
  filteredGames: any[];
  isLoading: boolean;
  error: unknown;
  expandedGames: Set<number>;
  toggleGameExpansion: (gameId: number) => void;
  onRefresh: () => void;
  getTeamPrimaryColor: (teamName: string) => string;
  hasLiveGames: boolean;
  autoRefresh: boolean;
  setAutoRefresh: (value: boolean) => void;
  selectedDate: string;
}

const GamesContent = ({
  filteredGames,
  isLoading,
  error,
  expandedGames,
  toggleGameExpansion,
  onRefresh,
  getTeamPrimaryColor,
  hasLiveGames,
  autoRefresh,
  setAutoRefresh,
  selectedDate,
}: GamesContentProps) => {
  const formattedDate = new Date(selectedDate).toLocaleDateString("en-US", {
    weekday: "long",
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[300px] bg-white rounded-none p-8 border border-gray-200">
        <LoadingSpinner size="large" message="Loading games data..." />
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-white rounded-none p-8 border border-gray-200">
        <ErrorMessage
          message="Failed to load games data. Please try again."
          onRetry={onRefresh}
        />
      </div>
    );
  }

  if (!filteredGames || filteredGames.length === 0) {
    return (
      <div className="bg-white rounded-none p-8 text-center border border-gray-200">
        <svg className="w-16 h-16 text-gray-300 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M9.172 16.172a4 4 0 015.656 0M12 14a2 2 0 100-4 2 2 0 000 4z" />
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p className="text-gray-500">No game data available for the selected date.</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="inline-block px-2 py-0.5 text-xs uppercase tracking-wider bg-[#FACC15]">{formattedDate}</span>
        </div>
        <div className="flex items-center gap-2">
          {hasLiveGames && (
            <div className="flex items-center bg-[#1A1A1A]/5 px-3 py-1.5 rounded-none text-sm border border-[#1A1A1A]/20">
              <input
                type="checkbox"
                id="autoRefresh"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
                className="mr-2"
              />
              <label htmlFor="autoRefresh" className="text-[#1A1A1A] flex items-center text-sm">
                Auto-refresh
                {autoRefresh && (
                  <span className="ml-1 h-2 w-2 rounded-full bg-red-500 animate-pulse" title="Refreshing every 30 seconds"></span>
                )}
              </label>
            </div>
          )}
          <button
            onClick={onRefresh}
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
  );
};

// ── Fantasy tab content (extended data from backend) ───────────────────────

import type { FantasyTeamInAction } from "@/types/matchDay";

interface FantasyContentProps {
  fantasyTeams: FantasyTeamInAction[];
  isLoading: boolean;
  getTeamPrimaryColor: (teamName: string) => string;
}

const FantasyContent = ({ fantasyTeams, isLoading, getTeamPrimaryColor }: FantasyContentProps) => {
  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[300px]">
        <LoadingSpinner size="large" message="Loading fantasy data..." />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <FantasySummaryCards fantasyTeams={fantasyTeams} />

      <div>
        {fantasyTeams.length === 0 ? (
          <div className="bg-white border-2 border-[#1A1A1A] rounded-none p-8 text-center">
            <p className="text-gray-500 text-sm uppercase tracking-wider font-medium">
              No fantasy teams have players in today's games.
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-6">
            {fantasyTeams
              .sort((a, b) => b.totalPlayersToday - a.totalPlayersToday)
              .map((team) => (
                <FantasyTeamCard
                  key={team.teamId}
                  team={team}
                />
              ))}
          </div>
        )}
      </div>
    </div>
  );
};

// ── Matchups tab content ───────────────────────────────────────────────────

interface MatchupsContentProps {
  filteredGames: any[];
  fantasyTeams: FantasyTeamInAction[];
  isLoading: boolean;
  getTeamPrimaryColor: (teamName: string) => string;
}

const MatchupsContent = ({ filteredGames, fantasyTeams, isLoading, getTeamPrimaryColor }: MatchupsContentProps) => {
  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[300px]">
        <LoadingSpinner size="large" message="Loading matchup data..." />
      </div>
    );
  }

  if (!filteredGames || filteredGames.length === 0) {
    return (
      <div className="bg-white rounded-none p-8 text-center border border-gray-200">
        <p className="text-gray-500">No games available for matchup comparisons.</p>
      </div>
    );
  }

  // Check if there are any actual matchups to show
  const hasAnyMatchups = fantasyTeams.length > 0 && filteredGames.some((game) =>
    fantasyTeams.some((team) =>
      team.playersInAction.some(
        (p) => p.nhlTeam === game.homeTeam || p.nhlTeam === game.awayTeam,
      ),
    ),
  );

  if (!hasAnyMatchups) {
    return (
      <div className="bg-white border-2 border-[#1A1A1A] rounded-none p-8 text-center">
        <p className="text-gray-500 text-sm uppercase tracking-wider font-medium">
          No fantasy player matchups for today's games.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {filteredGames.map((game) => (
        <div key={game.id} className="space-y-2">
          <div className="flex items-center gap-2 text-sm font-bold uppercase tracking-wider text-[#1A1A1A]">
            <span
              className="w-3 h-3 rounded-none"
              style={{ backgroundColor: getTeamPrimaryColor(game.awayTeam) }}
            />
            {game.awayTeam}
            <span className="text-gray-400 font-normal">@</span>
            {game.homeTeam}
            <span
              className="w-3 h-3 rounded-none"
              style={{ backgroundColor: getTeamPrimaryColor(game.homeTeam) }}
            />
          </div>
          <PlayerComparison game={game} fantasyTeams={fantasyTeams} />
        </div>
      ))}
    </div>
  );
};

export default GamesPage;
