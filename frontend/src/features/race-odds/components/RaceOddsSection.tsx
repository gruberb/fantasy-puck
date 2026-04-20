import { useRaceOdds } from "../hooks/use-race-odds";

import { FantasyChampionBoard } from "./FantasyChampionBoard";
import { LeagueRaceTable } from "./LeagueRaceTable";
import { RivalryCard } from "./RivalryCard";

interface RaceOddsSectionProps {
  /**
   * When provided (league mode), enables the rivalry card against the
   * caller's closest rival. Leave undefined for the global champion view.
   */
  myTeamId?: number | null;
}

/**
 * Insights-page section wrapping the full race-odds experience:
 * - League mode: league-race table + rivalry card.
 * - Champion mode: top-20 Fantasy Champion leaderboard.
 *
 * Renders its own header so pages can drop it in as a peer of other
 * InsightCard blocks without duplicating the border/accent chrome.
 */
export function RaceOddsSection({ myTeamId }: RaceOddsSectionProps) {
  const { data, isLoading, isError, refetch } = useRaceOdds({ myTeamId });

  const isLeague = data?.mode === "league";
  const title = isLeague ? "Race Odds" : "Fantasy Champion";
  const blurb = isLeague
    ? `Monte Carlo, ${data?.trials.toLocaleString() ?? ""} bracket trials. Projected is each team's mean final total with the p10–p90 likely range; Win % and Top-3 are the share of trials finishing first or in the top three.`
    : `Top NHL skaters by projected playoff fantasy points across ${data?.trials.toLocaleString() ?? ""} bracket simulations.`;

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header
        className="px-6 py-3 border-b-2 border-[#1A1A1A]"
        style={{ backgroundColor: "#2563EB" }}
      >
        <h2 className="font-extrabold text-white uppercase tracking-wider text-sm">
          {title}
        </h2>
      </header>
      <div className="p-6 space-y-4">
        {isLoading && (
          <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
            Running simulation…
          </p>
        )}
        {isError && (
          <div className="flex items-center gap-3">
            <p className="text-xs text-[var(--color-error)] flex-1">
              Couldn't load race odds — the simulation service may be warming up.
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
            <p className="text-[11px] text-[var(--color-ink-muted)] leading-relaxed">
              {blurb}
            </p>
            {/* In 2-team leagues the rivalry card is identical data to the
                race board below — showing both is redundant. Hide it once
                there are only two teams and let the board speak. */}
            {data.rivalry && data.teamOdds.length > 2 && (
              <RivalryCard rivalry={data.rivalry} />
            )}
            {isLeague ? (
              <LeagueRaceTable
                teams={data.teamOdds}
                myTeamId={myTeamId}
                generatedAt={data.generatedAt}
              />
            ) : (
              <FantasyChampionBoard players={data.championLeaderboard} />
            )}
          </>
        )}
      </div>
    </section>
  );
}
