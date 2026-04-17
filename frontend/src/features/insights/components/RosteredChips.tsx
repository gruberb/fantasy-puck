import { useLeague } from "@/contexts/LeagueContext";

import type { RosteredPlayerTag } from "@/features/insights";

interface RosteredChipsProps {
  tags: RosteredPlayerTag[];
}

/**
 * Shows only the caller's fantasy-team exposure on this NHL team. In small
 * leagues listing every owner is useful; in 15-team leagues it floods the
 * layout. The signal most users scan for is "do I have skin in this game?" —
 * so that's all we render. Nothing shows when the caller isn't in the league
 * or doesn't own any players on this NHL team.
 */
export function RosteredChips({ tags }: RosteredChipsProps) {
  const { activeLeagueId, myMemberships } = useLeague();

  if (!activeLeagueId || tags.length === 0) return null;

  const myTeamName = myMemberships.find(
    (m) => m.league_id === activeLeagueId,
  )?.fantasy_teams?.name;
  if (!myTeamName) return null;

  const myTag = tags.find((t) => t.fantasyTeamName === myTeamName);
  if (!myTag) return null;

  return (
    <span className="inline-flex items-center gap-1 bg-[var(--color-you-tint)] text-[#1A1A1A] px-1 py-0 text-[9px] uppercase tracking-wider font-bold">
      You: {myTag.count}
    </span>
  );
}
