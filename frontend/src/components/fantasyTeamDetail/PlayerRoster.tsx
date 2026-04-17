import { SkaterStats } from "@/types/skaters";
import { usePlayoffsData } from "@/features/rankings";
import RankingTable from "@/components/common/RankingTable";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { APP_CONFIG } from "@/config";

interface PlayerRosterProps {
  players: SkaterStats[];
}

export default function PlayerRoster({ players }: PlayerRosterProps) {
  const { isTeamInPlayoffs } = usePlayoffsData();

  // Define columns for the RankingTable
  const columns = [
    {
      key: "name",
      header: "Skater",
      render: (value: string, player: SkaterStats) => {
        const isInPlayoffs = isTeamInPlayoffs(player.nhlTeam);
        return (
          <div
            className={`flex items-center ${!isInPlayoffs ? "opacity-25" : ""} w-[10rem]`}
          >
            {player.imageUrl ? (
              <img
                src={player.imageUrl}
                alt={value}
                className="h-10 w-10 rounded-none"
              />
            ) : (
              <div className="h-10 w-10 rounded-none bg-[#2563EB]/10 flex items-center justify-center">
                <span className="text-xs font-medium text-[#2563EB]">
                  {value.substring(0, 2).toUpperCase()}
                </span>
              </div>
            )}
            <div>
              <div className="ml-4">
                <a
                  href={`https://www.nhl.com/player/${player.nhlId}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-gray-900 hover:text-[#2563EB] hover:underline flex items-center font-medium"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="text-sm text-gray-900">{value}</div>
                </a>
              </div>
              <div className="ml-4 flex items-center text-xs text-gray-500">
                {player.teamLogo && (
                  <img
                    src={player.teamLogo}
                    alt={player.nhlTeam || ""}
                    className="w-3 h-3 mr-1"
                  />
                )}
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
                  {player.position ? `• ${player.position}` : ""}
                </span>
              </div>
            </div>
          </div>
        );
      },
    },
    {
      key: "totalPoints",
      header: "Points",
      sortable: true,
      className: "font-semibold",
      render: (value: number, player: SkaterStats) => {
        const isInPlayoffs = isTeamInPlayoffs(player.nhlTeam);
        return (
          <div
            className={`text-sm h-7 font-semibold text-gray-900 ${!isInPlayoffs ? "opacity-25" : ""}`}
          >
            {value}
          </div>
        );
      },
    },
    {
      key: "goals",
      header: "Goals",
      sortable: true,
      render: (value: number, player: SkaterStats) => {
        const isInPlayoffs = isTeamInPlayoffs(player.nhlTeam);
        return (
          <div
            className={`text-sm h-7 text-gray-900 ${!isInPlayoffs ? "opacity-25" : ""}`}
          >
            {value}
          </div>
        );
      },
    },
    {
      key: "assists",
      header: "Assists",
      sortable: true,
      render: (value: number, player: SkaterStats) => {
        const isInPlayoffs = isTeamInPlayoffs(player.nhlTeam);
        return (
          <div
            className={`text-sm h-7 text-gray-900 ${!isInPlayoffs ? "opacity-25" : ""}`}
          >
            {value}
          </div>
        );
      },
    },
  ];

  // Check if we have players
  if (players.length === 0) {
    return (
      <section className="card">
        <h2 className="text-2xl font-bold mb-4">Player Roster</h2>
        <p className="text-gray-500">No skaters available for this team.</p>
      </section>
    );
  }

  return (
    <section className="card">
      <div className="overflow-x-auto">
        <RankingTable
          data={players}
          columns={columns}
          keyField="nhlId"
          initialSortKey="totalPoints"
          initialSortDirection="desc"
          showRankColors={false} // Don't show rank colors
          className="bg-transparent shadow-none border-0"
          title="Fantasy Team Roster"
          dateBadge={APP_CONFIG.SEASON_LABEL}
        />
      </div>
    </section>
  );
}
