import type { TeamOdds } from "../types";

interface LeagueRaceBoardProps {
  teams: TeamOdds[];
  myTeamId?: number | null;
}

/**
 * Per-team race-odds panel for league mode. One row per team, single visual
 * encoding (win-probability bar), with projected finish in supporting text
 * below. Deliberately avoids a second chart track — the audit flagged the
 * two-bar layout as a Ch 6 / Ch 7 violation (no dominant element, too many
 * factors changing at once).
 */
export function LeagueRaceBoard({ teams, myTeamId }: LeagueRaceBoardProps) {
  if (teams.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)]">
        Not enough data for a simulation yet — check back once the playoffs start.
      </p>
    );
  }

  // Only surface top-3 probability when it actually differentiates — in a
  // 2- or 3-team league every team is top-3, so the column is noise.
  const showTop3 = teams.length > 3;

  const maxWinProb = Math.max(...teams.map((t) => t.winProb), 0.001);

  return (
    <ol className="divide-y divide-[var(--color-divider)] border border-[var(--color-divider)]">
      {teams.map((team, i) => {
        const isMine = myTeamId === team.teamId;
        const winPct = Math.round(team.winProb * 100);
        // Bars are always normalised so the top team spans the full track;
        // this makes every other team legible by visual *ratio* rather than
        // as a tiny sliver compared to an abstract 100% max.
        const barWidth = Math.max(4, (team.winProb / maxWinProb) * 100);
        return (
          <li
            key={team.teamId}
            className={`px-4 py-3 ${
              isMine
                ? "bg-[var(--color-you-tint)] border-l-4 border-[var(--color-you)]"
                : ""
            }`}
          >
            {/* Row 1: rank · name · win % */}
            <div className="flex items-baseline gap-3">
              <span className="text-xs text-[var(--color-ink-muted)] tabular-nums font-bold w-4">
                {i + 1}
              </span>
              <span
                className={`font-bold text-sm uppercase tracking-wider flex-1 truncate ${
                  isMine ? "text-[#1A1A1A]" : "text-[#1A1A1A]"
                }`}
              >
                {team.teamName}
                {isMine && (
                  <span className="ml-2 text-[10px] bg-[var(--color-you)] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
                    YOU
                  </span>
                )}
              </span>
              <span className="font-extrabold text-sm tabular-nums text-[#1A1A1A]">
                {winPct}%
              </span>
              {showTop3 && (
                <span className="text-[10px] tabular-nums text-[var(--color-ink-muted)] w-16 text-right">
                  {Math.round(team.top3Prob * 100)}% top-3
                </span>
              )}
            </div>

            {/* Row 2: probability bar. Single encoding, dominant visual. */}
            <div className="mt-1.5 h-2 bg-[var(--color-divider)]">
              <div
                className={`h-full ${
                  isMine ? "bg-[var(--color-you)]" : "bg-[var(--color-rival)]"
                }`}
                style={{ width: `${barWidth}%` }}
              />
            </div>

            {/* Row 3: supporting text. Mean + likely range as one line. */}
            <p className="mt-1.5 text-[11px] text-[var(--color-ink-muted)] tabular-nums">
              Projected{" "}
              <span className="text-[#1A1A1A] font-bold">
                ~{Math.round(team.projectedFinalMean)} pts
              </span>{" "}
              (likely {Math.round(team.p10)}–{Math.round(team.p90)})
            </p>
          </li>
        );
      })}
    </ol>
  );
}
