import type { Column } from "@/components/common/RankingTable/types";
import type { TeamOdds } from "@/features/race-odds/types";

const pct = (p: number) => `${Math.round(p * 100)}%`;

/**
 * Column set for the league race table: current pts, projected final,
 * likely range, win / top-3 probability and, when the caller has a team,
 * the pairwise probability of finishing ahead of each rival.
 */
export function getLeagueRaceColumns(
  teams: TeamOdds[],
  myTeamId?: number | null,
): Column<TeamOdds>[] {
  const me =
    myTeamId != null ? teams.find((t) => t.teamId === myTeamId) : undefined;

  const columns: Column<TeamOdds>[] = [
    { key: "rank", header: "#" },
    {
      key: "teamName",
      header: "Team",
      className: "font-bold uppercase tracking-wider text-xs text-[#1A1A1A]",
      render: (value: string, row) => (
        <span className="whitespace-nowrap">
          {value}
          {row.teamId === myTeamId && (
            <span className="ml-2 text-[9px] bg-[var(--color-you)] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
              YOU
            </span>
          )}
        </span>
      ),
    },
    { key: "currentPoints", header: "Current", className: "tabular-nums" },
    {
      key: "projectedFinalMean",
      header: "Projected",
      className: "tabular-nums font-extrabold text-[#1A1A1A]",
      render: (value: number) => `~${Math.round(value)}`,
    },
    {
      key: "p10",
      header: "Likely",
      responsive: "sm",
      className: "tabular-nums text-[var(--color-ink-muted)] whitespace-nowrap",
      render: (_value, row) => `${Math.round(row.p10)}–${Math.round(row.p90)}`,
    },
    {
      key: "winProb",
      header: "Win %",
      className: "tabular-nums font-extrabold",
      render: pct,
    },
  ];

  if (teams.length > 3) {
    columns.push({
      key: "top3Prob",
      header: "Top-3",
      responsive: "md",
      className: "tabular-nums text-[var(--color-ink-muted)]",
      render: pct,
    });
  }

  if (me && teams.length > 1) {
    columns.push({
      key: "headToHead",
      header: "You beat",
      className: "tabular-nums text-[var(--color-ink-muted)]",
      // Read from the caller's own head-to-head map so the number is
      // always "P(I finish ahead of this team)".
      render: (_value, row) => {
        const p = row.teamId === me.teamId ? null : me.headToHead[String(row.teamId)];
        return p == null ? "-" : pct(p);
      },
    });
  }

  return columns;
}
