import type { Column } from "@/components/common/RankingTable/types";
import type { SkaterStats } from "@/types/skaters";
import { BucketPill, GradeBadge } from "@/components/common/PlayerBreakdownBadges";
import { nhlPlayerProfileUrl } from "@/utils/nhlTeams";
import { formatToi } from "@/utils/format";

export function useTeamBreakdownColumns(): Column[] {
  return [
    {
      key: "name",
      header: "Skater",
      sortable: true,
      className: "font-medium",
      render: (_v, row) => {
        const s = row as unknown as SkaterStats;
        return (
          <a
            href={nhlPlayerProfileUrl(s.nhlId)}
            target="_blank"
            rel="noopener noreferrer"
            className="font-bold text-sm text-[#1A1A1A] hover:text-[#2563EB] whitespace-nowrap"
          >
            {abbreviateName(s.name)}
            <span className="text-gray-500 font-normal"> · {s.nhlTeam}</span>
          </a>
        );
      },
    },
    {
      key: "breakdownGp",
      header: "GP",
      sortable: true,
      render: (_v, row) => {
        const b = (row as SkaterStats).breakdown;
        return <span>{b?.gamesPlayed ?? 0}</span>;
      },
    },
    { key: "goals", header: "G", sortable: true },
    { key: "assists", header: "A", sortable: true },
    {
      key: "totalPoints",
      header: "P",
      sortable: true,
      className: "font-bold",
    },
    {
      key: "breakdownSog",
      header: "SOG",
      sortable: true,
      responsive: "md",
      render: (_v, row) => <span>{(row as SkaterStats).breakdown?.sog ?? 0}</span>,
    },
    {
      key: "breakdownPim",
      header: "PIM",
      sortable: true,
      responsive: "md",
      render: (_v, row) => <span>{(row as SkaterStats).breakdown?.pim ?? 0}</span>,
    },
    {
      key: "breakdownPlusMinus",
      header: "+/-",
      sortable: true,
      responsive: "md",
      render: (_v, row) => {
        const v = (row as SkaterStats).breakdown?.plusMinus ?? 0;
        const cls =
          v > 0 ? "text-green-700 font-bold" : v < 0 ? "text-red-700 font-bold" : "";
        return <span className={cls}>{v > 0 ? `+${v}` : v}</span>;
      },
    },
    {
      key: "breakdownHits",
      header: "HIT",
      sortable: true,
      responsive: "md",
      render: (_v, row) => <span>{(row as SkaterStats).breakdown?.hits ?? 0}</span>,
    },
    {
      key: "breakdownToi",
      header: "TOI",
      sortable: true,
      responsive: "lg",
      render: (_v, row) => {
        const s = (row as SkaterStats).breakdown?.toiSecondsPerGame ?? 0;
        return <span className="tabular-nums">{s > 0 ? formatToi(s) : "—"}</span>;
      },
    },
    {
      key: "breakdownProjectedPpg",
      header: "PROJ",
      sortable: true,
      responsive: "lg",
      render: (_v, row) => {
        const ppg = (row as SkaterStats).breakdown?.projectedPpg ?? 0;
        return <span className="tabular-nums">{ppg.toFixed(2)}</span>;
      },
    },
    {
      key: "breakdownGrade",
      header: "Grade",
      sortable: true,
      render: (_v, row) => {
        const b = (row as SkaterStats).breakdown;
        if (!b) return <span className="text-gray-400">—</span>;
        return <GradeBadge grade={b.grade.grade} />;
      },
    },
    {
      key: "breakdownRemaining",
      header: "Rest-of-run",
      sortable: true,
      responsive: "md",
      render: (_v, row) => {
        const b = (row as SkaterStats).breakdown;
        if (!b || b.remainingImpact.nhlTeamEliminated) {
          return <span className="text-gray-400">—</span>;
        }
        if (b.remainingImpact.expectedRemainingPoints === 0) {
          return <span className="text-gray-400">—</span>;
        }
        return (
          <span className="tabular-nums">
            {b.remainingImpact.expectedRemainingPoints.toFixed(1)}
          </span>
        );
      },
    },
    {
      key: "breakdownBucket",
      header: "Status",
      sortable: true,
      render: (_v, row) => {
        const b = (row as SkaterStats).breakdown;
        if (!b) return <span className="text-gray-400">—</span>;
        return <BucketPill bucket={b.bucket} />;
      },
    },
  ];
}

/** "Alex Tuch" → "A. Tuch"; "Sebastian Aho" → "S. Aho". Keeps accents
 *  and hyphens intact so "Juraj Slafkovský" → "J. Slafkovský". */
function abbreviateName(name: string): string {
  const trimmed = name.trim();
  const space = trimmed.indexOf(" ");
  if (space <= 0) return trimmed;
  const first = trimmed.slice(0, space);
  const rest = trimmed.slice(space + 1);
  const initial = Array.from(first)[0] ?? "";
  return `${initial}. ${rest}`;
}
