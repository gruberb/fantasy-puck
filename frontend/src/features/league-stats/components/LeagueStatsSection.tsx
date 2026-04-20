import { useLeagueStats } from "../hooks/use-league-stats";
import { NhlTeamRosterTable } from "./NhlTeamRosterTable";
import { TopRosteredSkatersTable } from "./TopRosteredSkatersTable";

interface Props {
  leagueId: string | null;
  myFantasyTeamId?: number | null;
}

/**
 * League-wide stats section for the /stats page: two tables powered
 * by a single /api/fantasy/league-stats read.
 *
 * 1. NHL teams we roster most - which playoff teams our rosters are
 *    concentrated in, how much those teams have scored, and who's
 *    leading each one.
 * 2. Top 10 rostered skaters - the league's own leaderboard of
 *    playoff fantasy points, with each player's fantasy-team owner.
 */
export function LeagueStatsSection({ leagueId, myFantasyTeamId }: Props) {
  const { data, isLoading, isError, refetch } = useLeagueStats(leagueId);

  return (
    <section className="bg-white border-2 border-[#1A1A1A]">
      <header
        className="px-6 py-3 border-b-2 border-[#1A1A1A]"
        style={{ backgroundColor: "#2563EB" }}
      >
        <h2 className="font-extrabold text-white uppercase tracking-wider text-sm">
          League Stats
        </h2>
      </header>

      <div className="p-6 space-y-8">
        {isLoading && (
          <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
            Loading league stats…
          </p>
        )}
        {isError && (
          <div className="flex items-center gap-3">
            <p className="text-xs text-[var(--color-error)] flex-1">
              Couldn't load league stats.
            </p>
            <button
              onClick={() => refetch()}
              className="text-[10px] uppercase tracking-wider font-bold border-2 border-[#1A1A1A] px-2 py-1 hover:bg-[#1A1A1A] hover:text-white"
            >
              Retry
            </button>
          </div>
        )}
        {data && (
          <>
            <div className="space-y-3">
              <h3 className="font-extrabold uppercase tracking-wider text-xs text-[#1A1A1A]">
                NHL teams we roster
              </h3>
              <NhlTeamRosterTable rows={data.nhlTeamsRostered} />
            </div>

            <div className="space-y-3">
              <h3 className="font-extrabold uppercase tracking-wider text-xs text-[#1A1A1A]">
                Top 10 rostered skaters
              </h3>
              <TopRosteredSkatersTable
                rows={data.topRosteredSkaters}
                myFantasyTeamId={myFantasyTeamId}
              />
            </div>
          </>
        )}
      </div>
    </section>
  );
}
