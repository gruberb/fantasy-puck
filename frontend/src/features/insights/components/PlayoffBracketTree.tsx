import { getNHLTeamLogoUrl, getNHLTeamShortName } from "@/utils/nhlTeams";

import type {
  SeriesStateCode,
  TeamSeriesProjection,
} from "@/features/insights";
import { RosteredChips } from "./RosteredChips";

interface PlayoffBracketTreeProps {
  projections: TeamSeriesProjection[];
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

/**
 * Bracket view of the active playoff round. Shows score, state color, who's
 * favored by regular-season strength, and which fantasy teams own players
 * on each side. Replaces the old per-team "Series Projections" grid, which
 * was a lookup-table rephrasing of information already on the scoreboard.
 */
export function PlayoffBracketTree({ projections }: PlayoffBracketTreeProps) {
  const matchups = pairMatchups(projections);
  if (matchups.length === 0) {
    return (
      <p className="text-xs text-[var(--color-ink-muted)]">
        Bracket will fill in once the first-round matchups are published.
      </p>
    );
  }
  return <BracketList matchups={matchups} />;
}


// ----------------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------------

interface Matchup {
  top: TeamSeriesProjection;
  bottom: TeamSeriesProjection;
}

function BracketList({ matchups }: { matchups: Matchup[] }) {
  return (
    <ol className="grid grid-cols-1 lg:grid-cols-2 gap-3">
      {matchups.map((m) => (
        <li
          key={`${m.top.teamAbbrev}-${m.bottom.teamAbbrev}`}
          className="border-2 border-[#1A1A1A] bg-white"
        >
          <MatchupRow projection={m.top} opponent={m.bottom} />
          <div className="h-px bg-[#1A1A1A]" />
          <MatchupRow projection={m.bottom} opponent={m.top} />
        </li>
      ))}
    </ol>
  );
}

/**
 * The projections list contains one entry per team — two entries per series.
 * Pair them into matchups by (team, opponent) identity. When team ratings
 * are available, put the higher-rated team on top for consistent scanning;
 * otherwise fall back to alphabetical for stability.
 */
function pairMatchups(projections: TeamSeriesProjection[]): Matchup[] {
  const seen = new Set<string>();
  const out: Matchup[] = [];
  const byAbbrev = new Map(projections.map((p) => [p.teamAbbrev, p]));
  for (const p of projections) {
    if (seen.has(p.teamAbbrev)) continue;
    const opponent = byAbbrev.get(p.opponentAbbrev);
    if (!opponent) continue;
    seen.add(p.teamAbbrev);
    seen.add(opponent.teamAbbrev);

    const [top, bottom] = orderMatchup(p, opponent);
    out.push({ top, bottom });
  }
  return out;
}

function orderMatchup(
  a: TeamSeriesProjection,
  b: TeamSeriesProjection,
): [TeamSeriesProjection, TeamSeriesProjection] {
  if (a.teamRating != null && b.teamRating != null) {
    if (a.teamRating > b.teamRating) return [a, b];
    if (b.teamRating > a.teamRating) return [b, a];
  }
  return a.teamAbbrev.localeCompare(b.teamAbbrev) < 0 ? [a, b] : [b, a];
}

function MatchupRow({
  projection,
  opponent,
}: {
  projection: TeamSeriesProjection;
  opponent: TeamSeriesProjection;
}) {
  const isLeading = projection.wins > opponent.wins;
  const strengthTag = strengthLabel(projection, opponent);
  const rating = projection.teamRating;

  return (
    <div
      className={`flex items-center gap-3 px-3 py-2.5 ${
        isLeading ? "bg-[#FAFAFA]" : ""
      }`}
    >
      <img
        src={getNHLTeamLogoUrl(projection.teamAbbrev)}
        alt={projection.teamAbbrev}
        className="w-7 h-7 flex-shrink-0"
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <p className="text-xs font-bold uppercase tracking-wider truncate text-[#1A1A1A]">
            {getNHLTeamShortName(projection.teamAbbrev)}
          </p>
          {strengthTag && (
            <span
              className={`text-[9px] uppercase tracking-widest font-bold px-1.5 py-0.5 ${strengthTag.className}`}
            >
              {strengthTag.label}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 mt-0.5 text-[10px] text-[var(--color-ink-muted)]">
          {rating != null && <StrengthBadge value={rating} />}
          {projection.rosteredTags.length > 0 && (
            <RosteredChips tags={projection.rosteredTags} />
          )}
        </div>
      </div>
      <span className="font-extrabold text-xl tabular-nums w-6 text-right text-[#1A1A1A]">
        {projection.wins}
      </span>
      <span
        className={`text-[10px] uppercase tracking-wider font-bold px-1.5 py-0.5 ${
          STATE_STYLES[projection.seriesState]
        }`}
      >
        {Math.round(projection.oddsToAdvance * 100)}%
      </span>
    </div>
  );
}

/**
 * Derive a "favored / even / underdog" tag from regular-season standings
 * points. Threshold of 5 points produces sensible labels for most NHL
 * seeding spreads (top seeds typically sit 10-30 pts above wildcards).
 * Returns null when we don't have both ratings.
 */
function strengthLabel(
  team: TeamSeriesProjection,
  opponent: TeamSeriesProjection,
): { label: string; className: string } | null {
  if (team.teamRating == null || opponent.teamRating == null) return null;
  const diff = team.teamRating - opponent.teamRating;
  if (Math.abs(diff) < 5) {
    return {
      label: "Even",
      className: "bg-[var(--color-divider)] text-[#1A1A1A]",
    };
  }
  return diff > 0
    ? {
        label: "Favored",
        className: "bg-[#1A1A1A] text-white",
      }
    : {
        label: "Underdog",
        className: "bg-[var(--color-rival)] text-white",
      };
}

/**
 * Numeric team strength with an (i) affordance that explains the blend.
 * Native `title` attribute handles the tooltip — zero JS, keyboard-accessible,
 * works on touch (tap & hold). Not brutalist-styled because native tooltips
 * are OS-rendered, but the (i) glyph itself lives inside the design system.
 */
function StrengthBadge({ value }: { value: number }) {
  const description =
    "Team strength rating. Blend of regular-season standings points (70%) and last-10-games pace extrapolated to 82 games (30%). Used as the per-game win-probability prior in the Monte Carlo simulation.";
  return (
    <span className="inline-flex items-center gap-1 tabular-nums">
      <span className="uppercase tracking-widest text-[9px] font-bold text-[var(--color-ink-muted)]">
        Strength
      </span>
      <span className="text-[#1A1A1A] font-bold">{Math.round(value)}</span>
      <span
        className="inline-flex items-center justify-center w-3 h-3 border border-[var(--color-ink-muted)] text-[8px] font-bold text-[var(--color-ink-muted)] cursor-help leading-none"
        role="img"
        aria-label={description}
        title={description}
      >
        i
      </span>
    </span>
  );
}
