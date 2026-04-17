import { useState } from "react";

import type { RosteredPlayerTag } from "@/features/insights";

interface RosteredChipsProps {
  tags: RosteredPlayerTag[];
  /** How many chips to show before collapsing the rest. Defaults to 3. */
  limit?: number;
}

/**
 * Collapsed roster-ownership chips. Shows the top `limit` fantasy teams by
 * count; any remaining teams are hidden behind a "+N MORE" toggle that
 * expands the list in place.
 */
export function RosteredChips({ tags, limit = 3 }: RosteredChipsProps) {
  const [expanded, setExpanded] = useState(false);
  if (tags.length === 0) return null;

  const sorted = [...tags].sort((a, b) => b.count - a.count);
  const visible = expanded ? sorted : sorted.slice(0, limit);
  const hiddenCount = sorted.length - limit;
  const showToggle = hiddenCount > 0;

  return (
    <div className="flex flex-wrap gap-1 items-center">
      {visible.map((tag) => (
        <span
          key={tag.fantasyTeamName}
          className="inline-flex items-center gap-1 bg-[var(--color-you-tint)] text-[#1A1A1A] px-1 py-0 text-[9px] uppercase tracking-wider font-bold"
        >
          {tag.fantasyTeamName}: {tag.count}
        </span>
      ))}
      {showToggle && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setExpanded((v) => !v);
          }}
          aria-expanded={expanded}
          aria-label={
            expanded
              ? "Collapse rostered teams"
              : `Show ${hiddenCount} more rostered team${hiddenCount === 1 ? "" : "s"}`
          }
          className="inline-flex items-center gap-1 border border-[#1A1A1A] bg-white text-[#1A1A1A] hover:bg-[#1A1A1A] hover:text-white active:bg-[#1A1A1A] active:text-white px-1.5 py-0.5 text-[9px] uppercase tracking-wider font-bold transition-colors cursor-pointer touch-manipulation select-none"
        >
          {expanded ? "Collapse" : `+${hiddenCount} More`}
        </button>
      )}
    </div>
  );
}
