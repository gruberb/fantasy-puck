import { getNHLTeamLogoUrl, getNHLTeamShortName } from "@/utils/nhlTeams";

import type { PlayerOdds } from "../types";

interface FantasyChampionBoardProps {
  players: PlayerOdds[];
}

/**
 * Global Fantasy Champion view: top NHL skaters by projected final playoff
 * fantasy points. Shown on the no-league Insights page. Non-interactive
 * preview — rosters still get drafted at the league level.
 */
export function FantasyChampionBoard({ players }: FantasyChampionBoardProps) {
  if (players.length === 0) {
    return (
      <p className="text-xs text-gray-400">
        Leaderboard isn't available yet — try again once the playoffs begin.
      </p>
    );
  }
  return (
    <ol className="divide-y divide-gray-100 border border-gray-200">
      {players.map((player, i) => (
        <li
          key={player.nhlId}
          className="flex items-center gap-3 px-3 py-2 text-sm"
        >
          <span className="w-6 h-6 flex items-center justify-center bg-gray-100 font-bold text-xs tabular-nums">
            {i + 1}
          </span>
          {player.imageUrl && (
            <img
              src={player.imageUrl}
              alt={player.name}
              className="w-8 h-8 bg-gray-100"
              onError={(e) => {
                (e.currentTarget as HTMLImageElement).style.display = "none";
              }}
            />
          )}
          <div className="flex-1 min-w-0">
            <p className="font-bold truncate">{player.name}</p>
            <p className="text-[10px] text-gray-500 flex items-center gap-1">
              <img
                src={getNHLTeamLogoUrl(player.nhlTeam)}
                alt={player.nhlTeam}
                className="w-3 h-3"
              />
              <span>{getNHLTeamShortName(player.nhlTeam)}</span>
              <span>&middot;</span>
              <span>{player.position}</span>
            </p>
          </div>
          <div className="text-right">
            <p className="font-bold text-base tabular-nums">
              ~{Math.round(player.projectedFinalMean)}
              <span className="text-[10px] text-gray-400 ml-1">pts</span>
            </p>
            <p className="text-[10px] text-gray-400 tabular-nums">
              {Math.round(player.p10)}–{Math.round(player.p90)}
            </p>
          </div>
        </li>
      ))}
    </ol>
  );
}
