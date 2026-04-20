import type { TeamOdds } from "../types";

interface LeagueRaceTableProps {
  teams: TeamOdds[];
  myTeamId?: number | null;
  /**
   * ISO timestamp from the race-odds response (`generatedAt`). Used to
   * caption the Win % / Top-3 columns so users know those numbers came
   * from the daily prewarm and don't refresh per goal — unlike Current /
   * Projected which DO update live.
   */
  generatedAt?: string;
}

/**
 * Columnar league-race view: rank · team · current pts · projected final ·
 * likely range · win probability · top-3 probability · pairwise probability
 * vs caller. The dominant (and only) visual for the league race — precise
 * numbers across every metric, no secondary chart tracks competing for the
 * eye.
 */
export function LeagueRaceTable({ teams, myTeamId, generatedAt }: LeagueRaceTableProps) {
  if (teams.length === 0) return null;
  const me = myTeamId != null ? teams.find((t) => t.teamId === myTeamId) : undefined;
  const showH2H = me != null && teams.length > 1;
  const showTop3 = teams.length > 3;
  const winSimAt = formatGeneratedAt(generatedAt);

  return (
    <div className="border border-[var(--color-divider)] overflow-x-auto">
      <table className="w-full text-sm">
        {winSimAt && (
          <caption className="caption-bottom px-3 py-2 text-[10px] text-[var(--color-ink-muted)] tabular-nums text-right">
            Current / Projected update live; Win % &amp; Top-3 from the simulation last run {winSimAt}.
          </caption>
        )}
        <thead>
          <tr className="bg-[var(--color-surface-sunk)] text-[10px] uppercase tracking-widest text-[var(--color-ink-muted)] font-bold">
            <th className="px-3 py-2 text-left w-8">#</th>
            <th className="px-3 py-2 text-left">Team</th>
            <th className="px-3 py-2 text-right">Current</th>
            <th className="px-3 py-2 text-right">Projected</th>
            <th className="px-3 py-2 text-right hidden sm:table-cell">Likely</th>
            <th className="px-3 py-2 text-right">Win %</th>
            {showTop3 && (
              <th className="px-3 py-2 text-right hidden md:table-cell">
                Top-3
              </th>
            )}
            {showH2H && (
              <th className="px-3 py-2 text-right" title="Probability you finish ahead of this team">
                You beat
              </th>
            )}
          </tr>
        </thead>
        <tbody>
          {teams.map((team, i) => {
            const isMine = myTeamId === team.teamId;
            // From caller's POV: P(I finish ahead of this team), pulled from
            // my own team's head_to_head map for semantic clarity.
            const iBeatThem =
              showH2H && !isMine && me
                ? me.headToHead[String(team.teamId)] ?? null
                : null;
            return (
              <tr
                key={team.teamId}
                className={`border-t border-[var(--color-divider)] ${
                  isMine
                    ? "bg-[var(--color-you-tint)]"
                    : ""
                }`}
              >
                <td className="px-3 py-2 text-left tabular-nums font-bold text-[var(--color-ink-muted)]">
                  {i + 1}
                </td>
                <td className="px-3 py-2 text-left font-bold uppercase tracking-wider text-xs text-[#1A1A1A]">
                  <span className="truncate">{team.teamName}</span>
                  {isMine && (
                    <span className="ml-2 text-[9px] bg-[var(--color-you)] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
                      YOU
                    </span>
                  )}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {team.currentPoints}
                </td>
                <td className="px-3 py-2 text-right tabular-nums font-extrabold text-[#1A1A1A]">
                  ~{Math.round(team.projectedFinalMean)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums text-[var(--color-ink-muted)] hidden sm:table-cell">
                  {Math.round(team.p10)}–{Math.round(team.p90)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums font-extrabold">
                  {Math.round(team.winProb * 100)}%
                </td>
                {showTop3 && (
                  <td className="px-3 py-2 text-right tabular-nums text-[var(--color-ink-muted)] hidden md:table-cell">
                    {Math.round(team.top3Prob * 100)}%
                  </td>
                )}
                {showH2H && (
                  <td className="px-3 py-2 text-right tabular-nums text-[var(--color-ink-muted)]">
                    {isMine
                      ? "—"
                      : iBeatThem != null
                        ? `${Math.round(iBeatThem * 100)}%`
                        : "—"}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/**
 * Format the response's `generatedAt` ISO timestamp into a short
 * "10:00 UTC, today" / "10:00 UTC, yesterday" form. The simulation
 * fires at 10:00 UTC daily so most of the time the user is reading
 * a same-day or one-day-old run; explicit dates only show up
 * if the cache hasn't been refreshed for some reason.
 */
function formatGeneratedAt(iso?: string): string | null {
  if (!iso) return null;
  const ts = new Date(iso);
  if (Number.isNaN(ts.getTime())) return null;
  const now = new Date();
  const sameDay =
    ts.getUTCFullYear() === now.getUTCFullYear() &&
    ts.getUTCMonth() === now.getUTCMonth() &&
    ts.getUTCDate() === now.getUTCDate();
  const yesterday = new Date(now);
  yesterday.setUTCDate(now.getUTCDate() - 1);
  const isYesterday =
    ts.getUTCFullYear() === yesterday.getUTCFullYear() &&
    ts.getUTCMonth() === yesterday.getUTCMonth() &&
    ts.getUTCDate() === yesterday.getUTCDate();
  const time = `${String(ts.getUTCHours()).padStart(2, "0")}:${String(ts.getUTCMinutes()).padStart(2, "0")} UTC`;
  if (sameDay) return `${time} today`;
  if (isYesterday) return `${time} yesterday`;
  const date = ts.toISOString().slice(0, 10);
  return `${time} on ${date}`;
}
