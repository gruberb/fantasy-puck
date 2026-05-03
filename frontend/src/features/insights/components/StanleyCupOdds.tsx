import { useRaceOdds } from "@/features/race-odds/hooks/use-race-odds";
import {
  getNHLTeamLogoUrl,
  getNHLTeamShortName,
} from "@/utils/nhlTeams";

import type { TeamSeriesProjection } from "@/features/insights";

interface StanleyCupOddsProps {
  /**
   * Active-round series context keyed by NHL abbrev, used to annotate each
   * row with "vs OPP · 2-1". Passed from Insights which already fetches it.
   */
  projections: TeamSeriesProjection[];
}

/**
 * Championship-focused ranked table of every still-alive NHL playoff team.
 * Complements the matchup-focused Bracket view. Transparent about methodology
 * so the user knows what the numbers mean and where they came from.
 */
export function StanleyCupOdds({ projections }: StanleyCupOddsProps) {
  const { data, isLoading, isError } = useRaceOdds();

  if (isLoading) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)] uppercase tracking-wider">
        Running simulation…
      </p>
    );
  }
  if (isError || !data || data.nhlTeams.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)]">
        Cup odds aren't available yet — the first-round bracket needs to be
        published for the simulation to have a starting state.
      </p>
    );
  }

  // Index the projections by NHL abbrev for series-state lookups.
  const seriesByAbbrev = new Map<string, TeamSeriesProjection>();
  for (const p of projections) {
    seriesByAbbrev.set(p.teamAbbrev, p);
  }

  return (
    <div>
      <p className="text-[11px] text-[var(--color-ink-muted)] mb-3 leading-relaxed">
        Monte Carlo, {data.trials.toLocaleString()} bracket trials. Team
        strength blends regular-season Elo, every completed playoff game
        (dynamic replay), each team's starting-goalie SV%, and the home/road
        split. Round-depth mean reversion damps compounding confidence on
        deep bracket paths. Re-run every morning.
      </p>
      <div className="border border-[var(--color-divider)] overflow-hidden">
        {/* Header */}
        <div className="grid grid-cols-[minmax(0,1fr)_4.5rem_3.5rem_3.5rem_3rem] md:grid-cols-[minmax(0,1fr)_6rem_4rem_4rem_4rem_4rem] items-center gap-2 px-3 py-2 bg-[var(--color-surface-sunk)] text-[10px] uppercase tracking-widest text-[var(--color-ink-muted)] font-bold border-b border-[var(--color-divider)]">
          <span>Team</span>
          <span className="text-right">Series</span>
          <span className="text-right">Win R1</span>
          <span className="text-right hidden md:block">Final</span>
          <span className="text-right">Cup</span>
          <span className="text-right">Games</span>
        </div>
        <ol>
          {data.nhlTeams.map((team) => {
            const series = seriesByAbbrev.get(team.abbrev);
            const isInactive = !series && team.cupWinProb <= 0;
            return (
              <li
                key={team.abbrev}
                className={`grid grid-cols-[minmax(0,1fr)_4.5rem_3.5rem_3.5rem_3rem] md:grid-cols-[minmax(0,1fr)_6rem_4rem_4rem_4rem_4rem] items-center gap-2 px-3 py-3 sm:py-4 border-b border-[var(--color-divider)] last:border-b-0 ${
                  isInactive ? "bg-[#F3F4F6] opacity-60" : ""
                }`}
              >
                <TeamCell abbrev={team.abbrev} inactive={isInactive} />
                <span
                  className={`text-right text-xs tabular-nums ${
                    isInactive
                      ? "text-[var(--color-ink-muted)] font-bold uppercase tracking-wider"
                      : "text-[var(--color-ink-muted)]"
                  }`}
                >
                  {series ? seriesLabel(series) : isInactive ? "Out" : "Awaiting"}
                </span>
                <OddsCell value={team.advanceRound1Prob} inactive={isInactive} />
                <OddsCell
                  value={team.cupFinalsProb}
                  hiddenOnMobile
                  inactive={isInactive}
                />
                <OddsCell value={team.cupWinProb} emphasis inactive={isInactive} />
                <span
                  className={`text-right text-xs tabular-nums ${
                    isInactive
                      ? "text-[var(--color-ink-muted)]"
                      : "text-[var(--color-ink-muted)]"
                  }`}
                >
                  {team.expectedGames.toFixed(1)}
                </span>
              </li>
            );
          })}
        </ol>
      </div>
    </div>
  );
}

function seriesLabel(series: TeamSeriesProjection): string {
  return `${series.wins}-${series.opponentWins} vs ${series.opponentAbbrev}`;
}

function TeamCell({
  abbrev,
  inactive,
}: {
  abbrev: string;
  inactive?: boolean;
}) {
  return (
    <div className="flex items-center gap-3 min-w-0">
      <img
        src={getNHLTeamLogoUrl(abbrev)}
        alt={abbrev}
        className={`w-10 h-10 sm:w-12 sm:h-12 flex-shrink-0 ${
          inactive ? "grayscale" : ""
        }`}
      />
      <div className="min-w-0 flex-1">
        <p
          className={`text-sm font-bold uppercase tracking-wider truncate ${
            inactive ? "text-[var(--color-ink-muted)]" : "text-[#1A1A1A]"
          }`}
        >
          <span className="md:hidden">{abbrev}</span>
          <span className="hidden md:inline">{getNHLTeamShortName(abbrev)}</span>
        </p>
      </div>
    </div>
  );
}

function OddsCell({
  value,
  emphasis,
  hiddenOnMobile,
  inactive,
}: {
  value: number;
  emphasis?: boolean;
  hiddenOnMobile?: boolean;
  inactive?: boolean;
}) {
  const pct = Math.round(value * 100);
  return (
    <span
      className={`text-right tabular-nums ${
        inactive
          ? "text-xs text-[var(--color-ink-muted)]"
          : emphasis
          ? "text-sm font-extrabold text-[#1A1A1A]"
          : "text-xs text-[var(--color-ink-muted)]"
      } ${hiddenOnMobile ? "hidden md:inline" : ""}`}
    >
      {pct}%
    </span>
  );
}
