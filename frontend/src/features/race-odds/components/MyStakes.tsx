import { useRaceOdds } from "../hooks/use-race-odds";
import {
  getNHLTeamLogoUrl,
  getNHLTeamShortName,
  nhlPlayerProfileUrl,
} from "@/utils/nhlTeams";
import type {
  FantasyTeamForecast,
  PlayerForecastCell,
} from "@/features/pulse";
import type { NhlTeamOdds } from "../types";

interface MyStakesProps {
  /** The Series Forecast entry for the caller's fantasy team. */
  myTeam: FantasyTeamForecast | null;
}

/**
 * Answers "which NHL series am I rooting for?" — lists every NHL team the
 * caller has a player on, sorted by impact on the race (roughly: # of my
 * players × team's expected games remaining). Each row shows the team's
 * current series, remaining-round probabilities, and my projected points
 * from that side of the bracket.
 */
export function MyStakes({ myTeam }: MyStakesProps) {
  const { data, isLoading } = useRaceOdds({ myTeamId: myTeam?.teamId });

  if (!myTeam || myTeam.cells.length === 0) {
    return null;
  }
  if (isLoading || !data) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
        Running simulation…
      </p>
    );
  }

  const byAbbrev: Map<string, NhlTeamOdds> = new Map(
    data.nhlTeams.map((t) => [t.abbrev, t]),
  );
  // Group the caller's roster by NHL team.
  const stakes = groupStakes(myTeam.cells, byAbbrev);
  if (stakes.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)]">
        Your players aren't on any active playoff teams — no stakes this round.
      </p>
    );
  }

  return (
    <div>
      <p className="text-[11px] text-[var(--color-ink-muted)] mb-3 leading-relaxed">
        Every NHL team you have skin in, sorted by how much they move your
        race. Cup % comes from {data.trials.toLocaleString()} bracket trials,
        re-run every morning.
      </p>
      <ul className="border border-[var(--color-divider)] divide-y divide-[var(--color-divider)]">
        {stakes.map((stake) => (
          <StakeRow key={stake.abbrev} stake={stake} />
        ))}
      </ul>
    </div>
  );
}

// ---------------------------------------------------------------------------

interface Stake {
  abbrev: string;
  odds: NhlTeamOdds | undefined;
  series: {
    opponent: string | null;
    wins: number;
    opponentWins: number;
    gamesRemaining: number;
  };
  players: PlayerForecastCell[];
  /**
   * Rough stakes score used for sorting: teams with more of my players + more
   * expected games + higher advance odds rise. Not a prediction — just a
   * "where's my attention" proxy.
   */
  impact: number;
}

function groupStakes(
  cells: PlayerForecastCell[],
  byAbbrev: Map<string, NhlTeamOdds>,
): Stake[] {
  const groups = new Map<string, Stake>();
  for (const cell of cells) {
    const existing = groups.get(cell.nhlTeam);
    if (existing) {
      existing.players.push(cell);
    } else {
      const odds = byAbbrev.get(cell.nhlTeam);
      groups.set(cell.nhlTeam, {
        abbrev: cell.nhlTeam,
        odds,
        series: {
          opponent: cell.opponentAbbrev,
          wins: cell.wins,
          opponentWins: cell.opponentWins,
          gamesRemaining: cell.gamesRemaining,
        },
        players: [cell],
        impact: 0,
      });
    }
  }
  for (const stake of groups.values()) {
    const expectedGames = stake.odds?.expectedGames ?? 0;
    stake.impact = stake.players.length * expectedGames;
  }
  return Array.from(groups.values()).sort((a, b) => b.impact - a.impact);
}

function StakeRow({ stake }: { stake: Stake }) {
  const oddsCupPct = stake.odds
    ? Math.round(stake.odds.cupWinProb * 100)
    : null;
  const oddsR1Pct = stake.odds
    ? Math.round(stake.odds.advanceRound1Prob * 100)
    : null;
  const oddsFinalsPct = stake.odds
    ? Math.round(stake.odds.cupFinalsProb * 100)
    : null;

  return (
    <li className="p-3">
      <div className="flex items-center gap-3 mb-2">
        <img
          src={getNHLTeamLogoUrl(stake.abbrev)}
          alt={stake.abbrev}
          className="w-8 h-8 flex-shrink-0"
        />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-extrabold uppercase tracking-wider text-[#1A1A1A]">
            {getNHLTeamShortName(stake.abbrev)}
          </p>
          <p className="text-[11px] text-[var(--color-ink-muted)]">
            {stake.series.opponent
              ? `vs ${stake.series.opponent} · ${stake.series.wins}-${stake.series.opponentWins}`
              : "—"}
            {" · "}
            {stake.players.length} of your players
          </p>
        </div>
        {oddsCupPct != null && (
          <div className="text-right">
            <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
              Cup
            </p>
            <p className="text-lg font-extrabold tabular-nums text-[#1A1A1A]">
              {oddsCupPct}%
            </p>
          </div>
        )}
      </div>

      {/* Odds breakdown bar: R1 | Finals | Cup. Same 0-100 scale across rows
          so you can compare teams at a glance. */}
      {stake.odds && (
        <div className="grid grid-cols-3 gap-2 mb-3">
          <MiniStat label="Win R1" value={`${oddsR1Pct}%`} />
          <MiniStat label="Final" value={`${oddsFinalsPct}%`} />
          <MiniStat label="Games (avg)" value={stake.odds.expectedGames.toFixed(1)} />
        </div>
      )}

      {/* Player chips, linked to NHL profile. */}
      <div className="flex flex-wrap gap-1.5">
        {stake.players.map((p) => (
          <a
            key={p.nhlId}
            href={nhlPlayerProfileUrl(p.nhlId)}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 border border-[var(--color-divider)] px-1.5 py-0.5 hover:border-[#1A1A1A] hover:bg-[var(--color-you-tint)] transition-colors duration-100"
          >
            <img
              src={p.headshotUrl}
              alt=""
              className="w-5 h-5 bg-gray-100"
              onError={(e) => {
                (e.currentTarget as HTMLImageElement).style.display = "none";
              }}
            />
            <span className="text-[11px] font-medium text-[#1A1A1A]">
              {p.playerName}
            </span>
            <span className="text-[9px] text-[var(--color-ink-muted)] uppercase">
              {p.position}
            </span>
          </a>
        ))}
      </div>
    </li>
  );
}

function MiniStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-[var(--color-surface-sunk)] px-2 py-1">
      <p className="text-[9px] uppercase tracking-widest text-[var(--color-ink-muted)] font-bold">
        {label}
      </p>
      <p className="text-sm font-bold tabular-nums text-[#1A1A1A]">{value}</p>
    </div>
  );
}
