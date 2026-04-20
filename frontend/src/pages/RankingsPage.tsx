import RankingTable from "@/components/common/RankingTable";
import { useRankingsData } from "@/features/rankings";
import { useLeague } from "@/contexts/LeagueContext";
import { useLeagueStats } from "@/features/league-stats/hooks/use-league-stats";
import { APP_CONFIG } from "@/config";

import { useDailyRankingsColumns } from "@/components/rankingsPageTableColumns/dailysColumns";
import { usePlayoffRankingsColumns } from "@/components/rankingsPageTableColumns/playoffColumns";
import { TeamStatsColumns } from "@/components/rankingsPageTableColumns/teamStatsColumns";
import { useNhlTeamRosterColumns } from "@/components/rankingsPageTableColumns/nhlTeamRosterColumns";
import { useTopRosteredSkatersColumns } from "@/components/rankingsPageTableColumns/topRosteredSkatersColumns";

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

  const {
    data: leagueStats,
    isLoading: leagueStatsLoading,
    isError: leagueStatsError,
  } = useLeagueStats(activeLeagueId);

  const teamStatsColumns = TeamStatsColumns();
  const dailyRankingsColumns = useDailyRankingsColumns();
  const playoffRankingsColumns = usePlayoffRankingsColumns();
  const nhlTeamRosterColumns = useNhlTeamRosterColumns();
  const topRosteredSkatersColumns = useTopRosteredSkatersColumns();

  const processedDailyRankings = Array.isArray(dailyRankings) ? dailyRankings : [];

  return (
    <div>
      {/* 1. Season Overview */}
      <div>
        <RankingTable
          columns={teamStatsColumns}
          data={teamStats || []}
          keyField="teamId"
          rankField="rank"
          title="Season Overview"
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

      {/* 2. Daily Fantasy Scores — date picker + prev/next + Yesterday */}
      <div className="mt-8">
        <RankingTable
          columns={dailyRankingsColumns}
          data={processedDailyRankings}
          keyField="teamId"
          rankField="rank"
          title="Daily Fantasy Scores"
          isLoading={dailyRankingsLoading}
          emptyMessage={"No daily rankings available for this date"}
          showDatePicker={true}
          selectedDate={selectedDate}
          onDateChange={setSelectedDate}
          minDate={APP_CONFIG.PLAYOFF_START}
          maxDate={APP_CONFIG.SEASON_END}
          initialSortKey="dailyPoints"
          initialSortDirection="desc"
        />
      </div>

      {/* 3. Playoff Stats */}
      <div className="mt-8">
        <RankingTable
          columns={playoffRankingsColumns}
          data={playoffRankings}
          keyField="teamId"
          rankField="rank"
          title="Playoff Stats"
          isLoading={playoffRankingsLoading}
          emptyMessage="No playoff rankings data available"
          initialSortKey="rank"
          initialSortDirection="asc"
        />
      </div>

      {/* 4. NHL Teams we roster */}
      <div className="mt-8">
        <RankingTable
          columns={nhlTeamRosterColumns}
          data={leagueStats?.nhlTeamsRostered ?? []}
          keyField="nhlTeam"
          rankField="rank"
          title="NHL Teams We Roster"
          isLoading={leagueStatsLoading}
          emptyMessage={
            leagueStatsError
              ? "Failed to load league stats"
              : "No rostered NHL teams yet"
          }
          initialSortKey="playoffPoints"
          initialSortDirection="desc"
        />
      </div>

      {/* 5. Top 10 Rostered Skaters */}
      <div className="mt-8">
        <RankingTable
          columns={topRosteredSkatersColumns}
          data={leagueStats?.topRosteredSkaters ?? []}
          keyField="nhlId"
          rankField="rank"
          title="Top 10 Rostered Skaters"
          isLoading={leagueStatsLoading}
          emptyMessage={
            leagueStatsError
              ? "Failed to load league stats"
              : "No rostered skaters with playoff stats yet"
          }
          initialSortKey="playoffPoints"
          initialSortDirection="desc"
        />
      </div>
    </div>
  );
};

export default RankingsPage;
