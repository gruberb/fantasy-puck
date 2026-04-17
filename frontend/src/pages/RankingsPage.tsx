import RankingTable from "@/components/common/RankingTable";
import { useRankingsData } from "@/features/rankings";
import { useLeague } from "@/contexts/LeagueContext";
import { APP_CONFIG } from "@/config";

import { useDailyRankingsColumns } from "@/components/rankingsPageTableColumns/dailysColumns";
import { usePlayoffRankingsColumns } from "@/components/rankingsPageTableColumns/playoffColumns";
import { TeamStatsColumns } from "@/components/rankingsPageTableColumns/teamStatsColumns";

const RankingsPage = () => {
  const { activeLeagueId } = useLeague();
  const {
    selectedDate,
    setSelectedDate,
    dailyRankings,
    dailyRankingsLoading,
    playoffRankings,
    playoffRankingsLoading,
    teamStats,
    teamStatsLoading,
    teamStatsError,
  } = useRankingsData(activeLeagueId);

  const teamStatsColumns = TeamStatsColumns();
  const dailyRankingsColumns = useDailyRankingsColumns();
  const playoffRankingsColumns = usePlayoffRankingsColumns();

  const processedDailyRankings = Array.isArray(dailyRankings) ? dailyRankings : [];

  return (
    <div>
      <div>
        {/* Team Stats Table */}
        <RankingTable
          columns={teamStatsColumns}
          data={teamStats || []}
          keyField="teamId"
          rankField="rank"
          title="Season Overview"
          dateBadge={APP_CONFIG.SEASON_LABEL}
          onRowClick={null}
          isLoading={teamStatsLoading}
          emptyMessage={
            teamStatsError
              ? "Failed to load team statistics"
              : "No team statistics available"
          }
          initialSortKey="totalPoints"
          initialSortDirection="desc"
        />
      </div>

      {/* Playoff Rankings */}
      <div className="mt-8">
        <RankingTable
          columns={playoffRankingsColumns}
          data={playoffRankings}
          keyField="teamId"
          rankField="rank"
          dateBadge={APP_CONFIG.SEASON_LABEL}
          title="Playoff Stats"
          isLoading={playoffRankingsLoading}
          emptyMessage="No playoff rankings data available"
          initialSortKey="rank"
          initialSortDirection="asc"
        />
      </div>

      <div className="mt-8">
        <RankingTable
          columns={dailyRankingsColumns}
          data={processedDailyRankings}
          keyField="teamId"
          rankField="rank"
          title="Daily Fantasy Scores"
          isLoading={dailyRankingsLoading}
          emptyMessage={"No daily rankings available for this date"}
          showDatePicker={true} // Enable date picker
          selectedDate={selectedDate}
          onDateChange={setSelectedDate}
          initialSortKey="dailyPoints"
          initialSortDirection="desc"
        />
      </div>
    </div>
  );
};

export default RankingsPage;
