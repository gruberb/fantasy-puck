import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/use-league";
import type { Column } from "@/components/common/RankingTable/types";
import type { Ranking } from "@/types/rankings";

export const useSeasonRankingsColumns = (): Column<Ranking>[] => {
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
      render: (value: string, team: Ranking) => (
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
  ];
};
