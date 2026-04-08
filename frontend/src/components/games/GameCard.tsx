import { Game } from "@/types/games";
import PlayerCard from "@/components/common/PlayerCard";
import { useLeague } from "@/contexts/LeagueContext";
import NHLGameCard from "./NHLGameCard";

interface StandardGameCardProps {
  game: Game;
  isExpanded: boolean;
  onToggleExpand: () => void;
  getTeamPrimaryColor: (teamName: string) => string;
}

const GameCard = ({
  game,
  isExpanded,
  onToggleExpand,
  getTeamPrimaryColor,
}: StandardGameCardProps) => {
  const { activeLeagueId } = useLeague();
  const hasLeague = !!activeLeagueId;

  const awayTeamColor = getTeamPrimaryColor(game.awayTeam);
  const homeTeamColor = getTeamPrimaryColor(game.homeTeam);

  return (
    <NHLGameCard
      game={game}
      getTeamPrimaryColor={getTeamPrimaryColor}
      expandLabel="Show Game Details"
      collapseLabel="Hide Game Details"
      isExpanded={hasLeague ? isExpanded : false}
      onToggleExpand={hasLeague ? onToggleExpand : undefined}
    >
      <PlayerDetails
        game={game}
        awayTeamColor={awayTeamColor}
        homeTeamColor={homeTeamColor}
      />
    </NHLGameCard>
  );
};

// Separate component for player details using PlayerCard
const PlayerDetails = ({
  game,
  awayTeamColor,
  homeTeamColor,
}: {
  game: Game;
  awayTeamColor: string;
  homeTeamColor: string;
}) => {
  return (
    <div className="border-t border-gray-200">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 p-5 bg-gray-50">
        {/* Away team players */}
        <div className="bg-white rounded-none p-3 border border-gray-100">
          <div className="flex items-center mb-3">
            <div
              className="w-1 h-5 rounded-full mr-2.5"
              style={{ backgroundColor: awayTeamColor }}
            />
            <h3 className="font-bold text-sm uppercase tracking-wide text-[#1A1A1A]">
              {game.awayTeam} Skaters
            </h3>
          </div>

          <div className="grid grid-cols-1 gap-2">
            {game.awayTeamPlayers.length === 0 ? (
              <p className="text-xs text-gray-400 uppercase tracking-wider py-2">
                No fantasy team has players for this team
              </p>
            ) : (
              game.awayTeamPlayers.map((player, idx) => {
                player.nhlTeam = game.awayTeam;
                return (
                  <PlayerCard
                    key={idx}
                    player={player}
                    compact={true}
                    showFantasyTeam={true}
                    showPoints={true}
                    valueLabel="pts"
                  />
                );
              })
            )}
          </div>
        </div>

        {/* Home team players */}
        <div className="bg-white rounded-none p-3 border border-gray-100">
          <div className="flex items-center mb-3">
            <div
              className="w-1 h-5 rounded-full mr-2.5"
              style={{ backgroundColor: homeTeamColor }}
            />
            <h3 className="font-bold text-sm uppercase tracking-wide text-[#1A1A1A]">
              {game.homeTeam} Skaters
            </h3>
          </div>

          <div className="grid grid-cols-1 gap-2">
            {game.homeTeamPlayers.length === 0 ? (
              <p className="text-xs text-gray-400 uppercase tracking-wider py-2">
                No fantasy team has players for this team
              </p>
            ) : (
              game.homeTeamPlayers.map((player, idx) => {
                player.nhlTeam = game.homeTeam;
                return (
                  <PlayerCard
                    key={idx}
                    player={player}
                    compact={true}
                    showFantasyTeam={true}
                    showPoints={true}
                    valueLabel="pts"
                  />
                );
              })
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default GameCard;
