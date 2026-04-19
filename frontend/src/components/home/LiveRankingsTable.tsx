import { Link } from "react-router-dom";
import { usePulse } from "@/features/pulse";
import RankingTable from "@/components/common/RankingTable";
import {
  useLiveRankingsColumns,
  type LiveRankingRow,
} from "@/components/rankingsPageTableColumns/liveColumns";
import type { PulseResponse } from "@/features/pulse/types";

/**
 * Appears at the top of the dashboard only while games are in flight
 * (`hasLiveGames` from Pulse). Lists every league team with at least
 * one rostered skater in tonight's NHL slate, sorted by today's live
 * points (from `v_daily_fantasy_totals`, same source as the Pulse
 * League Live Board).
 *
 * Uses the shared `RankingTable` body so rank badges, row heights,
 * and cell typography match the other dashboard tables (Overall,
 * Yesterday, Sleepers). Its red banner + pulse dot + "→ Live Games"
 * button sit inside the same outer border as the table body via the
 * `customHeader` slot.
 */
export function LiveRankingsTable() {
  const { pulse, isLoading } = usePulse();
  const columns = useLiveRankingsColumns();

  // Render nothing when pulse hasn't loaded, there are no live games,
  // or no team has any active skaters. The section is a live-only
  // surface; we'd rather omit it than flash an empty frame.
  if (isLoading || !pulse || !pulse.hasLiveGames) return null;

  const today = new Date().toISOString().slice(0, 10);
  const rosterByTeam = buildRosterIndex(pulse);
  const rows = buildRows(pulse, rosterByTeam);

  if (rows.length === 0) return null;

  return (
    <div className="mb-6">
      <RankingTable
        columns={columns}
        data={rows}
        keyField="teamId"
        rankField="rank"
        customHeader={<LiveHeader to={`/games/${today}`} />}
        /* Rows are already pre-sorted (pointsToday DESC, tiebreak by
           playersActiveToday) with `rank` assigned to match. Without
           these props `RankingTable` falls back to its default
           `"desc"` sort on the first column, which flips the list
           upside-down (rank 14 at the top). */
        initialSortKey="rank"
        initialSortDirection="asc"
      />
    </div>
  );
}

function LiveHeader({ to }: { to: string }) {
  return (
    <div className="bg-[#EF4444] text-white px-5 py-3 flex items-center justify-between gap-3 border-b-2 border-[#1A1A1A]">
      <div className="flex items-center gap-2 min-w-0">
        <span
          className="w-2 h-2 bg-white rounded-full animate-pulse flex-shrink-0"
          aria-hidden
        />
        <h2 className="font-extrabold uppercase tracking-wider text-2xl truncate">
          Live Rankings
        </h2>
      </div>
      <Link
        to={to}
        className="px-3 py-1 bg-white text-[#1A1A1A] font-extrabold uppercase tracking-wider text-[11px] border-2 border-white hover:bg-[#1A1A1A] hover:text-white hover:border-[#1A1A1A] transition-colors whitespace-nowrap"
      >
        → Live Games
      </Link>
    </div>
  );
}

function buildRows(
  pulse: PulseResponse,
  rosterByTeam: Map<number, Set<string>>,
): LiveRankingRow[] {
  return pulse.leagueBoard
    .filter((t) => t.playersActiveToday > 0)
    .sort((a, b) => {
      if (b.pointsToday !== a.pointsToday) return b.pointsToday - a.pointsToday;
      return b.playersActiveToday - a.playersActiveToday;
    })
    .map((t, idx) => {
      const roster = rosterByTeam.get(t.teamId) ?? new Set<string>();
      const games = pulse.gamesToday.filter(
        (g) => roster.has(g.homeTeam) || roster.has(g.awayTeam),
      );
      return {
        rank: idx + 1,
        teamId: t.teamId,
        teamName: t.teamName,
        pointsToday: t.pointsToday,
        playersActiveToday: t.playersActiveToday,
        games,
        roster,
        isMyTeam: t.isMyTeam,
      };
    });
}

function buildRosterIndex(pulse: PulseResponse): Map<number, Set<string>> {
  const map = new Map<number, Set<string>>();
  for (const team of pulse.seriesForecast) {
    const set = new Set<string>();
    for (const cell of team.cells) {
      set.add(cell.nhlTeam);
    }
    map.set(team.teamId, set);
  }
  return map;
}
