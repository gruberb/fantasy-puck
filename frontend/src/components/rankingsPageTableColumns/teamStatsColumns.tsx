import { nhlPlayerProfileUrl, nhlTeamUrl } from "@/utils/nhlTeams";
import { Column } from "@/components/common/RankingTable/types";
import { TeamStats } from "@/types/teamStats";
import { useState } from "react";
import { Link } from "react-router-dom";
import DatesPopover from "@/components/common/DatesPopover";
import { useLeague } from "@/contexts/use-league";

// A hook rather than a static array because the date popovers need open/close state.
export const useTeamStatsColumns = () => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const [openTooltip, setOpenTooltip] = useState<{
    type: string;
    teamId: number;
  } | null>(null);

  const closeTooltip = () => setOpenTooltip(null);

  const columns: Column<TeamStats>[] = [
    {
      key: "rank",
      header: "Rank",
      render: (_value: any, _row: any, index: number) => index + 1,
    },
    {
      key: "teamName",
      header: "Team",
      className: "font-medium",
      sortable: true,
      render: (value: string, team: TeamStats) => (
        <Link
          to={`${lp}/teams/${team.teamId}`}
          className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
        >
          {value}
        </Link>
      ),
    },
    {
      key: "totalPoints",
      header: "Total Points",
      className: "font-bold text-center",
    },
    {
      key: "dailyWins",
      header: "Daily Wins",
      className: "text-center",
      render: (value: number, team: TeamStats) => (
        <div className="relative inline-flex">
          <div className="px-3 py-1 rounded-none bg-green-100 text-green-800 font-medium flex items-center">
            {value}
            {team.winDates?.length > 0 && (
              <button
                className="ml-1 bg-white rounded-none w-4 h-4 flex items-center justify-center text-xs text-green-800 border border-green-800"
                onClick={(e) => {
                  e.stopPropagation();
                  setOpenTooltip(
                    openTooltip?.teamId === team.teamId &&
                      openTooltip?.type === "wins"
                      ? null
                      : { type: "wins", teamId: team.teamId },
                  );
                }}
              >
                i
              </button>
            )}
          </div>

          {openTooltip?.teamId === team.teamId &&
            openTooltip?.type === "wins" && (
              <DatesPopover
                dates={team.winDates}
                title="Daily Win Dates"
                isOpen={true}
                onClose={closeTooltip}
              />
            )}
        </div>
      ),
    },
    {
      key: "dailyTopThree",
      header: "Daily Top 3",
      className: "text-center",
      render: (value: number, team: TeamStats) => (
        <div className="relative inline-flex">
          <div className="px-3 py-1 rounded-none bg-blue-100 text-blue-800 font-medium flex items-center">
            {value}
            {team.topThreeDates?.length > 0 && (
              <button
                className="ml-1 bg-white rounded-none w-4 h-4 flex items-center justify-center text-xs text-blue-800 border border-blue-800"
                onClick={(e) => {
                  e.stopPropagation();
                  setOpenTooltip(
                    openTooltip?.teamId === team.teamId &&
                      openTooltip?.type === "top3"
                      ? null
                      : { type: "top3", teamId: team.teamId },
                  );
                }}
              >
                i
              </button>
            )}
          </div>

          {openTooltip?.teamId === team.teamId &&
            openTooltip?.type === "top3" && (
              <DatesPopover
                dates={team.topThreeDates}
                title="Daily Top 3 Dates"
                isOpen={true}
                onClose={closeTooltip}
              />
            )}
        </div>
      ),
    },
    {
      key: "topPlayers",
      header: "Top Player",
      render: (topPlayers: any[]) => {
        if (!topPlayers || topPlayers.length === 0) {
          return <span className="text-gray-400">None</span>;
        }

        const player = topPlayers[0];

        return (
          <div className="flex items-center space-x-2 w-[10rem]">
            {player.imageUrl ? (
              <img
                src={player.imageUrl}
                alt={player.name}
                className="w-8 h-8 rounded-none"
              />
            ) : (
              <div className="w-8 h-8 bg-gray-200 rounded-none flex items-center justify-center">
                <span className="text-xs font-medium">
                  {player.name.substring(0, 2).toUpperCase()}
                </span>
              </div>
            )}
            <div>
              <a
                href={nhlPlayerProfileUrl(player.nhlId)}
                target="_blank"
                rel="noopener noreferrer"
                className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
              >
                {player.name}
              </a>
              <div className="text-xs text-gray-500">
                {player.position} •
                <a
                  href={nhlTeamUrl(player.nhlTeam)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-[#2563EB] ml-1"
                >
                  {player.nhlTeam}
                </a>
                • {player.points} pts
              </div>
            </div>
          </div>
        );
      },
    },
    {
      key: "topNhlTeams",
      header: "Top NHL Team",
      render: (topTeams: any[]) => {
        if (!topTeams || topTeams.length === 0) {
          return <span className="text-gray-400">None</span>;
        }

        const team = topTeams[0];

        return (
          <div className="flex items-center space-x-2 w-[12rem]">
            {team.teamLogo ? (
              <img src={team.teamLogo} alt={team.nhlTeam} className="w-8 h-8" />
            ) : (
              <div className="w-8 h-8 bg-gray-200 rounded-none flex items-center justify-center">
                <span className="text-xs font-medium">{team.nhlTeam}</span>
              </div>
            )}
            <div>
              <a
                href={nhlTeamUrl(team.nhlTeam)}
                target="_blank"
                rel="noopener noreferrer"
                className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
              >
                {team.teamName}
              </a>
              <div className="text-xs text-gray-500">{team.points} pts</div>
            </div>
          </div>
        );
      },
    },
  ];

  return columns;
};
