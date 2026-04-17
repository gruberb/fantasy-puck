import { NHLTeamBet } from "@/types/fantasyTeams";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { usePlayoffsData } from "@/features/rankings";
import RankingTable from "@/components/common/RankingTable";
import { APP_CONFIG } from "@/config";

interface TeamBetsTableProps {
  teamBets: NHLTeamBet[];
}

export default function TeamBetsTable({ teamBets }: TeamBetsTableProps) {
  const { isTeamInPlayoffs } = usePlayoffsData();

  if (teamBets.length === 0) {
    return null;
  }

  const sortedTeamBets = [...teamBets].sort(
    (a, b) => b.numPlayers - a.numPlayers,
  );

  // Add a rank property to the team bets so the RankingTable can use it
  const rankedTeamBets = [...sortedTeamBets].map((bet, index) => ({
    ...bet,
    rank: index + 1,
  }));

  // Define columns for the RankingTable
  const columns = [
    {
      key: "nhlTeamName",
      header: "NHL Team",
      render: (_value: string, bet: NHLTeamBet & { rank: number }) => {
        const isInPlayoffs = isTeamInPlayoffs(bet.nhlTeam);
        return (
          <div className={`${!isInPlayoffs ? "opacity-25" : ""}`}>
            <a
              href={`https://www.nhl.com/${getNHLTeamUrlSlug(bet.nhlTeam)}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-gray-900 hover:text-[#2563EB] hover:underline flex items-center font-medium"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="flex items-center">
                {bet.teamLogo ? (
                  <img
                    src={bet.teamLogo}
                    alt={`${bet.nhlTeam} logo`}
                    className="h-6 w-6 mr-2"
                  />
                ) : null}
                <span>{bet.nhlTeamName}</span>
              </div>
            </a>
          </div>
        );
      },
    },
    {
      key: "numPlayers",
      header: "Number of Skaters",
      className: "text-center",
      sortable: true,
      render: (value: number, bet: NHLTeamBet) => {
        const isInPlayoffs = isTeamInPlayoffs(bet.nhlTeam);
        return (
          <div className={`text-center ${!isInPlayoffs ? "opacity-25" : ""}`}>
            {value}
          </div>
        );
      },
    },
  ];

  return (
    <section className="card">
      <div className="overflow-x-auto">
        <RankingTable
          data={rankedTeamBets}
          columns={columns}
          keyField="nhlTeam"
          rankField="rank"
          initialSortKey="numPlayers"
          initialSortDirection="desc"
          showRankColors={false} // Don't show rank colors
          className="bg-transparent shadow-none border-0"
          title="Skaters from NHL Teams"
          dateBadge={APP_CONFIG.SEASON_LABEL}
        />
      </div>
    </section>
  );
}
