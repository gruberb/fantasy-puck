import { Link } from "react-router-dom";
import type { Column } from "@/components/common/RankingTable/types";
import type { TopSkater } from "@/types/skaters";
import type { FantasyTeam } from "@/types/fantasyTeams";
import { nhlPlayerProfileUrl, nhlTeamUrl } from "@/utils/nhlTeams";
import { formatToi } from "@/utils/format";
import { useLeague } from "@/contexts/use-league";

/**
 * `TopSkater` flattened so every sortable stat is a top-level key, which
 * is what `RankingTable` sorts on. `sortName` keeps the "Last, First"
 * ordering of the Skater column.
 */
export interface TopSkaterRow {
  id: number;
  firstName: string;
  lastName: string;
  sortName: string;
  teamAbbrev: string;
  position: string;
  points: number;
  goals: number;
  assists: number;
  plusMinus?: number;
  penaltyMins?: number;
  toi?: number;
  fantasyTeam?: FantasyTeam;
}

export function toTopSkaterRows(skaters: TopSkater[]): TopSkaterRow[] {
  return skaters.map((s) => ({
    id: s.id,
    firstName: s.firstName,
    lastName: s.lastName,
    sortName: `${s.lastName}, ${s.firstName}`,
    teamAbbrev: s.teamAbbrev,
    position: s.position,
    points: s.stats.points,
    goals: s.stats.goals,
    assists: s.stats.assists,
    plusMinus: s.stats.plusMinus,
    penaltyMins: s.stats.penaltyMins,
    toi: s.stats.toi,
    fantasyTeam: s.fantasyTeam,
  }));
}

/** Column set for the /skaters table. */
export function useTopSkatersColumns(): Column<TopSkaterRow>[] {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return [
    { key: "rank", header: "#" },
    {
      key: "sortName",
      header: "Skater",
      sortable: true,
      render: (_value, row) => (
        <a
          href={nhlPlayerProfileUrl(row.id)}
          target="_blank"
          rel="noopener noreferrer"
          className="font-medium text-gray-900 hover:underline block whitespace-nowrap"
        >
          {row.firstName} {row.lastName}
        </a>
      ),
    },
    {
      key: "teamAbbrev",
      header: "Team",
      render: (value: string) => (
        <a
          href={nhlTeamUrl(value)}
          target="_blank"
          rel="noopener noreferrer"
          className="text-gray-900 hover:underline"
        >
          {value}
        </a>
      ),
    },
    { key: "position", header: "Pos" },
    {
      key: "points",
      header: "Points",
      sortable: true,
      className: "bg-sky-200/50 font-bold",
      render: (value: number | undefined) => value ?? "-",
    },
    {
      key: "goals",
      header: "Goals",
      sortable: true,
      className: "bg-sky-100/75",
      render: (value: number | undefined) => value ?? "-",
    },
    {
      key: "assists",
      header: "Assists",
      sortable: true,
      className: "bg-sky-100/75",
      render: (value: number | undefined) => value ?? "-",
    },
    {
      key: "plusMinus",
      header: "+/-",
      sortable: true,
      render: (value: number | undefined) =>
        value == null ? (
          "-"
        ) : (
          <span
            className={
              value > 0 ? "text-green-600" : value < 0 ? "text-red-600" : ""
            }
          >
            {value > 0 ? "+" : ""}
            {value}
          </span>
        ),
    },
    {
      key: "penaltyMins",
      header: "PIM",
      sortable: true,
      render: (value: number | undefined) => value ?? 0,
    },
    {
      key: "toi",
      header: "TOI",
      sortable: true,
      render: (value: number | undefined) =>
        value == null ? "-" : formatToi(value),
    },
    {
      key: "fantasyTeam",
      header: "Fantasy",
      className: "whitespace-nowrap",
      render: (value: FantasyTeam | undefined) =>
        value ? (
          <Link
            to={`${lp}/teams/${value.teamId}`}
            className="text-[#2563EB] hover:underline"
          >
            {value.teamName}
          </Link>
        ) : (
          <span className="text-gray-500">-</span>
        ),
    },
  ];
}
