import type { RosteredSkaterRow } from "@/types/leagueStats";

interface Props {
  rows: RosteredSkaterRow[];
  myFantasyTeamId?: number | null;
}

/**
 * Top 10 rostered skaters by playoff fantasy points. One row per
 * player; the fantasy team that rosters them sits in the rightmost
 * column so it's easy to scan "who's carrying whom."
 */
export function TopRosteredSkatersTable({ rows, myFantasyTeamId }: Props) {
  if (rows.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
        No rostered skaters with playoff stats yet.
      </p>
    );
  }

  return (
    <div className="border border-[var(--color-divider)] overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="bg-[var(--color-surface-sunk)] text-[10px] uppercase tracking-widest text-[var(--color-ink-muted)] font-bold">
            <th className="px-3 py-2 text-left w-8">#</th>
            <th className="px-3 py-2 text-left">Skater</th>
            <th className="px-3 py-2 text-left hidden sm:table-cell">NHL team</th>
            <th className="px-3 py-2 text-right">Playoff pts</th>
            <th className="px-3 py-2 text-left">Rostered by</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => {
            const isMine = myFantasyTeamId === row.fantasyTeamId;
            return (
              <tr
                key={row.nhlId}
                className={`border-t border-[var(--color-divider)] ${
                  isMine ? "bg-[var(--color-you-tint)]" : ""
                }`}
              >
                <td className="px-3 py-2 text-left tabular-nums font-bold text-[var(--color-ink-muted)]">
                  {i + 1}
                </td>
                <td className="px-3 py-2 text-left">
                  <div className="flex items-center gap-2 min-w-0">
                    <PlayerPhoto src={row.photo} name={row.name} />
                    <span className="font-bold uppercase tracking-wider text-xs text-[#1A1A1A] truncate">
                      {row.name}
                    </span>
                  </div>
                </td>
                <td className="px-3 py-2 text-left hidden sm:table-cell">
                  <div className="flex items-center gap-2">
                    <TeamLogo src={row.teamLogo} abbrev={row.nhlTeam} />
                    <span className="text-xs tabular-nums text-[var(--color-ink-muted)]">
                      {row.nhlTeam}
                    </span>
                  </div>
                </td>
                <td className="px-3 py-2 text-right tabular-nums font-extrabold text-[#1A1A1A]">
                  {row.playoffPoints}
                </td>
                <td className="px-3 py-2 text-left">
                  <span className="font-bold uppercase tracking-wider text-xs text-[#1A1A1A]">
                    {row.fantasyTeamName}
                  </span>
                  {isMine && (
                    <span className="ml-2 text-[9px] bg-[var(--color-you)] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
                      YOU
                    </span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function TeamLogo({ src, abbrev }: { src: string; abbrev: string }) {
  if (!src)
    return (
      <div className="w-6 h-6 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[8px] font-bold">
        {abbrev}
      </div>
    );
  return (
    <img
      src={src}
      alt={abbrev}
      className="w-6 h-6 flex-shrink-0"
      onError={(e) => {
        const el = document.createElement("div");
        el.className =
          "w-6 h-6 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[8px] font-bold";
        el.textContent = abbrev;
        e.currentTarget.replaceWith(el);
      }}
    />
  );
}

function PlayerPhoto({ src, name }: { src: string; name: string }) {
  const initials = name
    .split(" ")
    .map((s) => s[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
  if (!src)
    return (
      <div className="w-8 h-8 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[10px] font-bold">
        {initials}
      </div>
    );
  return (
    <img
      src={src}
      alt={name}
      className="w-8 h-8 flex-shrink-0 rounded-none bg-[var(--color-surface-sunk)]"
      onError={(e) => {
        const el = document.createElement("div");
        el.className =
          "w-8 h-8 flex-shrink-0 bg-[var(--color-surface-sunk)] flex items-center justify-center text-[10px] font-bold";
        el.textContent = initials;
        e.currentTarget.replaceWith(el);
      }}
    />
  );
}
