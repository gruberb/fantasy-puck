import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";

interface LiveRankingRow {
  teamId: number;
  teamName: string;
  playersActiveToday: number;
  pointsToday: number;
  totalPoints: number;
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
      header: "Active",
      sortable: false,
    },
    {
      key: "totalPoints",
      header: "Total",
      sortable: false,
    },
  ];
};
