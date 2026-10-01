import { useMemo } from "react";
import RankingTable from "@/components/common/RankingTable";
import {
  toTopSkaterRows,
  useTopSkatersColumns,
  type TopSkaterRow,
} from "@/components/rankingsPageTableColumns/topSkatersColumns";
import { TopSkater } from "@/types/skaters";
import { usePlayoffsData } from "@/features/rankings";

interface TopSkatersTableProps {
  skaters: TopSkater[];
  isLoading: boolean;
}

const TopSkatersTable = ({ skaters, isLoading }: TopSkatersTableProps) => {
  const { isTeamInPlayoffs } = usePlayoffsData();
  const columns = useTopSkatersColumns();
  const rows = useMemo(() => toTopSkaterRows(skaters), [skaters]);

  return (
    <RankingTable<TopSkaterRow>
      data={rows}
      columns={columns}
      initialSortKey="points"
      showRankColors={false}
      stickyHeader
      isLoading={isLoading}
      emptyMessage="No skaters found matching your criteria."
      rowClassName={(row) =>
        isTeamInPlayoffs(row.teamAbbrev) ? "" : "opacity-25"
      }
    />
  );
};

export default TopSkatersTable;
