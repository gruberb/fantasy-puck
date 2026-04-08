import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { useLeague } from "@/contexts/LeagueContext";

export const useSleepersRankingsColumns = () => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return [
    {
      key: "rank",
      header: "Rank",
      // Use index as rank
      render: (_value: any, _row: any, index: number) => index + 1,
    },
    {
      key: "name",
      header: "Player",
      render: (value: string, player: any) => (
        <div className="flex space-x-2  w-[10rem]">
          {player.imageUrl ? (
            <img
              src={player.imageUrl}
              alt={value}
              className="w-8 h-8 rounded-none"
            />
          ) : (
            <div className="w-8 h-8 bg-gray-200 rounded-none">
              <span className="text-xs font-medium">
                {value.substring(0, 2).toUpperCase()}
              </span>
            </div>
          )}
          <div>
            <a
              href={`https://www.nhl.com/player/${player.nhlId}`}
              target="_blank"
              rel="noopener noreferrer"
              className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
            >
              {value}
            </a>
            <div className="text-xs text-gray-500">
              {player.position} •
              <a
                href={`https://www.nhl.com/${getNHLTeamUrlSlug(player.nhlTeam)}`}
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-[#2563EB] ml-1"
              >
                {player.nhlTeam}
              </a>
            </div>
          </div>
        </div>
      ),
      className: "w-64",
      sortable: true,
    },
    {
      key: "totalPoints",
      header: "Points",
      className: "font-bold",
      sortable: true,
    },
    {
      key: "goals",
      header: "Goals",
      sortable: true,
    },
    {
      key: "assists",
      header: "Assists",
      sortable: true,
    },
    {
      key: "plusMinus",
      header: "+/-",
      render: (value: number | null) => (
        <span
          className={
            value
              ? value > 0
                ? "text-green-600"
                : value < 0
                  ? "text-red-600"
                  : ""
              : ""
          }
        >
          {value === null ? "-" : (value > 0 ? "+" : "") + value}
        </span>
      ),
      sortable: true,
    },
    {
      key: "fantasyTeam",
      header: "Fantasy Team",
      render: (value: string | null, player: any) => (
        <div>
          {value ? (
            <a
              href={`${lp}/teams/${player.fantasyTeamId}`}
              className="text-[#2563EB] hover:underline"
            >
              {value}
            </a>
          ) : (
            <span className="text-gray-400">?</span>
          )}
        </div>
      ),
      sortable: true,
    },
  ];
};
