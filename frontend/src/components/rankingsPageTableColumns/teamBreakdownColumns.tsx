import type { Column } from "@/components/common/RankingTable/types";
import type { PlayerBucket, PlayerGrade, SkaterStats } from "@/types/skaters";

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
          <div className="flex items-center space-x-2 min-w-[10rem]">
            {s.imageUrl ? (
              <img
                src={s.imageUrl}
                alt={s.name}
                className="w-8 h-8 rounded-none"
              />
            ) : (
              <div className="w-8 h-8 bg-gray-200 flex items-center justify-center text-[10px] font-bold">
                {s.name
                  .split(" ")
                  .map((w) => w[0])
                  .join("")
                  .slice(0, 2)
                  .toUpperCase()}
              </div>
            )}
            <a
              href={`https://www.nhl.com/player/${s.nhlId}`}
              target="_blank"
              rel="noopener noreferrer"
              className="font-bold text-base text-[#1A1A1A] hover:text-[#2563EB]"
            >
              {s.name}
            </a>
          </div>
        );
      },
    },
    {
      key: "nhlTeam",
      header: "Team",
      sortable: true,
      render: (_v, row) => {
        const s = row as unknown as SkaterStats;
        return (
          <span className="text-xs tracking-wider">
            {s.nhlTeam} · {s.position}
          </span>
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
        return <span className="tabular-nums">{formatToi(s)}</span>;
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

// -- primitives ------------------------------------------------------

const GRADE_COLORS: Record<PlayerGrade, string> = {
  a: "bg-[#22C55E] text-white",
  b: "bg-[#84CC16] text-[#1A1A1A]",
  c: "bg-[#FACC15] text-[#1A1A1A]",
  d: "bg-[#F97316] text-white",
  f: "bg-[#EF4444] text-white",
  notEnoughData: "bg-gray-200 text-gray-600",
};

const GRADE_LABEL: Record<PlayerGrade, string> = {
  a: "A",
  b: "B",
  c: "C",
  d: "D",
  f: "F",
  notEnoughData: "—",
};

function GradeBadge({ grade }: { grade: PlayerGrade }) {
  return (
    <span
      className={`inline-block border-2 border-[#1A1A1A] px-2 py-0.5 text-xs font-bold tracking-wider uppercase ${GRADE_COLORS[grade]}`}
    >
      {GRADE_LABEL[grade]}
    </span>
  );
}

// Descriptive labels only — the roster is locked for the playoffs, so
// these describe the player's situation rather than prescribe an
// action. "On expected" replaces "On pace"; "Due" replaces "Keep
// faith"; "Fading" replaces "Need a miracle"; "Not in lineup"
// replaces "Problem asset".
const BUCKET_LABEL: Record<PlayerBucket, string> = {
  tooEarly: "TOO EARLY",
  outperforming: "AHEAD",
  onPace: "ON EXPECTED",
  keepFaith: "DUE",
  fineButFragile: "BELOW EXPECTED",
  needMiracle: "FADING",
  problemAsset: "NOT IN LINEUP",
  teamEliminated: "TEAM OUT",
};

const BUCKET_COLORS: Record<PlayerBucket, string> = {
  tooEarly: "bg-gray-200 text-gray-700",
  outperforming: "bg-[#22C55E] text-white",
  onPace: "bg-[#84CC16] text-[#1A1A1A]",
  keepFaith: "bg-[#FACC15] text-[#1A1A1A]",
  fineButFragile: "bg-[#FACC15] text-[#1A1A1A]",
  needMiracle: "bg-[#F97316] text-white",
  problemAsset: "bg-[#EF4444] text-white",
  teamEliminated: "bg-gray-300 text-gray-700",
};

function BucketPill({ bucket }: { bucket: PlayerBucket }) {
  return (
    <span
      className={`inline-block border-2 border-[#1A1A1A] px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase whitespace-nowrap ${BUCKET_COLORS[bucket]}`}
    >
      {BUCKET_LABEL[bucket]}
    </span>
  );
}

function formatToi(seconds: number): string {
  if (!seconds || seconds <= 0) return "—";
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
