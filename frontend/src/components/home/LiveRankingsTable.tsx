import { usePulse } from "@/features/pulse";
import RankingTable from "@/components/common/RankingTable";
import { useLiveRankingsColumns } from "@/components/rankingsPageTableColumns/liveColumns";

/**
 * Appears at the top of the dashboard only while games are in flight
 * (`hasLiveGames` from Pulse). Lists every league team with at least
 * one rostered skater in tonight's NHL slate, sorted by today's live
 * points (from `v_daily_fantasy_totals`, same source as the Pulse
 * League Live Board). Renders via the shared `RankingTable` so row
 * heights and column treatment match Overall / Yesterday / Sleepers.
 *
 * Uses `usePulse` to piggy-back on its existing auto-refresh loop —
 * no new endpoint, no additional Claude calls.
 */
export function LiveRankingsTable() {
  const { pulse, isLoading } = usePulse();
  const columns = useLiveRankingsColumns();

  // Render nothing when pulse hasn't loaded, there are no live games,
  // or no team has any active skaters — the section is a live-only
  // surface and should not flash an empty frame during pre-game state.
  if (isLoading || !pulse || !pulse.hasLiveGames) return null;

  const today = new Date().toISOString().slice(0, 10);
  const rows = pulse.leagueBoard
    .filter((t) => t.playersActiveToday > 0)
    .sort((a, b) => {
      if (b.pointsToday !== a.pointsToday) return b.pointsToday - a.pointsToday;
      return b.totalPoints - a.totalPoints;
    })
    .map((t, idx) => ({
      rank: idx + 1,
      teamId: t.teamId,
      teamName: t.teamName,
      pointsToday: t.pointsToday,
      playersActiveToday: t.playersActiveToday,
      totalPoints: t.totalPoints,
    }));

  if (rows.length === 0) return null;

  return (
    <div className="mb-6">
      <RankingTable
        columns={columns}
        data={rows}
        keyField="teamId"
        rankField="rank"
        title="Live Rankings"
        viewAllLink={`/games/${today}`}
        viewAllText="Live Games"
        alwaysShowViewAll
      />
    </div>
  );
}
