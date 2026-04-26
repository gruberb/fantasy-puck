import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/api/client";
import RankingTable from "@/components/common/RankingTable";
import {
  useLiveRankingsColumns,
  type LiveRankingRow,
} from "@/components/rankingsPageTableColumns/liveColumns";
import { QUERY_INTERVALS } from "@/config";
import { useLeague } from "@/contexts/LeagueContext";
import type { Game, GamesResponse } from "@/types/games";
import type { FantasyTeamInAction } from "@/types/matchDay";
import { getHockeyDateToday } from "@/utils/timezone";

/**
 * Appears at the top of the dashboard only while games are in flight
 * according to the Games mirror endpoint. Lists every league team
 * with at least one rostered skater in today's NHL slate, sorted by
 * current same-day boxscore points.
 *
 * Uses the shared `RankingTable` body so rank badges, row heights,
 * and cell typography match the other dashboard tables (Overall,
 * Yesterday, Sleepers). Its red banner + pulse dot + "→ Live Games"
 * button sit inside the same outer border as the table body via the
 * `customHeader` slot.
 */
export function LiveRankingsTable() {
  const { activeLeagueId, myMemberships } = useLeague();
  const columns = useLiveRankingsColumns();
  const today = getHockeyDateToday();

  const { data, isLoading } = useQuery({
    queryKey: ["dashboardLiveGames", today, activeLeagueId],
    queryFn: () => api.getGames(today, activeLeagueId ?? undefined),
    enabled: !!activeLeagueId,
    retry: 1,
    staleTime: QUERY_INTERVALS.GAMES_LIVE_REFRESH_MS,
    refetchInterval: (query) => {
      const games = (query.state.data as GamesResponse | undefined)?.games ?? [];
      return games.some((g) => isUnfinishedState(g.gameState))
        ? QUERY_INTERVALS.GAMES_LIVE_REFRESH_MS
        : false;
    },
  });

  // Render nothing when games haven't loaded, there are no live games,
  // or no team has any active skaters. The section is a live-only
  // surface; we'd rather omit it than flash an empty frame.
  if (isLoading || !data || !data.games.some((g) => isLiveState(g.gameState))) return null;

  const myTeamId =
    myMemberships.find((m) => m.league_id === activeLeagueId)?.fantasy_team_id ?? null;
  const rows = buildRows(data, myTeamId);

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

function buildRows(data: GamesResponse, myTeamId: number | null): LiveRankingRow[] {
  return (data.fantasyTeams ?? [])
    .map((team) => buildRow(team, data.games, myTeamId))
    .filter((row) => row.playersActiveToday > 0)
    .sort((a, b) => {
      if (b.pointsToday !== a.pointsToday) return b.pointsToday - a.pointsToday;
      if (b.playersActiveToday !== a.playersActiveToday) {
        return b.playersActiveToday - a.playersActiveToday;
      }
      return a.teamName.localeCompare(b.teamName);
    })
    .map((row, idx) => ({ ...row, rank: idx + 1 }));
}

function buildRow(
  team: FantasyTeamInAction,
  gamesToday: Game[],
  myTeamId: number | null,
): LiveRankingRow {
  const roster = new Set(team.playersInAction.map((p) => p.nhlTeam));
  const games = gamesToday
    .filter((g) => roster.has(g.homeTeam) || roster.has(g.awayTeam))
    .map((g) => ({ homeTeam: g.homeTeam, awayTeam: g.awayTeam }));

  return {
    rank: 0,
    teamId: team.teamId,
    teamName: team.teamName,
    pointsToday: team.playersInAction.reduce((sum, p) => sum + p.points, 0),
    playersActiveToday: team.totalPlayersToday,
    games,
    roster,
    isMyTeam: team.teamId === myTeamId,
  };
}

function isLiveState(state: string | null | undefined): boolean {
  const s = (state ?? "").toUpperCase();
  return s === "LIVE" || s === "CRIT";
}

function isUnfinishedState(state: string | null | undefined): boolean {
  const s = (state ?? "").toUpperCase();
  return s !== "OFF" && s !== "FINAL";
}
