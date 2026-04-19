import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import type { GameMatchup } from "@/features/pulse/types";

export interface LiveRankingRow {
  rank: number;
  teamId: number;
  teamName: string;
  pointsToday: number;
  playersActiveToday: number;
  /** Today's NHL matchups this fantasy team has a stake in, with
   *  `roster` carrying the NHL abbrevs the team actually owns on
   *  either side. Precomputed so the column render can stay pure. */
  games: GameMatchup[];
  roster: Set<string>;
  isMyTeam: boolean;
}

export const useLiveRankingsColumns = () => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return [
    {
      key: "rank",
      header: "Rank",
      sortable: false,
    },
    {
      key: "teamName",
      header: "Team",
      className: "font-medium",
      sortable: false,
      render: (value: string, team: LiveRankingRow) => (
        <Link
          to={`${lp}/teams/${team.teamId}`}
          className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
        >
          {value}
        </Link>
      ),
    },
    {
      key: "pointsToday",
      header: "Today",
      className: "font-bold",
      sortable: false,
    },
    {
      key: "playersActiveToday",
      header: "Players",
      sortable: false,
    },
    {
      key: "games",
      header: "Games",
      sortable: false,
      render: (_: unknown, team: LiveRankingRow) => {
        if (team.games.length === 0) {
          return <span className="text-xs text-gray-400">—</span>;
        }
        return (
          <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-gray-500 tabular-nums">
            {team.games.map((g, i) => (
              <span key={i} className="whitespace-nowrap">
                {team.roster.has(g.awayTeam) ? (
                  <strong className="text-[#1A1A1A]">{g.awayTeam}</strong>
                ) : (
                  <span>{g.awayTeam}</span>
                )}
                <span className="text-gray-400">–</span>
                {team.roster.has(g.homeTeam) ? (
                  <strong className="text-[#1A1A1A]">{g.homeTeam}</strong>
                ) : (
                  <span>{g.homeTeam}</span>
                )}
              </span>
            ))}
          </div>
        );
      },
    },
  ];
};
