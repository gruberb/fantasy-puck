import type { Column } from "@/components/common/RankingTable/types";
import type { NhlTeamRosterRow } from "@/types/leagueStats";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";

/**
 * Column set for the "NHL Teams" table on /stats — one row per NHL
 * team with at least one rostered fantasy player in the league. Same
 * `RankingTable` contract as the other page tables (Season Overview,
 * Playoff Stats, Daily Fantasy Scores) so every table on this page
 * shares one look.
 */
export function useNhlTeamRosterColumns(): Column[] {
  return [
    { key: "rank", header: "#", sortable: false },
    {
      key: "teamName",
      header: "NHL Team",
      className: "font-medium",
      sortable: true,
      render: (_value: unknown, row: Record<string, unknown>) => {
        const team = row as unknown as NhlTeamRosterRow;
        return (
          <div className="flex items-center space-x-2 w-[12rem]">
            {team.teamLogo ? (
              <img src={team.teamLogo} alt={team.nhlTeam} className="w-8 h-8" />
            ) : (
              <div className="w-8 h-8 bg-gray-200 flex items-center justify-center">
                <span className="text-xs font-medium">{team.nhlTeam}</span>
              </div>
            )}
            <div>
              <a
                href={`https://www.nhl.com/${getNHLTeamUrlSlug(team.nhlTeam)}`}
                target="_blank"
                rel="noopener noreferrer"
                className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
              >
                {team.teamName}
              </a>
              <div className="text-xs text-gray-500">{team.nhlTeam}</div>
            </div>
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
      key: "rosteredCount",
      header: "Rostered",
      className: "whitespace-nowrap",
      sortable: true,
    },
    {
      key: "topSkaterName",
      header: "Top Skater",
      sortable: false,
      render: (_value: unknown, row: Record<string, unknown>) => {
        const team = row as unknown as NhlTeamRosterRow;
        if (!team.topSkaterName) {
          return <span className="text-gray-400">—</span>;
        }
        const initials = team.topSkaterName
          .split(" ")
          .map((s) => s[0])
          .join("")
          .slice(0, 2)
          .toUpperCase();
        return (
          <div className="flex items-center space-x-2 w-[11rem]">
            {team.topSkaterPhoto ? (
              <img
                src={team.topSkaterPhoto}
                alt={team.topSkaterName}
                className="w-8 h-8 rounded-none"
              />
            ) : (
              <div className="w-8 h-8 bg-gray-200 flex items-center justify-center">
                <span className="text-xs font-medium">{initials}</span>
              </div>
            )}
            <div>
              <div className="font-bold text-base text-[#1A1A1A]">
                {team.topSkaterName}
              </div>
              <div className="text-xs text-gray-500">
                {team.topSkaterPoints ?? 0} pts
              </div>
            </div>
          </div>
        );
      },
    },
  ];
}
