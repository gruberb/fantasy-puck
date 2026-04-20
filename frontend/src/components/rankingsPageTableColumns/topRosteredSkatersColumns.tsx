import type { Column } from "@/components/common/RankingTable/types";
import type { RosteredSkaterRow } from "@/types/leagueStats";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { useLeague } from "@/contexts/LeagueContext";
import { Link } from "react-router-dom";

/**
 * Column set for the "Top 10 Rostered Skaters" table on /stats:
 * league-wide skater leaderboard by playoff fantasy points, with the
 * fantasy team that rosters each skater. Same `RankingTable` contract
 * as the other tables on the page.
 */
export function useTopRosteredSkatersColumns(): Column[] {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return [
    { key: "rank", header: "#", sortable: false },
    {
      key: "name",
      header: "Skater",
      className: "font-medium",
      sortable: true,
      render: (_value: unknown, row: Record<string, unknown>) => {
        const skater = row as unknown as RosteredSkaterRow;
        const initials = skater.name
          .split(" ")
          .map((s) => s[0])
          .join("")
          .slice(0, 2)
          .toUpperCase();
        return (
          <div className="flex items-center space-x-2 w-[12rem]">
            {skater.photo ? (
              <img
                src={skater.photo}
                alt={skater.name}
                className="w-8 h-8 rounded-none"
              />
            ) : (
              <div className="w-8 h-8 bg-gray-200 flex items-center justify-center">
                <span className="text-xs font-medium">{initials}</span>
              </div>
            )}
            <a
              href={`https://www.nhl.com/player/${skater.nhlId}`}
              target="_blank"
              rel="noopener noreferrer"
              className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
            >
              {skater.name}
            </a>
          </div>
        );
      },
    },
    {
      key: "nhlTeam",
      header: "NHL Team",
      sortable: true,
      render: (_value: unknown, row: Record<string, unknown>) => {
        const skater = row as unknown as RosteredSkaterRow;
        return (
          <div className="flex items-center space-x-2">
            {skater.teamLogo ? (
              <img
                src={skater.teamLogo}
                alt={skater.nhlTeam}
                className="w-6 h-6"
              />
            ) : (
              <div className="w-6 h-6 bg-gray-200 flex items-center justify-center">
                <span className="text-[9px] font-medium">{skater.nhlTeam}</span>
              </div>
            )}
            <a
              href={`https://www.nhl.com/${getNHLTeamUrlSlug(skater.nhlTeam)}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-gray-600 hover:text-[#2563EB]"
            >
              {skater.nhlTeam}
            </a>
          </div>
        );
      },
    },
    {
      key: "playoffPoints",
      header: "Playoff Pts",
      className: "font-bold whitespace-nowrap",
      sortable: true,
    },
    {
      key: "fantasyTeamName",
      header: "Rostered By",
      sortable: true,
      render: (value: unknown, row: Record<string, unknown>) => {
        const skater = row as unknown as RosteredSkaterRow;
        return (
          <Link
            to={`${lp}/teams/${skater.fantasyTeamId}`}
            className="font-bold text-[#1A1A1A] hover:text-[#2563EB]"
          >
            {value as string}
          </Link>
        );
      },
    },
  ];
}
