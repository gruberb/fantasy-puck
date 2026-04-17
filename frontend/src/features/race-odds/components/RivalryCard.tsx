import type { RivalryCard as RivalryCardData } from "../types";

interface RivalryCardProps {
  rivalry: RivalryCardData;
  /**
   * Compact variant for the Pulse hero line (single row, no border). The
   * default variant is a full card suitable for an Insights section.
   */
  variant?: "card" | "compact";
}

/**
 * Head-to-head framing against the caller's closest rival by projected
 * mean. Replaces the gray→red split from the earlier design: now uses the
 * formalized you/rival axis (warm yellow vs cool slate) so the rival side
 * no longer reads as "danger".
 */
export function RivalryCard({ rivalry, variant = "card" }: RivalryCardProps) {
  const myPct = Math.round(rivalry.myHeadToHeadProb * 100);
  const rivalPct = 100 - myPct;
  const gap = rivalry.myProjectedMean - rivalry.rivalProjectedMean;
  const gapLabel = gap >= 0 ? `+${Math.round(gap)}` : `${Math.round(gap)}`;

  if (variant === "compact") {
    return (
      <div className="flex items-center gap-3">
        <div className="flex h-2 flex-1 border border-[#1A1A1A]">
          <div
            className="bg-[var(--color-you)]"
            style={{ width: `${myPct}%` }}
          />
          <div
            className="bg-[var(--color-rival)]"
            style={{ width: `${rivalPct}%` }}
          />
        </div>
        <p className="text-sm text-[#1A1A1A] tabular-nums">
          <strong className="font-extrabold">{myPct}%</strong>
          <span className="text-[var(--color-ink-muted)]"> to finish ahead of </span>
          <strong className="font-extrabold">{rivalry.rivalTeamName}</strong>
          <span className="text-[var(--color-ink-muted)]"> ({gapLabel} pts)</span>
        </p>
      </div>
    );
  }

  return (
    <div className="border-2 border-[#1A1A1A] bg-white">
      <div className="bg-[#1A1A1A] px-4 py-2">
        <span className="text-[10px] uppercase tracking-widest text-white font-bold">
          Head-to-Head
        </span>
      </div>
      <div className="p-4">
        {/* Names row: YOU on the left, RIVAL on the right, with each team's
            identity color anchoring their side. No red anywhere. */}
        <div className="flex items-center gap-3 mb-3">
          <div className="flex-1">
            <p className="text-[10px] uppercase tracking-wider text-[var(--color-ink-muted)] font-bold">
              You
            </p>
            <p className="text-sm font-extrabold uppercase tracking-wider truncate text-[#1A1A1A]">
              {rivalry.myTeamName}
            </p>
          </div>
          <span className="text-[10px] uppercase tracking-widest text-[var(--color-ink-muted)]">
            vs
          </span>
          <div className="flex-1 text-right">
            <p className="text-[10px] uppercase tracking-wider text-[var(--color-ink-muted)] font-bold">
              Rival
            </p>
            <p className="text-sm font-extrabold uppercase tracking-wider truncate text-[#1A1A1A]">
              {rivalry.rivalTeamName}
            </p>
          </div>
        </div>

        {/* Divergent probability bar. Warm yellow (you) vs cool slate (rival):
            winner-ness comes from width, not from a red "danger" cue. */}
        <div className="flex h-3 border-2 border-[#1A1A1A]">
          <div
            className="bg-[var(--color-you)]"
            style={{ width: `${myPct}%` }}
          />
          <div
            className="bg-[var(--color-rival)]"
            style={{ width: `${rivalPct}%` }}
          />
        </div>
        <div className="flex justify-between mt-1 text-[11px] tabular-nums font-bold">
          <span className="text-[#1A1A1A]">{myPct}%</span>
          <span className="text-[#1A1A1A]">{rivalPct}%</span>
        </div>

        <p className="mt-3 text-xs text-[var(--color-ink-muted)] leading-relaxed">
          Projected finish:{" "}
          <strong className="text-[#1A1A1A] tabular-nums">
            {Math.round(rivalry.myProjectedMean)}
          </strong>{" "}
          vs{" "}
          <strong className="text-[#1A1A1A] tabular-nums">
            {Math.round(rivalry.rivalProjectedMean)}
          </strong>{" "}
          <span>({gapLabel} pts)</span>
        </p>
      </div>
    </div>
  );
}
