import { PlayerInAction } from "@/types/matchDay";

interface PlayerWithStatsProps {
  player: PlayerInAction;
}

const PlayerWithStats = ({ player }: PlayerWithStatsProps) => {
  return (
    <div className="bg-white rounded-none border border-gray-200 p-3">
      <div className="flex items-center mb-2">
        {player.imageUrl ? (
          <img
            src={player.imageUrl}
            alt={player.playerName}
            className="w-10 h-10 rounded-none mr-2 border border-gray-200"
          />
        ) : (
          <div className="w-10 h-10 bg-gray-200 rounded-none flex items-center justify-center mr-2">
            <span className="text-xs font-medium">
              {player.playerName.substring(0, 2).toUpperCase()}
            </span>
          </div>
        )}
        <div>
          <div className="font-medium text-gray-900">
            <a
              href={`https://www.nhl.com/player/${player.nhlId}`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-[#2563EB]"
            >
              {player.playerName}
            </a>
          </div>
          <div className="text-xs text-gray-500">
            {player.position} • {player.nhlTeam}
          </div>
        </div>
      </div>

      {/* Stats section */}
      <div className="border-t border-gray-100 pt-2 mt-2">
        <div className="grid grid-cols-3 gap-1 text-center">
          <div className="bg-gray-50 rounded p-1">
            <div className="text-xs text-gray-500">Today</div>
            <div className="font-medium text-gray-900">
              {player.points || 0}
            </div>
          </div>
          <div className="bg-gray-50 rounded p-1">
            <div className="text-xs text-gray-500">Playoffs</div>
            <div className="font-medium text-gray-900">
              {player.playoffPoints || 0}
            </div>
          </div>
          <div className="bg-gray-50 rounded p-1 group relative">
            <div className="text-xs text-gray-500 flex items-center justify-center">
              Last 5
              <span className="text-gray-400 hover:text-gray-600 ml-1 cursor-help">
                ⓘ
                <span className="absolute invisible group-hover:visible opacity-0 group-hover:opacity-100 transition-opacity duration-300 bottom-full left-1/2 transform -translate-x-1/2 w-48 bg-gray-900 text-white text-xs rounded py-1 px-2 z-50 mb-2">
                  <span className="block font-medium mb-1">
                    Form Calculation:
                  </span>
                  <span className="block">
                    Combined stats from player's 5 most recent games showing
                    current performance trend.
                  </span>
                  <span className="absolute bottom-[-5px] left-1/2 transform -translate-x-1/2 w-0 h-0 border-l-4 border-l-transparent border-r-4 border-r-transparent border-t-4 border-t-gray-900"></span>
                </span>
              </span>
            </div>
            <div className="font-medium text-gray-900">
              {player.form?.points || 0}
            </div>
          </div>
        </div>
      </div>

      {/* Recent form if available */}
      {player.form && (
        <div className="text-xs text-gray-500 mt-2 flex justify-between">
          <span>
            Form: {player.form.goals}G {player.form.assists}A
          </span>
          <span>TOI: {player.timeOnIce || "-"}</span>
        </div>
      )}
    </div>
  );
};

export default PlayerWithStats;
