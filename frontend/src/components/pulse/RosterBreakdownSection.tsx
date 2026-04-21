import RankingTable from "@/components/common/RankingTable";
import { useTeamBreakdownColumns } from "@/components/rankingsPageTableColumns/teamBreakdownColumns";
import type { SkaterStats } from "@/types/skaters";

interface Props {
  players: SkaterStats[];
}

/**
 * Full per-player breakdown table. Lives below "Your Read" as its own
 * card so the narrative block doesn't inherit the table's 14-column
 * horizontal scroll.
 */
export default function RosterBreakdownSection({ players }: Props) {
  const columns = useTeamBreakdownColumns();

  // Flatten breakdown fields onto each row so RankingTable's
  // `row[sortKey]` lookup works for every sortable column. The column
  // renderers still read from `row` and pick whichever nested field
  // they need for display; these aliases exist purely for sorting.
  const rows = players.map((p) => ({
    ...p,
    breakdownGp: p.breakdown?.gamesPlayed ?? 0,
    breakdownSog: p.breakdown?.sog ?? 0,
    breakdownPim: p.breakdown?.pim ?? 0,
    breakdownPlusMinus: p.breakdown?.plusMinus ?? 0,
    breakdownHits: p.breakdown?.hits ?? 0,
    breakdownToi: p.breakdown?.toiSecondsPerGame ?? 0,
    breakdownProjectedPpg: p.breakdown?.projectedPpg ?? 0,
    breakdownGrade: p.breakdown?.grade.zScore ?? 0,
    breakdownRemaining: p.breakdown?.remainingImpact.expectedRemainingPoints ?? 0,
    breakdownBucket: p.breakdown?.bucket ?? "tooEarly",
  }));

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[#1A1A1A] text-white px-6 py-3">
        <h2 className="font-extrabold uppercase tracking-wider text-sm">
          Roster Breakdown
        </h2>
      </header>
      <div className="overflow-x-auto">
        <RankingTable
          data={rows}
          columns={columns}
          keyField="nhlId"
          initialSortKey="totalPoints"
          initialSortDirection="desc"
          showRankColors={false}
          className="bg-transparent shadow-none border-0"
        />
      </div>
    </section>
  );
}
