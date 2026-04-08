import { usePlayoffsData } from "@/features/rankings";
import { PlayerInAction } from "@/types/matchDay";

interface PlayerComparisonProps {
  game: any;
  fantasyTeams: any[];
}

const PlayerComparison = ({ game, fantasyTeams }: PlayerComparisonProps) => {
  const { isTeamInPlayoffs } = usePlayoffsData();

  // Get all players from this game
  const getAllGamePlayers = () => {
    const players: PlayerInAction[] = [];

    // Extract players from all fantasy teams that are playing in this game
    fantasyTeams.forEach((team) => {
      team.playersInAction.forEach((player) => {
        if (
          player.nhlTeam === game.homeTeam ||
          player.nhlTeam === game.awayTeam
        ) {
          players.push(player);
        }
      });
    });

    return players;
  };

  // Get players grouped by their NHL team
  const getPlayersByNHLTeam = () => {
    const allPlayers = getAllGamePlayers();
    const homeTeamPlayers = allPlayers
      .filter((p) => p.nhlTeam === game.homeTeam)
      .sort((a, b) => (b.playoffPoints || 0) - (a.playoffPoints || 0));
    const awayTeamPlayers = allPlayers
      .filter((p) => p.nhlTeam === game.awayTeam)
      .sort((a, b) => (b.playoffPoints || 0) - (a.playoffPoints || 0));

    return {
      homeTeamPlayers,
      awayTeamPlayers,
    };
  };

  // Create player comparison pairs
  const createComparisonPairs = () => {
    const { homeTeamPlayers, awayTeamPlayers } = getPlayersByNHLTeam();
    const pairs = [];

    // Create pairs based on the smaller team's size
    const pairCount = Math.min(homeTeamPlayers.length, awayTeamPlayers.length);

    for (let i = 0; i < pairCount; i++) {
      pairs.push({
        homePlayer: homeTeamPlayers[i],
        awayPlayer: awayTeamPlayers[i],
      });
    }

    return pairs;
  };

  const playerPairs = createComparisonPairs();

  if (playerPairs.length === 0) {
    return null;
  }

  return (
    <div className="mb-6">
      <h3 className="text-lg font-bold mb-3">Player Matchups</h3>
      <div className="space-y-4">
        {playerPairs.slice(0, 3).map((pair, index) => (
          <div
            key={index}
            className="bg-white rounded-none overflow-hidden border border-gray-200"
          >
            <div className="grid grid-cols-2 divide-x divide-gray-200">
              {/* Away Player */}
              <div className="p-4">
                <div className="flex items-center mb-2">
                  {pair.awayPlayer.imageUrl ? (
                    <img
                      src={pair.awayPlayer.imageUrl}
                      alt={pair.awayPlayer.playerName}
                      className="w-12 h-12 rounded-none mr-3 border border-gray-200"
                    />
                  ) : (
                    <div className="w-12 h-12 bg-gray-200 rounded-none flex items-center justify-center mr-3">
                      <span className="text-sm font-medium">
                        {pair.awayPlayer.playerName
                          .substring(0, 2)
                          .toUpperCase()}
                      </span>
                    </div>
                  )}
                  <div>
                    <a
                      href={`https://www.nhl.com/player/${pair.awayPlayer.nhlId}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="font-medium text-gray-900 hover:text-[#2563EB] block"
                    >
                      {pair.awayPlayer.playerName}
                    </a>
                    <div className="text-xs text-gray-500 flex items-center">
                      {pair.awayPlayer.teamLogo && (
                        <img
                          src={pair.awayPlayer.teamLogo}
                          alt={pair.awayPlayer.nhlTeam}
                          className="w-4 h-4 mr-1"
                        />
                      )}
                      <span>
                        {pair.awayPlayer.position} • {pair.awayPlayer.nhlTeam}
                      </span>
                    </div>
                    <div className="text-xs text-[#2563EB]">
                      {pair.awayPlayer.fantasyTeam}
                    </div>
                  </div>
                </div>

                {/* Stats */}
                <div className="grid grid-cols-3 gap-2 mt-3">
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Playoff Pts</div>
                    <div className="font-bold text-lg">
                      {pair.awayPlayer.playoffPoints}
                    </div>
                  </div>
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Goals</div>
                    <div className="font-bold text-lg">
                      {pair.awayPlayer.playoffGoals}
                    </div>
                  </div>
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Assists</div>
                    <div className="font-bold text-lg">
                      {pair.awayPlayer.playoffAssists}
                    </div>
                  </div>
                </div>

                {/* Form */}
                {pair.awayPlayer.form && (
                  <div className="mt-3 text-sm">
                    <div className="text-xs text-gray-500 mb-1">
                      Last {pair.awayPlayer.form.games} games
                    </div>
                    <div className="flex space-x-2">
                      <span className="px-2 py-1 bg-green-100 text-green-800 rounded text-xs font-medium">
                        {pair.awayPlayer.form.goals} G
                      </span>
                      <span className="px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs font-medium">
                        {pair.awayPlayer.form.assists} A
                      </span>
                      <span className="px-2 py-1 bg-purple-100 text-purple-800 rounded text-xs font-medium">
                        {pair.awayPlayer.form.points} PTS
                      </span>
                    </div>
                  </div>
                )}
              </div>

              {/* Home Player */}
              <div className="p-4">
                <div className="flex items-center mb-2">
                  {pair.homePlayer.imageUrl ? (
                    <img
                      src={pair.homePlayer.imageUrl}
                      alt={pair.homePlayer.playerName}
                      className="w-12 h-12 rounded-none mr-3 border border-gray-200"
                    />
                  ) : (
                    <div className="w-12 h-12 bg-gray-200 rounded-none flex items-center justify-center mr-3">
                      <span className="text-sm font-medium">
                        {pair.homePlayer.playerName
                          .substring(0, 2)
                          .toUpperCase()}
                      </span>
                    </div>
                  )}
                  <div>
                    <a
                      href={`https://www.nhl.com/player/${pair.homePlayer.nhlId}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="font-medium text-gray-900 hover:text-[#2563EB] block"
                    >
                      {pair.homePlayer.playerName}
                    </a>
                    <div className="text-xs text-gray-500 flex items-center">
                      {pair.homePlayer.teamLogo && (
                        <img
                          src={pair.homePlayer.teamLogo}
                          alt={pair.homePlayer.nhlTeam}
                          className="w-4 h-4 mr-1"
                        />
                      )}
                      <span>
                        {pair.homePlayer.position} • {pair.homePlayer.nhlTeam}
                      </span>
                    </div>
                    <div className="text-xs text-[#2563EB]">
                      {pair.homePlayer.fantasyTeam}
                    </div>
                  </div>
                </div>

                {/* Stats */}
                <div className="grid grid-cols-3 gap-2 mt-3">
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Playoff Pts</div>
                    <div className="font-bold text-lg">
                      {pair.homePlayer.playoffPoints}
                    </div>
                  </div>
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Goals</div>
                    <div className="font-bold text-lg">
                      {pair.homePlayer.playoffGoals}
                    </div>
                  </div>
                  <div className="text-center bg-gray-50 p-2 rounded">
                    <div className="text-xs text-gray-500">Assists</div>
                    <div className="font-bold text-lg">
                      {pair.homePlayer.playoffAssists}
                    </div>
                  </div>
                </div>

                {/* Form */}
                {pair.homePlayer.form && (
                  <div className="mt-3 text-sm">
                    <div className="text-xs text-gray-500 mb-1">
                      Last {pair.homePlayer.form.games} games
                    </div>
                    <div className="flex space-x-2">
                      <span className="px-2 py-1 bg-green-100 text-green-800 rounded text-xs font-medium">
                        {pair.homePlayer.form.goals} G
                      </span>
                      <span className="px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs font-medium">
                        {pair.homePlayer.form.assists} A
                      </span>
                      <span className="px-2 py-1 bg-purple-100 text-purple-800 rounded text-xs font-medium">
                        {pair.homePlayer.form.points} PTS
                      </span>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default PlayerComparison;
