import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";

export const useDailyRankingsColumns = () => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return [
    {
      key: "rank",
      header: "Rank",
      sortable: true,
    },
    {
      key: "teamName",
      header: "Team",
      className: "font-medium",
      sortable: true,
      render: (value: string, team: any) => (
        <Link
          to={`${lp}/teams/${team.teamId}`}
          className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
        >
          {value}
        </Link>
      ),
    },
    {
      key: "dailyPoints",
      header: "Points",
      className: "font-bold whitespace-nowrap",
      sortable: true,
    },
    {
      key: "dailyGoals",
      header: "Goals",
      className: "whitespace-nowrap",
      sortable: true,
    },
    {
      key: "dailyAssists",
      header: "Assists",
      className: "whitespace-nowrap",
      sortable: true,
    },
    {
      key: "playerHighlights",
      header: "Top Skater",
      render: (playerHighlights: any[]) => {
        if (!playerHighlights || playerHighlights.length === 0) {
          return <span className="text-gray-400">None</span>;
        }

        const player = playerHighlights[0];

        return (
          <div className="flex w-[11rem]">
            {player.imageUrl ? (
              <img
                src={player.imageUrl}
                alt={player.playerName}
                className="w-8 h-8 rounded-none mr-2"
              />
            ) : (
              <div className="w-8 h-8 bg-[#2563EB]/10">
                <span className="text-xs font-medium text-[#2563EB] whitespace-nowrap">
                  {player.playerName.substring(0, 2).toUpperCase()}
                </span>
              </div>
            )}
            <div>
              {player.nhlId ? (
                <a
                  href={`https://www.nhl.com/player/${player.nhlId}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-gray-900 hover:text-[#2563EB] hover:underline"
                  onClick={(e) => e.stopPropagation()}
                >
                  <span>{player.playerName}</span>
                </a>
              ) : (
                <div className="text-gray-900">
                  <span>{player.playerName}</span>
                </div>
              )}
              <div className="text-xs text-gray-500">
                <span>
                  {player.nhlTeam ? (
                    <a
                      href={`https://www.nhl.com/${getNHLTeamUrlSlug(player.nhlTeam)}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="hover:text-[#2563EB] hover:underline"
                    >
                      {player.nhlTeam}
                    </a>
                  ) : (
                    player.nhlTeam
                  )}{" "}
                </span>
                • {player.points} pts
              </div>
            </div>
          </div>
        );
      },
    },
  ];
};
