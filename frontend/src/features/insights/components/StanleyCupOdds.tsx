import { useRaceOdds } from "@/features/race-odds/hooks/use-race-odds";
import {
  getNHLTeamLogoUrl,
  getNHLTeamShortName,
} from "@/utils/nhlTeams";

import type { TeamSeriesProjection } from "@/features/insights";
import { RosteredChips } from "./RosteredChips";

interface StanleyCupOddsProps {
  /**
   * Current-round series context keyed by NHL abbrev, used to annotate each
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
        Monte Carlo, {data.trials.toLocaleString()} bracket trials · team
        strength from regular-season standings points · current series state
        as the starting condition · re-run every morning · calibrated against
        HockeyStats.com round-1 reference odds within ~3pp. The model
        underweights goalie quality and injuries, so elite favorites may be
        modestly understated vs. the sharpest external models.
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
            return (
              <li
                key={team.abbrev}
                className="grid grid-cols-[minmax(0,1fr)_4.5rem_3.5rem_3.5rem_3rem] md:grid-cols-[minmax(0,1fr)_6rem_4rem_4rem_4rem_4rem] items-center gap-2 px-3 py-2 border-b border-[var(--color-divider)] last:border-b-0"
              >
                <TeamCell abbrev={team.abbrev} series={series} />
                <span className="text-right text-xs text-[var(--color-ink-muted)] tabular-nums">
                  {series
                    ? `${series.wins}-${series.opponentWins} vs ${series.opponentAbbrev}`
                    : "—"}
                </span>
                <OddsCell value={team.advanceRound1Prob} />
                <OddsCell value={team.cupFinalsProb} hiddenOnMobile />
                <OddsCell value={team.cupWinProb} emphasis />
                <span className="text-right text-xs text-[var(--color-ink-muted)] tabular-nums">
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

function TeamCell({
  abbrev,
  series,
}: {
  abbrev: string;
  series: TeamSeriesProjection | undefined;
}) {
  return (
    <div className="flex items-center gap-2 min-w-0">
      <img
        src={getNHLTeamLogoUrl(abbrev)}
        alt={abbrev}
        className="w-5 h-5 flex-shrink-0"
      />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-bold uppercase tracking-wider truncate text-[#1A1A1A]">
          <span className="md:hidden">{abbrev}</span>
          <span className="hidden md:inline">{getNHLTeamShortName(abbrev)}</span>
        </p>
        {series && series.rosteredTags.length > 0 && (
          <div className="mt-0.5">
            <RosteredChips tags={series.rosteredTags} />
          </div>
        )}
      </div>
    </div>
  );
}

function OddsCell({
  value,
  emphasis,
  hiddenOnMobile,
}: {
  value: number;
  emphasis?: boolean;
  hiddenOnMobile?: boolean;
}) {
  const pct = Math.round(value * 100);
  return (
    <span
      className={`text-right tabular-nums ${
        emphasis
          ? "text-sm font-extrabold text-[#1A1A1A]"
          : "text-xs text-[var(--color-ink-muted)]"
      } ${hiddenOnMobile ? "hidden md:inline" : ""}`}
    >
      {pct}%
    </span>
  );
}

