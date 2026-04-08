import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";

import { Game } from "@/types/games";

interface GameDetailViewProps {
  game: Game;
  expandedGames: Set<number>;
  toggleGameExpansion: (gameId: number) => void;
  getTeamPrimaryColor: (teamName: string) => string;
}

export default function GameDetailView({
  game,
  expandedGames,
  toggleGameExpansion,
  getTeamPrimaryColor,
}: GameDetailViewProps) {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const isExpanded = expandedGames.has(game.id);

  // Format time string
  let timeString;
  try {
    const gameDate = new Date(game.startTime);
    timeString = gameDate.toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
  } catch {
    timeString = "Time TBD";
  }

  // Game status
  const gameStatus = game.gameState || "SCHEDULED";

  // Check if game is complete
  const isGameComplete = gameStatus === "FINAL" || gameStatus === "OFF";

  // Get team colors
  const awayTeamColor = getTeamPrimaryColor(game.awayTeam);
  const homeTeamColor = getTeamPrimaryColor(game.homeTeam);

  const getStatusClass = (status: string): string => {
    switch (status.toUpperCase()) {
      case "LIVE":
        return "bg-red-100 text-red-800 border border-red-200";
      case "FINAL":
      case "OFF":
        return "bg-gray-100 text-gray-800 border border-gray-200";
      case "SCHEDULED":
      case "PRE":
        return "bg-green-100 text-green-800 border border-green-200";
      case "POSTPONED":
        return "bg-yellow-100 text-yellow-800 border border-yellow-200";
      default:
        return "bg-blue-100 text-blue-800 border border-blue-200";
    }
  };

  return (
    <div className="bg-white rounded-none overflow-hidden">
      {/* NHL-style game card with team colors on sides */}
      <div className="flex">
        {/* Left team color bar */}
        <div
          className="w-2 flex-shrink-0"
          style={{ backgroundColor: awayTeamColor }}
        ></div>

        {/* Main game content */}
        <div className="flex-grow">
          {/* Game header */}
          <div className="bg-gray-50 p-3 flex items-center justify-between border-b border-gray-100">
            <div className="text-sm font-bold">{timeString}</div>
            <div>
              <span
                className={`px-2 py-1 rounded-none text-xs font-bold ${getStatusClass(gameStatus)}`}
              >
                {gameStatus === "PRE" ? "SCHEDULED" : gameStatus}
                {game.period && ` • ${game.period}`}
              </span>
            </div>
          </div>

          {/* Team matchup - NHL style */}
          <div
            className="p-4 cursor-pointer hover:bg-gray-50"
            onClick={() =>
              window.open(`https://www.nhl.com/gamecenter/${game.id}`, "_blank")
            }
          >
            <div className="flex items-center">
              {/* Away team */}
              <div className="flex-1">
                <div className="flex items-center">
                  {game.awayTeamLogo ? (
                    <img
                      src={game.awayTeamLogo}
                      alt={`${game.awayTeam} logo`}
                      className="w-12 h-12 mr-3"
                    />
                  ) : (
                    <div
                      className="w-12 h-12 rounded-none flex items-center justify-center mr-3"
                      style={{
                        backgroundColor: `${awayTeamColor}20`,
                      }}
                    >
                      <span
                        className="text-sm font-bold"
                        style={{ color: awayTeamColor }}
                      >
                        {game.awayTeam.substring(0, 3)}
                      </span>
                    </div>
                  )}
                  <div>
                    <div className="text-lg font-bold">{game.awayTeam}</div>
                    {game.seriesStatus &&
                      game.seriesStatus.topSeedTeamAbbrev && (
                        <div className="text-xs text-gray-500">
                          {game.seriesStatus.topSeedTeamAbbrev ===
                          game.awayTeam.substring(0, 3)
                            ? `${game.seriesStatus.topSeedWins}-${game.seriesStatus.bottomSeedWins}`
                            : `${game.seriesStatus.bottomSeedWins}-${game.seriesStatus.topSeedWins}`}
                        </div>
                      )}
                  </div>
                </div>
              </div>

              {/* Score */}
              <div className="px-4 text-center flex flex-col">
                {game.awayScore !== undefined &&
                game.awayScore !== null &&
                game.homeScore !== undefined &&
                game.homeScore !== null ? (
                  <>
                    <div className="flex items-center justify-center">
                      <div className="text-3xl font-bold">{game.awayScore}</div>
                      <div className="mx-2 text-gray-300">-</div>
                      <div className="text-3xl font-bold">{game.homeScore}</div>
                    </div>

                    {/* Subtle text links for completed games - properly positioned underneath */}
                    {isGameComplete && (
                      <div className="mt-1 text-xs flex justify-center space-x-3">
                        <a
                          href={`https://www.nhl.com/gamecenter/${game.id}/recap`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-gray-500 hover:text-[#2563EB] hover:underline flex items-center"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <svg
                            className="w-3 h-3 mr-1"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                          >
                            <path d="M8 5v14l11-7z" />
                          </svg>
                          Highlights
                        </a>
                        <span className="text-gray-300">|</span>
                        <a
                          href={`https://www.nhl.com/gamecenter/${game.id}/boxscore`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-gray-500 hover:text-[#2563EB] hover:underline flex items-center"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <svg
                            className="w-3 h-3 mr-1"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                          >
                            <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14z" />
                            <path d="M7 7h10v2H7zm0 4h10v2H7zm0 4h7v2H7z" />
                          </svg>
                          Box Score
                        </a>
                      </div>
                    )}
                  </>
                ) : (
                  <div className="text-lg font-bold text-gray-400">VS</div>
                )}
                {gameStatus === "LIVE" && game.period && (
                  <div className="text-xs text-red-600 font-bold mt-1 animate-pulse">
                    {game.period}
                  </div>
                )}
              </div>

              {/* Home team */}
              <div className="flex-1 text-right">
                <div className="flex items-center justify-end">
                  <div className="text-right">
                    <div className="text-lg font-bold">{game.homeTeam}</div>
                    {game.seriesStatus &&
                      game.seriesStatus.topSeedTeamAbbrev && (
                        <div className="text-xs text-gray-500">
                          {game.seriesStatus.topSeedTeamAbbrev ===
                          game.homeTeam.substring(0, 3)
                            ? `${game.seriesStatus.topSeedWins}-${game.seriesStatus.bottomSeedWins}`
                            : `${game.seriesStatus.bottomSeedWins}-${game.seriesStatus.topSeedWins}`}
                        </div>
                      )}
                  </div>
                  {game.homeTeamLogo ? (
                    <img
                      src={game.homeTeamLogo}
                      alt={`${game.homeTeam} logo`}
                      className="w-12 h-12 ml-3"
                    />
                  ) : (
                    <div
                      className="w-12 h-12 rounded-none flex items-center justify-center ml-3"
                      style={{
                        backgroundColor: `${homeTeamColor}20`,
                      }}
                    >
                      <span
                        className="text-sm font-bold"
                        style={{ color: homeTeamColor }}
                      >
                        {game.homeTeam.substring(0, 3)}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Right team color bar */}
        <div
          className="w-2 flex-shrink-0"
          style={{ backgroundColor: homeTeamColor }}
        ></div>
      </div>

      {/* Player information - Collapsible or in an accordion */}
      <div className="border-t">
        <button
          className="w-full py-2 px-4 text-sm font-medium text-gray-700 hover:bg-gray-50 flex items-center justify-center"
          onClick={() => toggleGameExpansion(game.id)}
        >
          <span>{isExpanded ? "Hide" : "Show"} Skater Details</span>
          <svg
            className={`ml-2 h-5 w-5 transform ${isExpanded ? "rotate-180" : ""} transition-transform duration-200`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>

        {/* Collapsible content */}
        {isExpanded && (
          <div className="p-4 grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Away team players */}
            <div>
              <h3 className="font-bold text-md mb-2 flex items-center">
                <div
                  className="w-3 h-3 mr-2"
                  style={{ backgroundColor: awayTeamColor }}
                ></div>
                {game.awayTeam} Skaters
              </h3>
              <div className="overflow-x-auto">
                <table className="min-w-full">
                  <thead className="bg-gray-50">
                    <tr>
                      <th className="py-1 px-2 text-left text-xs">Skater</th>
                      <th className="py-1 px-2 text-left text-xs">
                        Fantasy Team
                      </th>
                      <th className="py-1 px-2 text-left text-xs">Points</th>
                    </tr>
                  </thead>
                  <tbody>
                    {game.awayTeamPlayers.map((player, idx) => (
                      <tr key={idx} className="border-t hover:bg-gray-50">
                        <td className="py-1 px-2">
                          <div className="flex items-center">
                            {player.imageUrl ? (
                              <img
                                src={player.imageUrl}
                                alt={player.playerName || ""}
                                className="w-6 h-6 rounded-none mr-2"
                              />
                            ) : (
                              <div className="w-6 h-6 bg-gray-200 rounded-none flex items-center justify-center mr-2">
                                <span className="text-xs font-medium">
                                  {(player.playerName || "")
                                    .substring(0, 2)
                                    .toUpperCase()}
                                </span>
                              </div>
                            )}
                            {player.nhlId ? (
                              <a
                                href={`https://www.nhl.com/player/${player.nhlId}`}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="text-xs hover:text-[#2563EB] hover:underline"
                              >
                                {player.playerName || ""}
                              </a>
                            ) : (
                              <span className="text-xs">
                                {player.playerName || ""}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="py-1 px-2 text-xs">
                          {player.fantasyTeam ? (
                            <Link
                              to={`${lp}/teams/${player.fantasyTeamId || ""}`}
                              className="hover:text-[#2563EB] hover:underline"
                            >
                              {player.fantasyTeam}
                            </Link>
                          ) : (
                            "-"
                          )}
                        </td>
                        <td className="py-1 px-2 text-xs font-medium">
                          {player.points || 0}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>

            {/* Home team players */}
            <div>
              <h3 className="font-bold text-md mb-2 flex items-center">
                <div
                  className="w-3 h-3 mr-2"
                  style={{ backgroundColor: homeTeamColor }}
                ></div>
                {game.homeTeam} Skaters
              </h3>
              <table className="min-w-full">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="py-1 px-2 text-left text-xs">Skater</th>
                    <th className="py-1 px-2 text-left text-xs">
                      Fantasy Team
                    </th>
                    <th className="py-1 px-2 text-left text-xs">Points</th>
                  </tr>
                </thead>
                <tbody>
                  {game.homeTeamPlayers.map((player, idx) => (
                    <tr key={idx} className="border-t hover:bg-gray-50">
                      <td className="py-1 px-2">
                        <div className="flex items-center">
                          {player.imageUrl ? (
                            <img
                              src={player.imageUrl}
                              alt={player.playerName || ""}
                              className="w-6 h-6 rounded-none mr-2"
                            />
                          ) : (
                            <div className="w-6 h-6 bg-gray-200 rounded-none flex items-center justify-center mr-2">
                              <span className="text-xs font-medium">
                                {(player.playerName || "")
                                  .substring(0, 2)
                                  .toUpperCase()}
                              </span>
                            </div>
                          )}
                          {player.nhlId ? (
                            <a
                              href={`https://www.nhl.com/player/${player.nhlId}`}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-xs hover:text-[#2563EB] hover:underline"
                            >
                              {player.playerName || ""}
                            </a>
                          ) : (
                            <span className="text-xs">
                              {player.playerName || ""}
                            </span>
                          )}
                        </div>
                      </td>
                      <td className="py-1 px-2 text-xs">
                        {player.fantasyTeam ? (
                          <Link
                            to={`${lp}/teams/${player.fantasyTeamId || ""}`}
                            className="hover:text-[#2563EB] hover:underline"
                          >
                            {player.fantasyTeam}
                          </Link>
                        ) : (
                          "-"
                        )}
                      </td>
                      <td className="py-1 px-2 text-xs font-medium">
                        {player.points || 0}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
