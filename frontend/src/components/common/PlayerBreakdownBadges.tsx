import type { PlayerBucket, PlayerGrade } from "@/types/skaters";

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

export function GradeBadge({ grade }: { grade: PlayerGrade }) {
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

export function BucketPill({ bucket }: { bucket: PlayerBucket }) {
  return (
    <span
      className={`inline-block border-2 border-[#1A1A1A] px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase whitespace-nowrap ${BUCKET_COLORS[bucket]}`}
    >
      {BUCKET_LABEL[bucket]}
    </span>
  );
}
