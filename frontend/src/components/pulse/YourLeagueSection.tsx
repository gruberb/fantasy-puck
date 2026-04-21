import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import type { LeagueOutlook, LeagueOutlookEntry } from "@/features/pulse";

interface Props {
  data: LeagueOutlook;
}

/**
 * "Your League" block on Pulse: leader, points spread, top-3
 * projected finishers from the Monte Carlo. Descriptive only — no
 * action items. Hidden when `leagueOutlook` is null (cache cold,
 * regular-season mode, or no league totals yet).
 */
export default function YourLeagueSection({ data }: Props) {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const max = Math.max(...data.pointsDistribution, 1);

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[#1A1A1A] text-white px-6 py-3">
        <h2 className="font-extrabold uppercase tracking-wider text-sm">
          Your League
        </h2>
      </header>
      <div className="p-6 space-y-5">
        <div className="flex flex-wrap items-baseline gap-x-6 gap-y-1">
          <span className="text-xs font-bold uppercase tracking-wider text-gray-500">
            Leader
          </span>
          <Link
            to={`${lp}/teams/${data.leaderTeamId}`}
            className="font-bold text-lg hover:text-[#2563EB]"
          >
            {data.leaderName}
          </Link>
          <span className="tabular-nums font-bold">{data.leaderPoints} pts</span>
          <span className="text-xs text-gray-500">
            Median {data.medianPoints.toFixed(1)} · {data.totalTeams} teams
          </span>
        </div>

        <div>
          <div className="text-[10px] font-bold uppercase tracking-wider text-gray-500 mb-2">
            Points distribution
          </div>
          <div className="flex items-end gap-1 h-10">
            {data.pointsDistribution.map((pts, i) => {
              const h = Math.max(2, Math.round((pts / max) * 40));
              return (
                <div
                  key={i}
                  className="flex-1 bg-[#1A1A1A]"
                  style={{ height: `${h}px` }}
                  title={`${pts} pts`}
                />
              );
            })}
          </div>
        </div>

        {data.topThree.length > 0 && (
          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-gray-500 mb-2">
              Top 3 expected finish
            </div>
            <ol className="divide-y divide-gray-100 border-2 border-[#1A1A1A]">
              {data.topThree.map((e, i) => (
                <TopThreeRow key={e.teamId} rank={i + 1} entry={e} lp={lp} />
              ))}
            </ol>
          </div>
        )}
      </div>
    </section>
  );
}

function TopThreeRow({
  rank,
  entry,
  lp,
}: {
  rank: number;
  entry: LeagueOutlookEntry;
  lp: string;
}) {
  const why = buildWhy(entry);
  return (
    <li className="grid grid-cols-[2rem_minmax(0,1fr)_auto] gap-3 px-3 py-2 items-center">
      <span className="font-bold text-lg text-gray-500">{rank}</span>
      <div className="min-w-0">
        <Link
          to={`${lp}/teams/${entry.teamId}`}
          className="font-bold hover:text-[#2563EB] truncate block"
        >
          {entry.teamName}
        </Link>
        {why && <div className="text-xs text-gray-600 truncate">{why}</div>}
      </div>
      <div className="text-right whitespace-nowrap">
        <div className="tabular-nums font-bold text-sm">
          proj {entry.projectedFinalMean.toFixed(1)}
        </div>
        <div className="text-[10px] text-gray-500 uppercase tracking-wider">
          win {(entry.winProb * 100).toFixed(0)}% · top3 {(entry.top3Prob * 100).toFixed(0)}%
        </div>
      </div>
    </li>
  );
}

function buildWhy(entry: LeagueOutlookEntry): string | null {
  const stack = entry.topStack;
  if (!stack) {
    return `${entry.currentPoints} pts so far`;
  }
  const cup = stack.cupWinProb > 0 ? ` · cup ${(stack.cupWinProb * 100).toFixed(0)}%` : "";
  return `${entry.currentPoints} pts · top stack ${stack.nhlTeam} ×${stack.rostered}${cup}`;
}
