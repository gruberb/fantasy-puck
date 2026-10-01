import RankingTable from "@/components/common/RankingTable";
import { getLeagueRaceColumns } from "@/components/rankingsPageTableColumns/leagueRaceColumns";
import type { TeamOdds } from "@/features/race-odds/types";

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
 * Columnar league-race view: the dominant (and only) visual for the
 * league race. Precise numbers across every metric, no secondary chart
 * tracks competing for the eye.
 */
export function LeagueRaceTable({ teams, myTeamId, generatedAt }: LeagueRaceTableProps) {
  if (teams.length === 0) return null;
  const winSimAt = formatGeneratedAt(generatedAt);

  return (
    <div className="space-y-2">
      <RankingTable<TeamOdds>
        data={teams}
        columns={getLeagueRaceColumns(teams, myTeamId)}
        keyField="teamId"
        showRankColors={false}
        rowClassName={(row) =>
          row.teamId === myTeamId ? "bg-[var(--color-you-tint)]!" : ""
        }
      />
      {winSimAt && (
        <p className="text-[10px] text-[var(--color-ink-muted)] tabular-nums text-right">
          Current / Projected update live; Win % &amp; Top-3 from the simulation last run {winSimAt}.
        </p>
      )}
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
