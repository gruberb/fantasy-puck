import { Link } from "react-router-dom";
import { usePulse } from "@/features/pulse";
import { useLeague } from "@/contexts/LeagueContext";
import type { GameMatchup, PulseResponse } from "@/features/pulse/types";

/**
 * Appears at the top of the dashboard only while games are in flight
 * (`hasLiveGames` from Pulse). Lists every league team with at least
 * one rostered skater in tonight's NHL slate, sorted by today's live
 * points (from `v_daily_fantasy_totals`, same source as the Pulse
 * League Live Board).
 *
 * Each row's trailing column lists the NHL matchups that fantasy
 * team has a stake in — the rostered side(s) are bolded. Reuses
 * `usePulse` so we piggy-back on its auto-refresh loop; no new
 * endpoint, no additional Claude calls.
 */
export function LiveRankingsTable() {
  const { pulse, isLoading } = usePulse();
  const { activeLeagueId } = useLeague();

  // Render nothing when pulse hasn't loaded, there are no live games,
  // or every team has zero active skaters. The section is a live-only
  // surface; we'd rather omit it than flash an empty frame.
  if (isLoading || !pulse || !pulse.hasLiveGames) return null;

  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const today = new Date().toISOString().slice(0, 10);

  // NHL abbrevs rostered per fantasy team. `seriesForecast` is the
  // authoritative source since it carries every player's nhl_team.
  const rosterByTeam = buildRosterIndex(pulse);

  const rows = pulse.leagueBoard
    .filter((t) => t.playersActiveToday > 0)
    .sort((a, b) => {
      if (b.pointsToday !== a.pointsToday) return b.pointsToday - a.pointsToday;
      return b.playersActiveToday - a.playersActiveToday;
    });

  if (rows.length === 0) return null;

  return (
    <section className="mb-6 bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[#EF4444] text-white px-5 py-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span className="w-2 h-2 bg-white rounded-full animate-pulse flex-shrink-0" aria-hidden />
          <h2 className="font-extrabold uppercase tracking-wider text-sm truncate">
            Live Rankings
          </h2>
        </div>
        <Link
          to={`/games/${today}`}
          className="px-3 py-1 bg-white text-[#1A1A1A] font-extrabold uppercase tracking-wider text-[11px] border-2 border-white hover:bg-[#1A1A1A] hover:text-white hover:border-[#1A1A1A] transition-colors whitespace-nowrap"
        >
          → Live Games
        </Link>
      </header>
      <div>
        <div className="grid grid-cols-[2rem_minmax(0,1fr)_3.5rem_4rem_minmax(0,2fr)] gap-3 px-4 py-2 bg-[#F5F0E8] text-[10px] uppercase tracking-widest text-gray-500 font-bold border-b-2 border-[#1A1A1A]">
          <span>#</span>
          <span>Team</span>
          <span className="text-right">Today</span>
          <span className="text-right">Players</span>
          <span>Games</span>
        </div>
        {rows.map((team, idx) => {
          const roster = rosterByTeam.get(team.teamId) ?? new Set<string>();
          const games = pulse.gamesToday.filter(
            (g) => roster.has(g.homeTeam) || roster.has(g.awayTeam),
          );
          return (
            <div
              key={team.teamId}
              className={`grid grid-cols-[2rem_minmax(0,1fr)_3.5rem_4rem_minmax(0,2fr)] gap-3 px-4 py-3.5 text-sm items-center border-b border-gray-200 last:border-b-0 ${
                team.isMyTeam ? "bg-[#FACC15]/10 border-l-4 border-[#FACC15]" : ""
              }`}
            >
              <span className={`font-bold ${team.isMyTeam ? "" : "text-gray-400"}`}>
                {idx + 1}
              </span>
              <Link
                to={`${lp}/teams/${team.teamId}`}
                className={`truncate hover:text-[#2563EB] ${
                  team.isMyTeam ? "font-bold" : "font-bold"
                }`}
              >
                {team.teamName}
              </Link>
              <span
                className={`text-right tabular-nums font-bold ${
                  team.isMyTeam ? "text-[#2563EB]" : ""
                }`}
              >
                {team.pointsToday}
              </span>
              <span className="text-right tabular-nums text-gray-500">
                {team.playersActiveToday}
              </span>
              <GamesCell games={games} roster={roster} />
            </div>
          );
        })}
      </div>
    </section>
  );
}

function GamesCell({
  games,
  roster,
}: {
  games: GameMatchup[];
  roster: Set<string>;
}) {
  if (games.length === 0) {
    return <span className="text-xs text-gray-400">—</span>;
  }
  return (
    <div className="flex flex-wrap gap-x-2 gap-y-0.5 text-xs text-gray-600 tabular-nums">
      {games.map((g, i) => (
        <span key={i} className="whitespace-nowrap">
          {roster.has(g.awayTeam) ? (
            <strong className="text-[#1A1A1A]">{g.awayTeam}</strong>
          ) : (
            <span>{g.awayTeam}</span>
          )}
          <span className="text-gray-400">–</span>
          {roster.has(g.homeTeam) ? (
            <strong className="text-[#1A1A1A]">{g.homeTeam}</strong>
          ) : (
            <span>{g.homeTeam}</span>
          )}
        </span>
      ))}
    </div>
  );
}

/**
 * Build `{teamId -> Set(nhl_team_abbrev)}` from the series-forecast
 * cells. Every player on a fantasy roster carries its `nhlTeam`
 * there, so the derived set is exactly "who this team rosters."
 */
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
