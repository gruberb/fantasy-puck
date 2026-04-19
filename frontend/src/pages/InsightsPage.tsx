import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import { useInsights } from "@/features/insights";
// RaceOddsSection is a Pulse surface (personal/league projections); Insights
// stays NHL-centric. The Fantasy Champion leaderboard (no-league view) lives
// alongside the Stanley Cup view instead.
import { FantasyChampionBoard } from "@/features/race-odds/components/FantasyChampionBoard";
import { useRaceOdds } from "@/features/race-odds/hooks/use-race-odds";
import { PlayoffBracketTree } from "@/features/insights/components/PlayoffBracketTree";
import { StanleyCupOdds } from "@/features/insights/components/StanleyCupOdds";
import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import {
  getNHLTeamShortName,
  getNHLTeamLogoUrl,
  getNHLTeamUrlSlug,
  nhlPlayerProfileUrl,
} from "@/utils/nhlTeams";
import type {
  HotPlayerSignal,
  LastNightGame,
  PlayerLeader,
  TodaysGameSignal,
} from "@/features/insights";

const InsightsPage = () => {
  const { insights, isLoading, error, refetch } = useInsights();
  // Pull the global Fantasy Champion leaderboard — Insights shows this when
  // there's no active league; personal race/rivalry content lives on Pulse.
  const { data: raceOdds } = useRaceOdds();

  if (isLoading) {
    return <LoadingSpinner size="large" message="Generating insights..." />;
  }

  if (error || !insights) {
    return (
      <ErrorMessage
        message="Failed to load insights. Please try again."
        onRetry={() => refetch()}
      />
    );
  }

  const { narratives, signals } = insights;

  return (
    <div className="space-y-6">

      {/* Last Night — Daily Faceoff-style recap of the previous hockey-day's
          games. Rendered first so a morning visitor's first glance answers
          "what did I miss overnight" before being steered into the preview. */}
      {(signals.lastNight.length > 0 || narratives.lastNight) && (
        <InsightCard accent="#1A1A1A" title="Last Night">
          {signals.lastNight.length > 0 && (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 mb-4">
              {signals.lastNight.map((g, i) => (
                <LastNightCard key={i} game={g} />
              ))}
            </div>
          )}
          {narratives.lastNight && (
            <MarkdownNarrative text={narratives.lastNight} />
          )}
        </InsightCard>
      )}

      {/* What to Watch Today */}
      {(narratives.todaysWatch || signals.todaysGames.length > 0) && (
        <InsightCard accent="#2563EB" title="What to Watch Today">
          <Narrative text={narratives.todaysWatch} />
          {signals.todaysGames.length > 0 && (
            <div className="mt-4 grid grid-cols-1 lg:grid-cols-2 gap-4">
              {signals.todaysGames.map((game, i) => (
                <GameSignalCard
                  key={i}
                  game={game}
                  narrative={narratives.gameNarratives?.[i]}
                />
              ))}
            </div>
          )}
        </InsightCard>
      )}

      {/* Hot Hands + Cold Hands — stacked rows so each gets full width and
          cards don't clip against a half-width column. */}
      {(narratives.hotPlayers ||
        signals.hotPlayers.length > 0 ||
        signals.coldHands.length > 0) && (
        <InsightCard accent="#EF4444" title="Hot + Cold">
          <Narrative text={narratives.hotPlayers} />
          <div className="mt-4 space-y-5">
            <div>
              <p className="text-[10px] uppercase tracking-widest text-[#DC2626] font-bold mb-2">
                Hot
              </p>
              {signals.hotPlayers.length === 0 ? (
                <p className="text-xs text-gray-400">
                  Playoffs haven't produced data yet — check back after
                  tonight's games.
                </p>
              ) : (
                <div className="flex gap-3 overflow-x-auto pb-2 items-stretch">
                  {signals.hotPlayers.map((p, i) => (
                    <HotPlayerCard key={i} player={p} rank={i + 1} tone="hot" />
                  ))}
                </div>
              )}
            </div>
            <div>
              <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold mb-2">
                Cold (rostered)
              </p>
              {signals.coldHands.length === 0 ? (
                <p className="text-xs text-gray-400">
                  No one slumping yet — everyone's earning their keep.
                </p>
              ) : (
                <div className="flex gap-3 overflow-x-auto pb-2 items-stretch">
                  {signals.coldHands.map((p, i) => (
                    <HotPlayerCard key={i} player={p} rank={i + 1} tone="cold" />
                  ))}
                </div>
              )}
            </div>
          </div>
        </InsightCard>
      )}


      {/* Bracket — matchup-focused view of the active round. */}
      {signals.seriesProjections.length > 0 && (
        <InsightCard accent="#1A1A1A" title="Bracket">
          <p className="text-[11px] text-[var(--color-ink-muted)] mb-3">
            Score + historical odds · team strength from regular-season
            standings · fantasy-team ownership on each side.
          </p>
          <PlayoffBracketTree projections={signals.seriesProjections} />
        </InsightCard>
      )}

      {/* Stanley Cup Odds — championship-focused ranked list driven by the
          same Monte Carlo that powers Pulse's race odds. Re-runs daily. */}
      {signals.seriesProjections.length > 0 && (
        <InsightCard accent="#1A1A1A" title="Stanley Cup Odds">
          <StanleyCupOdds projections={signals.seriesProjections} />
        </InsightCard>
      )}

      {/* Fantasy Champion — top NHL skaters by projected playoff points;
          only rendered when there's no active league (global view). */}
      {raceOdds?.mode === "champion" && raceOdds.championLeaderboard.length > 0 && (
        <InsightCard accent="#1A1A1A" title="Fantasy Champion">
          <p className="text-[11px] text-[var(--color-ink-muted)] mb-3">
            Top skaters by projected playoff fantasy points across{" "}
            {raceOdds.trials.toLocaleString()} bracket simulations.
          </p>
          <FantasyChampionBoard players={raceOdds.championLeaderboard} />
        </InsightCard>
      )}

      {/* News Headlines */}
      {signals.newsHeadlines.length > 0 && (
        <InsightCard accent="#1A1A1A" title="Around the League">
          <ul className="space-y-2 mt-2">
            {signals.newsHeadlines.map((headline, i) => (
              <li key={i} className="text-sm text-gray-700 flex items-start gap-2">
                <span className="text-gray-400 mt-0.5 flex-shrink-0">&bull;</span>
                {headline}
              </li>
            ))}
          </ul>
        </InsightCard>
      )}
    </div>
  );
};

// -- Sub-components ----------------------------------------------------------

function InsightCard({
  accent,
  title,
  children,
}: {
  accent: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
      <div className="px-6 py-3 border-b-2 border-[#1A1A1A]" style={{ backgroundColor: accent }}>
        <h2 className="font-extrabold text-white uppercase tracking-wider text-sm">
          {title}
        </h2>
      </div>
      <div className="p-6">{children}</div>
    </div>
  );
}

/**
 * Per-game card for Last Night: headline, final score, series state,
 * and up to three top scorers with point totals. Purely presentational;
 * the narrative paragraph below contextualises these numbers.
 */
function LastNightCard({ game }: { game: LastNightGame }) {
  return (
    <div className="border-2 border-gray-200 overflow-hidden">
      <div className="px-3 py-2 bg-gray-50 border-b-2 border-gray-200">
        <p className="font-extrabold uppercase tracking-wider text-xs text-[#1A1A1A]">
          {game.headline}
        </p>
        <p className="text-[11px] text-gray-500 font-medium">
          {game.awayTeam} {game.awayScore} – {game.homeTeam} {game.homeScore}
          {game.seriesAfter ? ` · ${game.seriesAfter}` : ""}
        </p>
      </div>
      {game.topScorers.length > 0 && (
        <ul className="divide-y divide-gray-100 text-xs">
          {game.topScorers.map((s, i) => (
            <li key={i} className="flex justify-between items-center px-3 py-1.5">
              <span className="truncate pr-2">
                <span className="font-bold text-[#1A1A1A]">{s.name}</span>{" "}
                <span className="text-gray-400">({s.team})</span>
              </span>
              <span className="tabular-nums text-gray-600">
                {s.goals}G {s.assists}A · <strong className="text-[#1A1A1A]">{s.points} pts</strong>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * Renders text with a tiny markdown subset: `### Heading`, blank-line
 * paragraph breaks, and `**bold**`. Used by the Last Night section,
 * which gets per-game sub-headings from Claude. Unknown markdown lines
 * fall through as plain paragraphs.
 */
function MarkdownNarrative({ text }: { text: string }) {
  type Block =
    | { kind: "heading"; text: string }
    | { kind: "paragraph"; text: string };
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ").trim() });
      paragraph = [];
    }
  };
  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    if (line === "") { flush(); continue; }
    if (line.startsWith("### ")) {
      flush();
      blocks.push({ kind: "heading", text: line.slice(4).trim() });
      continue;
    }
    paragraph.push(line);
  }
  flush();
  return (
    <div className="space-y-3">
      {blocks.map((b, i) =>
        b.kind === "heading" ? (
          <h3
            key={i}
            className={`font-extrabold uppercase tracking-wider text-xs text-[#1A1A1A] ${
              i === 0 ? "" : "pt-3 border-t border-gray-200"
            }`}
          >
            {b.text}
          </h3>
        ) : (
          <Narrative key={i} text={b.text} />
        )
      )}
    </div>
  );
}

/** Renders text with **bold** markers parsed into <strong> tags */
function Narrative({ text }: { text: string }) {
  if (!text) return null;
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return (
    <p className="text-sm leading-relaxed text-gray-800">
      {parts.map((part, i) => {
        if (part.startsWith("**") && part.endsWith("**")) {
          return <strong key={i} className="font-bold text-[#1A1A1A]">{part.slice(2, -2)}</strong>;
        }
        return <span key={i}>{part}</span>;
      })}
    </p>
  );
}

function GameSignalCard({ game, narrative }: { game: TodaysGameSignal; narrative?: string }) {
  return (
    <div className={`border-2 overflow-hidden ${game.isElimination ? "border-red-400" : "border-gray-200"}`}>
      {/* Elimination banner */}
      {game.isElimination && (
        <div className="bg-red-600 px-4 py-1 text-center">
          <span className="text-[10px] font-bold uppercase tracking-wider text-white">Elimination Game</span>
        </div>
      )}

      {/* Header: Teams stacked vertically */}
      <div className={`px-4 py-3 ${game.isElimination ? "bg-red-50" : "bg-gray-50"}`}>
        {/* Away team */}
        <div className="flex items-center gap-3">
          <a
            href={`https://www.nhl.com/${getNHLTeamUrlSlug(game.awayTeam)}`}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-3 flex-1 min-w-0 hover:opacity-80"
            aria-label={`${getNHLTeamShortName(game.awayTeam)} on NHL.com`}
          >
            <img src={getNHLTeamLogoUrl(game.awayTeam)} alt={game.awayTeam} className="w-8 h-8 flex-shrink-0" />
            <div className="flex-1 min-w-0">
              <p className="font-extrabold text-sm uppercase tracking-wider leading-tight">{getNHLTeamShortName(game.awayTeam)}</p>
              {game.awayRecord && <p className="text-[11px] text-gray-400">{game.awayRecord}</p>}
            </div>
          </a>
          <div className="flex items-center gap-2 flex-shrink-0">
            {game.awayStreak && (
              <span className={`text-[10px] font-bold px-1.5 py-0.5 leading-none ${game.awayStreak.startsWith("W") ? "bg-green-100 text-green-700" : "bg-red-100 text-red-700"}`}>
                {formatStreak(game.awayStreak)}
              </span>
            )}
            {game.awayL10 && <span className="text-[10px] text-gray-400 tabular-nums">L10: {game.awayL10}</span>}
          </div>
        </div>

        {/* Venue divider */}
        <div className="flex items-center gap-3 my-2.5">
          <div className="flex-1 border-t border-gray-200" />
          <span className="text-[10px] text-gray-300 font-bold uppercase tracking-wider flex-shrink-0">@ {game.venue}</span>
          <div className="flex-1 border-t border-gray-200" />
        </div>

        {/* Home team */}
        <div className="flex items-center gap-3">
          <a
            href={`https://www.nhl.com/${getNHLTeamUrlSlug(game.homeTeam)}`}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-3 flex-1 min-w-0 hover:opacity-80"
            aria-label={`${getNHLTeamShortName(game.homeTeam)} on NHL.com`}
          >
            <img src={getNHLTeamLogoUrl(game.homeTeam)} alt={game.homeTeam} className="w-8 h-8 flex-shrink-0" />
            <div className="flex-1 min-w-0">
              <p className="font-extrabold text-sm uppercase tracking-wider leading-tight">{getNHLTeamShortName(game.homeTeam)}</p>
              {game.homeRecord && <p className="text-[11px] text-gray-400">{game.homeRecord}</p>}
            </div>
          </a>
          <div className="flex items-center gap-2 flex-shrink-0">
            {game.homeStreak && (
              <span className={`text-[10px] font-bold px-1.5 py-0.5 leading-none ${game.homeStreak.startsWith("W") ? "bg-green-100 text-green-700" : "bg-red-100 text-red-700"}`}>
                {formatStreak(game.homeStreak)}
              </span>
            )}
            {game.homeL10 && <span className="text-[10px] text-gray-400 tabular-nums">L10: {game.homeL10}</span>}
          </div>
        </div>

        {game.seriesContext && (
          <p className="text-[10px] text-gray-400 mt-2 text-center font-medium">{game.seriesContext}</p>
        )}

        {game.rosteredPlayerTags && game.rosteredPlayerTags.length > 0 && (
          <RosteredStakesTable tags={game.rosteredPlayerTags} />
        )}
      </div>

      {/* Per-game narrative */}
      {narrative && (
        <div className="px-4 py-3 border-t border-gray-100">
          <Narrative text={narrative} />
        </div>
      )}

      {/* Players to Watch - head-to-head comparison */}
      {game.pointsLeaders && (
        <div className="px-4 py-3 border-t border-gray-100">
          <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-3">
            Players to Watch &middot; Last 5
          </p>
          {[
            { label: "Points", leaders: game.pointsLeaders },
            { label: "Goals", leaders: game.goalsLeaders },
            { label: "Assists", leaders: game.assistsLeaders },
          ].filter(r => r.leaders).map(({ label, leaders }) => {
            const away = leaders![0];
            const home = leaders![1];
            const total = away.value + home.value;
            const awayPct = total > 0 ? (away.value / total) * 100 : 50;
            return (
              <div key={label} className="mb-4 last:mb-0">
                <div className="flex items-center gap-2">
                  <LeaderCell leader={away} align="left" />
                  <div className="flex items-baseline gap-1.5 flex-shrink-0">
                    <span className="font-bold text-base tabular-nums text-right w-5">{away.value}</span>
                    <span className="text-[10px] text-gray-400 uppercase w-12 text-center">{label}</span>
                    <span className="font-bold text-base tabular-nums w-5">{home.value}</span>
                  </div>
                  <LeaderCell leader={home} align="right" />
                </div>
                <div className="flex h-1 mt-1.5 gap-px">
                  <div className="bg-[#1A1A1A]" style={{ width: `${awayPct}%` }} />
                  <div className="bg-gray-300" style={{ width: `${100 - awayPct}%` }} />
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Goalies - side by side comparison */}
      {(game.awayGoalie || game.homeGoalie) && (
        <div className="px-4 py-3 border-t border-gray-100">
          <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-2">Goalies</p>
          <div className="grid grid-cols-2 gap-3">
            {game.awayGoalie && (
              <div>
                <p className="text-xs font-bold">{game.awayGoalie.name}</p>
                <p className="text-[11px] text-gray-500 tabular-nums">{game.awayGoalie.record}</p>
                <p className="text-[11px] text-gray-500 tabular-nums">{game.awayGoalie.gaa.toFixed(2)} GAA &middot; .{Math.round(game.awayGoalie.savePctg * 1000)} SV%</p>
              </div>
            )}
            {game.homeGoalie && (
              <div className="text-right">
                <p className="text-xs font-bold">{game.homeGoalie.name}</p>
                <p className="text-[11px] text-gray-500 tabular-nums">{game.homeGoalie.record}</p>
                <p className="text-[11px] text-gray-500 tabular-nums">{game.homeGoalie.gaa.toFixed(2)} GAA &middot; .{Math.round(game.homeGoalie.savePctg * 1000)} SV%</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Compact two-column table of fantasy teams with players in the current
 * game, sorted by player count desc (backend already sorts). Replaces the
 * earlier yellow-chip cluster, which became unreadable in leagues where
 * ten-plus teams own players across the matchup.
 *
 * Each row links to the fantasy team detail page inside the active
 * league. Outside a league context (global /insights route) the rows
 * render as plain text since there's no team route to deep-link to.
 */
function RosteredStakesTable({
  tags,
}: {
  tags: { fantasyTeamId: number; fantasyTeamName: string; count: number }[];
}) {
  const { activeLeagueId } = useLeague();
  const total = tags.reduce((acc, t) => acc + t.count, 0);
  const max = tags[0]?.count ?? 1;
  return (
    <div className="mt-3 -mx-4 border-t-2 border-[#1A1A1A] bg-[#FACC15]/10">
      <div className="flex items-baseline justify-between px-4 py-1.5 border-b border-[#1A1A1A]/10">
        <p className="text-[9px] font-bold uppercase tracking-widest text-[#1A1A1A]">
          Fantasy Stakes
        </p>
        <p className="text-[9px] font-bold tabular-nums text-[#1A1A1A]/70">
          {tags.length} {tags.length === 1 ? "team" : "teams"} &middot; {total}{" "}
          {total === 1 ? "player" : "players"}
        </p>
      </div>
      <ul className="grid grid-cols-2 gap-x-4 px-4 py-2">
        {tags.map((tag) => {
          const pct = Math.round((tag.count / max) * 100);
          const name = (
            <span className="flex-1 truncate font-medium uppercase tracking-wider text-[#1A1A1A]">
              {tag.fantasyTeamName}
            </span>
          );
          const row = (
            <>
              {activeLeagueId ? (
                <Link
                  to={`/league/${activeLeagueId}/teams/${tag.fantasyTeamId}`}
                  className="flex-1 truncate font-medium uppercase tracking-wider text-[#1A1A1A] hover:underline"
                >
                  {tag.fantasyTeamName}
                </Link>
              ) : (
                name
              )}
              <span className="relative h-1.5 w-10 bg-[#FACC15]/25">
                <span
                  className="absolute inset-y-0 left-0 bg-[#FACC15]"
                  style={{ width: `${pct}%` }}
                />
              </span>
              <span className="w-4 text-right tabular-nums font-bold text-[#1A1A1A]">
                {tag.count}
              </span>
            </>
          );
          return (
            <li
              key={tag.fantasyTeamId}
              className="flex items-center gap-2 py-0.5 text-[10px]"
            >
              {row}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/**
 * One side of the "Players to Watch" matchup row. Wraps the headshot
 * and name in an nhl.com link when the leader carries a `playerId`,
 * falling back to plain text when the pre-game landing payload omitted
 * it (rare but the DTO keeps the field optional).
 */
function LeaderCell({
  leader,
  align,
}: {
  leader: PlayerLeader;
  align: "left" | "right";
}) {
  const headshot = (
    <img
      src={leader.headshot}
      alt=""
      className="w-7 h-7 rounded-full bg-gray-100 flex-shrink-0"
      onError={(e) => {
        (e.target as HTMLImageElement).style.display = "none";
      }}
    />
  );
  const name = (
    <span
      className={`text-xs font-medium truncate ${align === "right" ? "text-right" : ""}`}
    >
      {leader.name}
    </span>
  );
  const wrapperClass = `flex items-center gap-1.5 flex-1 min-w-0 ${
    align === "right" ? "justify-end" : ""
  }`;
  if (!leader.playerId) {
    return (
      <div className={wrapperClass}>
        {align === "left" ? (
          <>
            {headshot}
            {name}
          </>
        ) : (
          <>
            {name}
            {headshot}
          </>
        )}
      </div>
    );
  }
  return (
    <a
      href={nhlPlayerProfileUrl(leader.playerId)}
      target="_blank"
      rel="noopener noreferrer"
      className={`${wrapperClass} hover:opacity-80`}
      aria-label={`${leader.name} on NHL.com`}
    >
      {align === "left" ? (
        <>
          {headshot}
          {name}
        </>
      ) : (
        <>
          {name}
          {headshot}
        </>
      )}
    </a>
  );
}

function HotPlayerCard({
  player,
  rank,
  tone = "hot",
}: {
  player: HotPlayerSignal;
  rank: number;
  tone?: "hot" | "cold";
}) {
  const badgeClass =
    tone === "hot"
      ? "bg-red-100 text-red-700"
      : "bg-gray-200 text-gray-600";
  const borderClass = tone === "hot" ? "border-gray-200" : "border-gray-300";
  const hasEdge = player.topSpeed != null || player.topShotSpeed != null;
  return (
    <div
      // flex-col + min-h locks every card to the same height regardless of
      // whether edge data or a fantasy-team footer is present; mt-auto on
      // the footer pushes it to the bottom so cards line up across the row.
      className={`flex-shrink-0 w-40 min-h-[230px] p-3 border-2 ${borderClass} rounded-none bg-white flex flex-col`}
    >
      <div className="flex items-center gap-2 mb-2">
        <span
          className={`w-5 h-5 flex items-center justify-center ${badgeClass} font-bold text-xs`}
        >
          {rank}
        </span>
        {player.imageUrl && (
          <a
            href={nhlPlayerProfileUrl(player.nhlId)}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={`${player.name} — NHL profile`}
          >
            <img
              src={player.imageUrl}
              alt={player.name}
              className="w-8 h-8 rounded-none bg-gray-200"
            />
          </a>
        )}
      </div>
      <a
        href={nhlPlayerProfileUrl(player.nhlId)}
        target="_blank"
        rel="noopener noreferrer"
        className="font-bold text-sm truncate block hover:underline"
      >
        {player.name}
      </a>
      <div className="flex items-center gap-1 text-xs text-gray-500">
        <span>{player.position}</span>
        <span>&bull;</span>
        <img src={getNHLTeamLogoUrl(player.nhlTeam)} alt={player.nhlTeam} className="w-4 h-4" />
        <span>{getNHLTeamShortName(player.nhlTeam)}</span>
      </div>
      <div className="mt-2 grid grid-cols-3 gap-1 text-center">
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">G</div>
          <div className="font-bold text-xs">{player.formGoals}</div>
        </div>
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">A</div>
          <div className="font-bold text-xs">{player.formAssists}</div>
        </div>
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">PTS</div>
          <div className="font-bold text-xs">{player.formPoints}</div>
        </div>
      </div>
      {player.playoffPoints > 0 && (
        <p className="mt-1.5 text-[10px] text-gray-500 tabular-nums">
          <span className="font-bold text-[#1A1A1A]">{player.playoffPoints}</span>{" "}
          playoff pts
        </p>
      )}

      {/* Footer block pushed to the bottom with mt-auto. Both the edge-data
          row and the fantasy-team line are always rendered so the card
          height is stable; missing values fall back to an invisible
          placeholder. */}
      <div className="mt-auto pt-2">
        {hasEdge ? (
          <div className="flex gap-1 text-center">
            {player.topSpeed != null && (
              <div className="flex-1 bg-blue-50 p-1">
                <div className="text-[10px] text-blue-400">SKATE</div>
                <div className="font-bold text-[10px] text-blue-700">
                  {player.topSpeed.toFixed(1)}
                  <span className="text-blue-400 font-normal"> mph</span>
                </div>
              </div>
            )}
            {player.topShotSpeed != null && (
              <div className="flex-1 bg-blue-50 p-1">
                <div className="text-[10px] text-blue-400">SHOT</div>
                <div className="font-bold text-[10px] text-blue-700">
                  {player.topShotSpeed.toFixed(1)}
                  <span className="text-blue-400 font-normal"> mph</span>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="h-[1.75rem]" aria-hidden />
        )}
        <div className="mt-2 min-h-[1.25rem]">
          {player.fantasyTeam && (
            <span className="inline-flex items-center gap-1 bg-[var(--color-you-tint)] text-[#1A1A1A] px-1.5 py-0.5 text-[9px] uppercase tracking-widest font-bold">
              Roster · {player.fantasyTeam}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

/** Convert streak codes like "W2" -> "Won 2", "L5" -> "Lost 5", "OT1" -> "OT 1" */
function formatStreak(streak: string | null): string {
  if (!streak) return "";
  if (streak.startsWith("W")) return `Won ${streak.slice(1)}`;
  if (streak.startsWith("L")) return `Lost ${streak.slice(1)}`;
  if (streak.startsWith("OT")) return `OT ${streak.slice(2)}`;
  return streak;
}

export default InsightsPage;
