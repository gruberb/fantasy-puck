import { nhlPlayerProfileUrl } from "@/utils/nhlTeams";
import type {
  FantasyTeamForecast,
  PlayerForecastCell,
  SeriesStateCode,
} from "@/features/pulse";

interface SeriesForecastHeroProps {
  forecasts: FantasyTeamForecast[];
  myTeamId: number | null;
}

const STATE_STYLES: Record<SeriesStateCode, string> = {
  eliminated: "bg-[#7F1D1D] text-white",
  facingElim: "bg-[#DC2626] text-white",
  trailing: "bg-[#FB923C] text-[#1A1A1A]",
  tied: "bg-[#E5E7EB] text-[#1A1A1A]",
  leading: "bg-[#86EFAC] text-[#1A1A1A]",
  aboutToAdvance: "bg-[#16A34A] text-white",
  advanced: "bg-[#14532D] text-white",
};

const STATE_LEGEND: { code: SeriesStateCode; label: string }[] = [
  { code: "eliminated", label: "Out" },
  { code: "facingElim", label: "Facing elim" },
  { code: "trailing", label: "Trailing" },
  { code: "tied", label: "Tied" },
  { code: "leading", label: "Leading" },
  { code: "aboutToAdvance", label: "Closing in" },
  { code: "advanced", label: "Advanced" },
];

export default function SeriesForecastHero({
  forecasts,
  myTeamId,
}: SeriesForecastHeroProps) {
  // Put my team first for scanability.
  const sorted = [...forecasts].sort((a, b) => {
    if (a.teamId === myTeamId) return -1;
    if (b.teamId === myTeamId) return 1;
    return a.teamName.localeCompare(b.teamName);
  });

  if (sorted.length === 0) {
    return (
      <section className="bg-white border-2 border-[#1A1A1A] p-6">
        <p className="text-[var(--color-ink-muted)] text-sm uppercase tracking-wider">
          Series forecast will appear once the bracket is set.
        </p>
      </section>
    );
  }

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[#1A1A1A] text-white px-6 py-3 flex items-center justify-between">
        <h2 className="font-extrabold uppercase tracking-wider text-sm">
          Series Rosters
        </h2>
        <div className="hidden md:flex items-center gap-2 text-[10px] uppercase tracking-wider">
          {STATE_LEGEND.map((s) => (
            <span key={s.code} className="flex items-center gap-1">
              <span
                className={`inline-block w-2.5 h-2.5 ${STATE_STYLES[s.code]}`}
              />
              <span>{s.label}</span>
            </span>
          ))}
        </div>
      </header>

      <div className="divide-y-2 divide-[#1A1A1A]">
        {sorted.map((team) => (
          <TeamForecastRow
            key={team.teamId}
            team={team}
            isMine={team.teamId === myTeamId}
          />
        ))}
      </div>
    </section>
  );
}

function TeamForecastRow({
  team,
  isMine,
}: {
  team: FantasyTeamForecast;
  isMine: boolean;
}) {
  const headline = composeHeadline(team);
  const hasRisk =
    team.playersFacingElimination > 0 || team.playersEliminated > 0;

  // Off-day condensation: every player is tied 0-0 → grid of identical cells
  // carries no information. Group by NHL matchup and show avatar chips.
  const allTied =
    team.cells.length > 0 &&
    team.cells.every((c) => c.wins === 0 && c.opponentWins === 0);

  return (
    <div
      className={`p-4 md:p-5 ${
        isMine ? "bg-[var(--color-you-tint)] border-l-4 border-[var(--color-you)]" : ""
      }`}
    >
      <div className="flex items-start justify-between gap-4 mb-3">
        <div>
          <h3 className="text-base md:text-lg font-extrabold uppercase tracking-wider">
            {team.teamName}
            {isMine && (
              <span className="ml-2 text-[10px] bg-[var(--color-you)] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
                YOU
              </span>
            )}
          </h3>
          <p
            className={`text-xs mt-1 ${
              hasRisk ? "text-[var(--color-error)] font-bold" : "text-[var(--color-ink-muted)]"
            }`}
          >
            {team.totalPlayers} player{team.totalPlayers !== 1 ? "s" : ""}
            {headline && ` — ${headline}`}
          </p>
        </div>
      </div>

      {allTied ? (
        <MatchupChipList cells={team.cells} />
      ) : (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-2">
          {team.cells.map((cell, i) => (
            <PlayerCell key={`${team.teamId}-${i}`} cell={cell} />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Build the "10 players — 3 facing elim, 4 tied, 2 leading" style headline,
 * only including groups that actually have members. A tied series is NOT
 * trailing, so it's listed under its own label.
 */
function composeHeadline(team: FantasyTeamForecast): string {
  const parts: string[] = [];
  if (team.playersFacingElimination > 0)
    parts.push(`${team.playersFacingElimination} facing elim`);
  if (team.playersEliminated > 0) parts.push(`${team.playersEliminated} out`);
  if (team.playersTrailing > 0) parts.push(`${team.playersTrailing} trailing`);
  if (team.playersTied > 0) parts.push(`${team.playersTied} tied`);
  if (team.playersLeading > 0) parts.push(`${team.playersLeading} leading`);
  if (team.playersAdvanced > 0) parts.push(`${team.playersAdvanced} advanced`);
  // Pre-bracket edge case: totalPlayers known but all cells default to 0-0
  // tied — headline reads "N tied" which is accurate but bland. Short-circuit
  // to a friendlier awaiting-puck-drop copy when the whole roster is tied.
  if (
    team.playersTied === team.totalPlayers &&
    team.totalPlayers > 0 &&
    parts.length === 1
  ) {
    return "awaiting puck drop";
  }
  return parts.join(", ");
}

/**
 * Off-day view: group the roster by NHL matchup and show each player as a
 * linked avatar chip. Players on the same NHL team are visually clustered
 * so the user can see "who I have on the Leafs vs Lightning series".
 */
function MatchupChipList({ cells }: { cells: PlayerForecastCell[] }) {
  const groups = new Map<
    string,
    {
      opponent: string | null;
      players: PlayerForecastCell[];
    }
  >();
  for (const cell of cells) {
    const existing = groups.get(cell.nhlTeam);
    if (existing) {
      existing.players.push(cell);
    } else {
      groups.set(cell.nhlTeam, {
        opponent: cell.opponentAbbrev,
        players: [cell],
      });
    }
  }
  return (
    <ul className="border border-gray-200 divide-y divide-gray-200">
      {Array.from(groups.entries()).map(([abbrev, group]) => (
        <li
          key={abbrev}
          className="flex items-center gap-3 px-3 py-2.5 flex-wrap"
        >
          <span className="text-xs font-bold uppercase tracking-wider w-20 flex-shrink-0 text-[#1A1A1A]">
            {abbrev}
            {group.opponent && (
              <span className="text-[var(--color-ink-muted)] font-normal"> vs {group.opponent}</span>
            )}
          </span>
          <div className="flex flex-wrap gap-1.5 flex-1 min-w-0">
            {group.players.map((p) => (
              <PlayerChip key={p.nhlId} player={p} />
            ))}
          </div>
          <span className="text-[10px] uppercase tracking-wider font-bold bg-[#E5E7EB] text-[#1A1A1A] px-1.5 py-0.5 flex-shrink-0">
            0-0 · 50%
          </span>
        </li>
      ))}
    </ul>
  );
}

function PlayerChip({ player }: { player: PlayerForecastCell }) {
  return (
    <a
      href={nhlPlayerProfileUrl(player.nhlId)}
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center gap-1.5 border border-gray-300 px-1.5 py-1 hover:border-[#1A1A1A] hover:bg-[var(--color-you-tint)] transition-colors duration-100"
    >
      <img
        src={player.headshotUrl}
        alt=""
        className="w-6 h-6 bg-gray-100"
        onError={(e) => {
          (e.currentTarget as HTMLImageElement).style.display = "none";
        }}
      />
      <span className="text-xs font-medium text-[#1A1A1A]">
        {player.playerName}
      </span>
      <span className="text-[10px] text-[var(--color-ink-muted)]">
        {player.position}
      </span>
    </a>
  );
}

function PlayerCell({ cell }: { cell: PlayerForecastCell }) {
  const pct = Math.round(cell.oddsToAdvance * 100);
  return (
    <div className="border border-gray-300">
      <div
        className={`px-2 py-1 text-[10px] uppercase tracking-wider font-bold ${
          STATE_STYLES[cell.seriesState]
        }`}
      >
        {cell.seriesLabel}
      </div>
      <a
        href={nhlPlayerProfileUrl(cell.nhlId)}
        target="_blank"
        rel="noopener noreferrer"
        className="p-2 flex items-start gap-2 hover:bg-[var(--color-you-tint)] transition-colors duration-100"
      >
        <img
          src={cell.headshotUrl}
          alt=""
          className="w-8 h-8 bg-gray-100 flex-shrink-0"
          onError={(e) => {
            (e.target as HTMLImageElement).style.display = "none";
          }}
        />
        <div className="min-w-0 flex-1">
          <p className="text-xs font-bold truncate text-[#1A1A1A]">
            {cell.playerName}
          </p>
          <p className="text-[10px] text-[var(--color-ink-muted)] mt-0.5">
            {cell.nhlTeam}
            {cell.opponentAbbrev ? ` vs ${cell.opponentAbbrev}` : ""} · {cell.position}
          </p>
          <p className="text-[10px] text-[var(--color-ink-muted)] mt-0.5 tabular-nums">
            {pct}% adv · {cell.gamesRemaining} left
          </p>
        </div>
      </a>
    </div>
  );
}
