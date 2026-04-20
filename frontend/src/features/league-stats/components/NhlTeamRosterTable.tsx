import type { NhlTeamRosterRow } from "@/types/leagueStats";

interface Props {
  rows: NhlTeamRosterRow[];
}

/**
 * "Where our rosters land" — one row per NHL team with at least one
 * rostered fantasy player in the league. Columns:
 *   # · NHL team (logo + name) · rostered count · NHL team playoff pts ·
 *   that NHL team's top playoff scorer (photo + pts)
 *
 * Sorted by rostered count DESC so heavily-owned teams surface first.
 */
export function NhlTeamRosterTable({ rows }: Props) {
  if (rows.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
        No rostered NHL teams yet.
      </p>
    );
  }

  return (
    <div className="border border-[var(--color-divider)] overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="bg-[var(--color-surface-sunk)] text-[10px] uppercase tracking-widest text-[var(--color-ink-muted)] font-bold">
            <th className="px-3 py-2 text-left w-8">#</th>
            <th className="px-3 py-2 text-left">NHL team</th>
            <th className="px-3 py-2 text-right">Rostered</th>
            <th className="px-3 py-2 text-right">Playoff pts</th>
            <th className="px-3 py-2 text-left hidden sm:table-cell">Top skater</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr
              key={row.nhlTeam}
              className="border-t border-[var(--color-divider)]"
            >
              <td className="px-3 py-2 text-left tabular-nums font-bold text-[var(--color-ink-muted)]">
                {i + 1}
              </td>
              <td className="px-3 py-2 text-left">
                <div className="flex items-center gap-2">
                  <TeamLogo src={row.teamLogo} abbrev={row.nhlTeam} />
                  <div className="flex flex-col min-w-0">
                    <span className="font-bold uppercase tracking-wider text-xs text-[#1A1A1A] truncate">
                      {row.teamName}
                    </span>
                    <span className="text-[10px] tabular-nums text-[var(--color-ink-muted)]">
                      {row.nhlTeam}
                    </span>
                  </div>
                </div>
              </td>
              <td className="px-3 py-2 text-right tabular-nums font-extrabold text-[#1A1A1A]">
                {row.rosteredCount}
              </td>
              <td className="px-3 py-2 text-right tabular-nums">
                {row.playoffPoints}
              </td>
              <td className="px-3 py-2 text-left hidden sm:table-cell">
                {row.topSkaterName ? (
                  <div className="flex items-center gap-2">
                    <PlayerPhoto
                      src={row.topSkaterPhoto}
                      name={row.topSkaterName}
                    />
                    <div className="flex flex-col min-w-0">
                      <span className="text-xs text-[#1A1A1A] truncate">
                        {row.topSkaterName}
                      </span>
                      <span className="text-[10px] tabular-nums text-[var(--color-ink-muted)]">
                        {row.topSkaterPoints ?? 0} pts
                      </span>
                    </div>
                  </div>
                ) : (
                  <span className="text-[11px] text-[var(--color-ink-muted)]">
                    —
                  </span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function TeamLogo({ src, abbrev }: { src: string; abbrev: string }) {
  if (!src) return <LogoFallback label={abbrev} />;
  return (
    <img
      src={src}
      alt={abbrev}
      className="w-8 h-8 flex-shrink-0"
      onError={(e) => {
        // NHL logo endpoints sometimes 404 for relocated franchises;
        // swap in the abbrev pill so the row still renders cleanly.
        e.currentTarget.replaceWith(buildFallback(abbrev));
      }}
    />
  );
}

function PlayerPhoto({
  src,
  name,
}: {
  src: string | null;
  name: string;
}) {
  const initials = name
    .split(" ")
    .map((s) => s[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
  if (!src) return <PhotoFallback label={initials} />;
  return (
    <img
      src={src}
      alt={name}
      className="w-8 h-8 flex-shrink-0 rounded-none bg-[var(--color-surface-sunk)]"
      onError={(e) => {
        e.currentTarget.replaceWith(buildFallback(initials));
      }}
    />
  );
}

function LogoFallback({ label }: { label: string }) {
  return (
    <div className="w-8 h-8 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[9px] font-bold tabular-nums">
      {label}
    </div>
  );
}

function PhotoFallback({ label }: { label: string }) {
  return (
    <div className="w-8 h-8 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[10px] font-bold">
      {label}
    </div>
  );
}

function buildFallback(label: string): HTMLDivElement {
  const el = document.createElement("div");
  el.className =
    "w-8 h-8 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[9px] font-bold";
  el.textContent = label;
  return el;
}
