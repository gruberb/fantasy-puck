import { ReactNode } from "react";
import { Game } from "@/types/games";
import { NHL_TEAMS_BY_ABBREV } from "@/utils/nhlTeams";

// ── Types ──────────────────────────────────────────────────────────────────

interface NHLGameCardProps {
  game: Game;
  getTeamPrimaryColor: (teamName: string) => string;
  /** Optional expand/collapse button label. If provided, shows toggle. */
  expandLabel?: string;
  collapseLabel?: string;
  isExpanded?: boolean;
  onToggleExpand?: () => void;
  /** Content rendered when expanded (player details, game info, etc.) */
  children?: ReactNode;
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/** Split "Tampa Bay Lightning" → { city: "Tampa Bay", name: "Lightning" } */
function splitTeamName(abbrev: string): { city: string; name: string } {
  const info = NHL_TEAMS_BY_ABBREV[abbrev];
  if (!info) return { city: "", name: abbrev };

  const full = info.fullName;
  const short = info.shortName;

  if (full.endsWith(short)) {
    const city = full.slice(0, full.length - short.length).trim();
    return { city, name: short };
  }
  return { city: "", name: full };
}

// ── Diagonal Stripes SVG ────────────────────────────────────────────────────
// Mimics NHL.com's diagonal stripe pattern:
//   - Large colored wedge filling from the card edge
//   - White gap (background showing through)
//   - Thin colored diagonal stripe
//   - Small gap
//   - Thin accent stripe
// Left side renders as-is; right side is horizontally mirrored via CSS.

function DiagonalStripes({
  color,
  side,
}: {
  color: string;
  side: "left" | "right";
}) {
  // Uses CSS clip-path with percentages so the shape is always correct
  // regardless of the container's aspect ratio (no SVG distortion).
  // Both sides: wedge wider at top, narrower at bottom.
  const isLeft = side === "left";

  return (
    <div
      className={`absolute inset-y-0 ${isLeft ? "left-0" : "right-0"} hidden sm:block sm:w-[110px] md:w-[140px] overflow-hidden pointer-events-none`}
      aria-hidden="true"
    >
      {/* Main wedge */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: color,
          clipPath: isLeft
            ? "polygon(0% 0%, 70% 0%, 30% 100%, 0% 100%)"
            : "polygon(30% 0%, 100% 0%, 100% 100%, 70% 100%)",
        }}
      />
      {/* Thin stripe */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: color,
          clipPath: isLeft
            ? "polygon(76% 0%, 80% 0%, 40% 100%, 36% 100%)"
            : "polygon(20% 0%, 24% 0%, 64% 100%, 60% 100%)",
        }}
      />
    </div>
  );
}

// ── Status Badge ────────────────────────────────────────────────────────────

function StatusBadge({
  gameStatus,
  period,
  isLive,
  isGameComplete,
}: {
  gameStatus: string;
  period: string | null | undefined;
  isLive: boolean;
  isGameComplete: boolean;
}) {
  if (isLive) {
    return (
      <span className="inline-flex items-center gap-1.5 px-2 py-1 bg-red-600 text-white text-xs font-bold uppercase tracking-wider rounded-sm whitespace-nowrap">
        <span className="w-2 h-2 bg-white rounded-full animate-pulse flex-shrink-0" />
        LIVE{period ? ` · P${period.replace(/[^0-9OT]/g, "")}` : ""}
      </span>
    );
  }
  if (isGameComplete) {
    return (
      <span className="px-3 py-1 border border-gray-300 text-[#1A1A1A] text-xs font-bold uppercase tracking-wider rounded-sm">
        FINAL{period && period.toLowerCase().includes("ot") ? " / OT" : ""}
      </span>
    );
  }
  return null;
}

// ── Series Record ───────────────────────────────────────────────────────────

function getSeriesRecord(game: Game, isAwayTeam: boolean): string | null {
  if (!game.seriesStatus || !game.seriesStatus.round) return null;
  const { topSeedTeamAbbrev, topSeedWins, bottomSeedWins } = game.seriesStatus;
  const teamAbbrev = isAwayTeam ? game.awayTeam : game.homeTeam;
  const isTopSeed = topSeedTeamAbbrev === teamAbbrev;
  return isTopSeed
    ? `Series ${topSeedWins}-${bottomSeedWins}`
    : `Series ${bottomSeedWins}-${topSeedWins}`;
}

// ── Main Component ──────────────────────────────────────────────────────────

const NHLGameCard = ({
  game,
  getTeamPrimaryColor,
  expandLabel = "Show Rostered Skaters",
  collapseLabel = "Hide Rostered Skaters",
  isExpanded = false,
  onToggleExpand,
  children,
}: NHLGameCardProps) => {
  // Time formatting
  let timeString: string;
  let dateString: string;
  try {
    const gameDate = new Date(game.startTime);
    timeString = gameDate
      .toLocaleTimeString([], {
        hour: "numeric",
        minute: "2-digit",
        hour12: true,
      })
      .toUpperCase();
    dateString = gameDate.toLocaleDateString([], {
      month: "numeric",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    timeString = "TBD";
    dateString = "";
  }

  // Game status
  const gameStatus = game.gameState || "SCHEDULED";
  const isLive =
    gameStatus.toUpperCase() === "LIVE" || gameStatus.toUpperCase() === "CRIT";
  const isGameComplete = gameStatus === "FINAL" || gameStatus === "OFF";
  const hasScore =
    game.awayScore !== undefined &&
    game.awayScore !== null &&
    game.homeScore !== undefined &&
    game.homeScore !== null;

  // Team info
  const awayColor = getTeamPrimaryColor(game.awayTeam);
  const homeColor = getTeamPrimaryColor(game.homeTeam);
  const away = splitTeamName(game.awayTeam);
  const home = splitTeamName(game.homeTeam);

  return (
    <div className="bg-white rounded-none overflow-hidden border border-gray-200 shadow-sm transition-all duration-200">
      {/* Top team color bar — hidden on mobile */}
      <div className="hidden sm:flex h-1">
        <div className="flex-1" style={{ backgroundColor: awayColor }} />
        <div className="flex-1" style={{ backgroundColor: homeColor }} />
      </div>

      {/* Main matchup area */}
      <div
        className="cursor-pointer hover:bg-gray-50/30 transition-colors"
        onClick={() =>
          window.open(`https://www.nhl.com/gamecenter/${game.id}`, "_blank")
        }
      >
        {/* Matchup row with diagonal stripes */}
        <div className="relative">
          <DiagonalStripes color={awayColor} side="left" />
          <DiagonalStripes color={homeColor} side="right" />

          <div className="relative z-10 px-4 sm:px-6 py-4 sm:py-5">
            <div className="flex items-center">
              {/* ─── Away team (left) ─── */}
              <div className="flex-1 flex flex-col sm:flex-row items-center gap-1 sm:gap-5 min-w-0 pl-0 sm:pl-[65px] md:pl-[85px]">
                {game.awayTeamLogo ? (
                  <img
                    src={game.awayTeamLogo}
                    alt={`${game.awayTeam} logo`}
                    className="w-14 h-14 sm:w-20 sm:h-20 flex-shrink-0 object-contain drop-shadow-md"
                  />
                ) : (
                  <div
                    className="w-14 h-14 sm:w-20 sm:h-20 flex-shrink-0 rounded-sm flex items-center justify-center"
                    style={{ backgroundColor: `${awayColor}15` }}
                  >
                    <span
                      className="text-sm font-bold"
                      style={{ color: awayColor }}
                    >
                      {game.awayTeam}
                    </span>
                  </div>
                )}
                <div className="min-w-0 text-center sm:text-left">
                  {away.city && (
                    <div className="text-xs sm:text-sm text-gray-500 font-medium leading-tight truncate hidden sm:block">
                      {away.city}
                    </div>
                  )}
                  <div className="text-sm sm:text-2xl font-extrabold text-[#1A1A1A] leading-tight truncate">
                    {away.name}
                  </div>
                  {game.seriesStatus && game.seriesStatus.round > 0 && (
                    <div className="text-[10px] sm:text-xs text-gray-400 font-medium mt-0.5">
                      {getSeriesRecord(game, true)}
                    </div>
                  )}
                </div>
              </div>

              {/* ─── Center: score or time ─── */}
              <div className="flex-shrink-0 mx-2 sm:mx-4 text-center">
                {hasScore ? (
                  <div className="flex items-center justify-center gap-3 sm:gap-5">
                    <span
                      className={`text-3xl sm:text-5xl font-black tabular-nums ${
                        isGameComplete && game.awayScore > game.homeScore
                          ? "text-[#1A1A1A]"
                          : isGameComplete
                            ? "text-gray-400"
                            : "text-[#1A1A1A]"
                      }`}
                    >
                      {game.awayScore}
                    </span>
                    <div className="flex flex-col items-center min-w-[60px] sm:min-w-[80px]">
                      <StatusBadge
                        gameStatus={gameStatus}
                        period={game.period}
                        isLive={isLive}
                        isGameComplete={isGameComplete}
                      />
                      <div className="text-[10px] sm:text-xs text-gray-500 font-medium mt-1">
                        {dateString}
                      </div>
                    </div>
                    <span
                      className={`text-3xl sm:text-5xl font-black tabular-nums ${
                        isGameComplete && game.homeScore > game.awayScore
                          ? "text-[#1A1A1A]"
                          : isGameComplete
                            ? "text-gray-400"
                            : "text-[#1A1A1A]"
                      }`}
                    >
                      {game.homeScore}
                    </span>
                  </div>
                ) : (
                  <div>
                    <div className="bg-white border border-gray-300 inline-block px-3 py-1.5 rounded-sm mb-1">
                      <div className="text-sm sm:text-base font-bold text-[#1A1A1A]">
                        {timeString}
                      </div>
                    </div>
                    <div className="text-[10px] sm:text-xs text-gray-400 font-medium">
                      {dateString}
                    </div>
                    {isLive && (
                      <div className="mt-1.5">
                        <StatusBadge
                          gameStatus={gameStatus}
                          period={game.period}
                          isLive={isLive}
                          isGameComplete={isGameComplete}
                        />
                      </div>
                    )}
                  </div>
                )}
              </div>

              {/* ─── Home team (right) ─── */}
              <div className="flex-1 flex flex-col-reverse sm:flex-row items-center justify-end gap-1 sm:gap-5 min-w-0 pr-0 sm:pr-[65px] md:pr-[85px]">
                <div className="min-w-0 text-center sm:text-right">
                  {home.city && (
                    <div className="text-xs sm:text-sm text-gray-500 font-medium leading-tight truncate hidden sm:block">
                      {home.city}
                    </div>
                  )}
                  <div className="text-sm sm:text-2xl font-extrabold text-[#1A1A1A] leading-tight truncate">
                    {home.name}
                  </div>
                  {game.seriesStatus && game.seriesStatus.round > 0 && (
                    <div className="text-[10px] sm:text-xs text-gray-400 font-medium mt-0.5">
                      {getSeriesRecord(game, false)}
                    </div>
                  )}
                </div>
                {game.homeTeamLogo ? (
                  <img
                    src={game.homeTeamLogo}
                    alt={`${game.homeTeam} logo`}
                    className="w-14 h-14 sm:w-20 sm:h-20 flex-shrink-0 object-contain drop-shadow-md"
                  />
                ) : (
                  <div
                    className="w-14 h-14 sm:w-20 sm:h-20 flex-shrink-0 rounded-sm flex items-center justify-center"
                    style={{ backgroundColor: `${homeColor}15` }}
                  >
                    <span
                      className="text-sm font-bold"
                      style={{ color: homeColor }}
                    >
                      {game.homeTeam}
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Venue */}
            {game.venue && (
              <div className="text-center mt-3 text-[10px] sm:text-xs text-gray-400 font-medium tracking-wide uppercase">
                {game.venue}
              </div>
            )}
          </div>
        </div>

        {/* Game links — outside the stripes container */}
        {isGameComplete && (
          <div className="px-4 sm:px-6">
            <div className="pb-3 pt-3 border-t border-gray-200 flex justify-center gap-6">
              <a
                href={`https://www.nhl.com/gamecenter/${game.id}/recap`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs font-bold uppercase tracking-wider text-gray-400 hover:text-[#2563EB] transition-colors flex items-center gap-1"
                onClick={(e) => e.stopPropagation()}
              >
                <svg
                  className="w-3.5 h-3.5"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M8 5v14l11-7z" />
                </svg>
                Highlights
              </a>
              <a
                href={`https://www.nhl.com/gamecenter/${game.id}/boxscore`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs font-bold uppercase tracking-wider text-gray-400 hover:text-[#2563EB] transition-colors flex items-center gap-1"
                onClick={(e) => e.stopPropagation()}
              >
                <svg
                  className="w-3.5 h-3.5"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14z" />
                  <path d="M7 7h10v2H7zm0 4h10v2H7zm0 4h7v2H7z" />
                </svg>
                Box Score
              </a>
            </div>
          </div>
        )}
      </div>

      {/* Expand/Collapse toggle */}
      {onToggleExpand && (
        <button
          className="w-full py-2.5 px-4 text-xs font-bold uppercase tracking-wider text-gray-500 hover:text-[#1A1A1A] hover:bg-gray-50 flex items-center justify-center border-t border-gray-200 transition-all"
          onClick={onToggleExpand}
        >
          {isExpanded ? collapseLabel : expandLabel}
          <svg
            className={`ml-1.5 h-3.5 w-3.5 transform ${isExpanded ? "rotate-180" : ""} transition-transform duration-200`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2.5}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>
      )}

      {/* Expanded children content */}
      {isExpanded && children}
    </div>
  );
};

export default NHLGameCard;
