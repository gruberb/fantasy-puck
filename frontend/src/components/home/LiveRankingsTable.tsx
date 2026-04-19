import { Link } from "react-router-dom";
import { usePulse } from "@/features/pulse";
import { useLeague } from "@/contexts/LeagueContext";

/**
 * Appears at the top of the dashboard only while games are in flight
 * (`hasLiveGames` from Pulse). Lists every league team with at least
 * one rostered skater in tonight's NHL slate, sorted by today's live
 * points (from `v_daily_fantasy_totals`, same source as the Pulse
 * League Live Board). First puck drop through the final whistle of
 * the last game on the slate.
 *
 * Reuses `usePulse` rather than adding a parallel endpoint so the
 * auto-refresh behaviour and per-team points stay in one place.
 */
export function LiveRankingsTable() {
  const { pulse, isLoading } = usePulse();
  const { activeLeagueId } = useLeague();

  // Hide when pulse hasn't loaded yet, when there are no live games,
  // or when we can't compute the board — staying quiet in those
  // states beats flashing an empty container across the whole page.
  if (isLoading || !pulse || !pulse.hasLiveGames) return null;

  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const today = new Date().toISOString().slice(0, 10);
  const active = pulse.leagueBoard
    .filter((t) => t.playersActiveToday > 0)
    .sort((a, b) => {
      if (b.pointsToday !== a.pointsToday) return b.pointsToday - a.pointsToday;
      return b.totalPoints - a.totalPoints;
    });

  if (active.length === 0) return null;

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
      <div className="divide-y divide-gray-100">
        <div className="grid grid-cols-[1.5rem_minmax(0,1fr)_3.5rem_3.5rem] sm:grid-cols-[2rem_minmax(0,1fr)_4.5rem_4.5rem] gap-2 px-3 sm:px-4 py-2 text-[10px] uppercase tracking-wider text-gray-400 font-bold">
          <span>#</span>
          <span>Team</span>
          <span className="text-right">Active</span>
          <span className="text-right">Today</span>
        </div>
        {active.map((team, idx) => (
          <div
            key={team.teamId}
            className={`grid grid-cols-[1.5rem_minmax(0,1fr)_3.5rem_3.5rem] sm:grid-cols-[2rem_minmax(0,1fr)_4.5rem_4.5rem] gap-2 px-3 sm:px-4 py-2.5 text-sm items-center ${
              team.isMyTeam ? "bg-[#FACC15]/10 border-l-4 border-[#FACC15]" : ""
            }`}
          >
            <span className={`font-bold ${team.isMyTeam ? "" : "text-gray-400"}`}>
              {idx + 1}
            </span>
            <Link
              to={`${lp}/teams/${team.teamId}`}
              className={`truncate hover:text-[#2563EB] ${
                team.isMyTeam ? "font-bold" : "font-medium"
              }`}
            >
              {team.teamName}
            </Link>
            <span className="text-right tabular-nums text-gray-500">
              {team.playersActiveToday}
            </span>
            <span
              className={`text-right tabular-nums ${
                team.isMyTeam ? "font-bold text-[#2563EB]" : "font-bold"
              }`}
            >
              {team.pointsToday}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}
