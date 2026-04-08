import { Link } from "react-router-dom";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { useLeague } from "@/contexts/LeagueContext";

interface PlayerCardProps {
  player: {
    playerName?: string;
    nhlId?: number;
    imageUrl?: string;
    nhlTeam?: string;
    teamLogo?: string;
    position?: string;
    fantasyTeam?: string;
    fantasyTeamId?: number;
    points?: number;
  };
  showFantasyTeam?: boolean;
  showPoints?: boolean;
  valueLabel?: string;
  compact?: boolean;
  onClick?: () => void;
}

const PlayerCard = ({
  player,
  showFantasyTeam = true,
  showPoints = true,
  valueLabel = "pts",
  compact = false,
  onClick,
}: PlayerCardProps) => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  // Class names based on card size
  const imageSize = compact ? "w-6 h-6" : "w-8 h-8";
  const nameTextSize = compact ? "text-xs" : "text-sm";
  const infoTextSize = compact ? "text-xs" : "text-xs";
  const valuePadding = compact ? "px-1.5 py-0.5" : "px-2 py-1";
  const valueTextSize = compact ? "text-xs" : "text-xs";

  // Base styles with increased padding
  const baseStyle =
    "bg-white rounded-none border border-gray-200 flex items-center justify-between";
  const containerPadding = compact ? "p-3" : "p-3";

  return (
    <div
      className={`${baseStyle} ${containerPadding} ${onClick ? "cursor-pointer hover:bg-gray-50" : ""}`}
      onClick={onClick}
    >
      <div className="flex items-center w-full min-w-0 overflow-hidden">
        {/* Player image */}
        {player.imageUrl ? (
          <img
            src={player.imageUrl}
            alt={player.playerName || ""}
            className={`${imageSize} rounded-none mr-3 border border-gray-100 flex-shrink-0`}
          />
        ) : (
          <div
            className={`${imageSize} bg-gray-200 rounded-none flex items-center justify-center mr-3 flex-shrink-0`}
          >
            <span className="text-xs font-medium">
              {(player.playerName || "").substring(0, 2).toUpperCase()}
            </span>
          </div>
        )}

        {/* Player details section */}
        <div className="flex-none min-w-0 overflow-hidden">
          {/* Player name with link to NHL.com if nhlId is available */}
          <div className={`${nameTextSize} font-medium text-gray-800 truncate`}>
            {player.nhlId ? (
              <a
                href={`https://www.nhl.com/player/${player.nhlId}`}
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-[#2563EB] hover:underline"
                onClick={(e) => e.stopPropagation()}
              >
                {player.playerName}
              </a>
            ) : (
              player.playerName
            )}
          </div>

          {/* Team and position info */}
          <div
            className={`flex items-center ${infoTextSize} text-gray-500 truncate`}
          >
            {player.teamLogo && (
              <img
                src={player.teamLogo}
                alt={player.nhlTeam || ""}
                className="w-3 h-3 mr-1 flex-shrink-0"
              />
            )}
            <span className="truncate">
              {player.nhlTeam ? (
                <a
                  href={`https://www.nhl.com/${getNHLTeamUrlSlug(player.nhlTeam)}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-[#2563EB] hover:underline"
                  onClick={(e) => e.stopPropagation()}
                >
                  {player.nhlTeam}
                </a>
              ) : (
                player.nhlTeam
              )}{" "}
              {player.position ? `• ${player.position}` : ""}{" "}
              {showFantasyTeam && player.fantasyTeam && (
                <Link
                  to={`${lp}/teams/${player.fantasyTeamId || ""}`}
                  className="hover:underline"
                  onClick={(e) => e.stopPropagation()}
                >
                  • {player.fantasyTeam}
                </Link>
              )}
            </span>
          </div>
        </div>

        {/* Points indicator (optional) */}
        {showPoints && (
          <div
            className={`${valueTextSize} font-bold ${valuePadding} rounded-none flex-shrink-0 ml-auto ${
              (player.points || 0) > 0
                ? "bg-green-100 text-green-800"
                : "bg-gray-100 text-gray-700"
            }`}
          >
            {player.points || 0} {valueLabel}
          </div>
        )}
      </div>
    </div>
  );
};

export default PlayerCard;
