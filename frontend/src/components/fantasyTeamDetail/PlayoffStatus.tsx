import { NHLTeamBet } from "@/types/fantasyTeams";
import { SkaterStats } from "@/types/skaters";
import { getNHLTeamUrlSlug } from "@/utils/nhlTeams";
import { usePlayoffsData } from "@/features/rankings";

interface PlayoffStatusProps {
  players: SkaterStats[];
  teamsInPlayoffs: NHLTeamBet[];
  playersInPlayoffs: SkaterStats[];
  totalTeams: number;
  totalPlayers: number;
}

export default function PlayoffStatus({
  players,
  teamsInPlayoffs,
  playersInPlayoffs,
  totalTeams,
  totalPlayers,
}: PlayoffStatusProps) {
  const { isTeamInPlayoffs } = usePlayoffsData();

  return (
    <section className="card">
      <h2 className="text-2xl font-bold mb-4">Playoff Status</h2>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div>
          <h3 className="text-lg font-medium mb-3">NHL Teams in Playoffs</h3>
          <div className="bg-gray-50 p-4 rounded-none">
            <div className="flex justify-between mb-2">
              <span>Teams in Playoffs</span>
              <span className="font-bold">
                {teamsInPlayoffs.length} / {totalTeams}
              </span>
            </div>

            {teamsInPlayoffs.length > 0 ? (
              <div className="mt-3">
                <h4 className="text-sm font-medium mb-2">Teams Still Active</h4>
                <div className="flex flex-wrap gap-2">
                  {teamsInPlayoffs.map((team) => (
                    <a
                      href={`https://www.nhl.com/${getNHLTeamUrlSlug(team.nhlTeam)}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-gray-900 hover:text-[#2563EB] hover:underline flex items-center font-medium"
                    >
                      <div
                        key={team.nhlTeam}
                        className="flex items-center bg-gray-50 text-gray-800 px-3 py-1 rounded border border-gray-200"
                      >
                        {team.teamLogo && (
                          <img
                            src={team.teamLogo}
                            alt={team.nhlTeam}
                            className="w-5 h-5 mr-2"
                          />
                        )}
                        <span className="text-sm font-medium">
                          {team.nhlTeamName}
                        </span>
                      </div>
                    </a>
                  ))}
                </div>
              </div>
            ) : (
              <p className="text-gray-500 text-sm mt-2">No teams in playoffs</p>
            )}
          </div>
        </div>

        <div>
          <h3 className="text-lg font-medium mb-3">Playoffs Stats</h3>
          <div className="bg-gray-50 p-4 rounded-none">
            <div className="flex justify-between mb-2">
              <span>Players in Playoffs</span>
              <span className="font-bold">
                {playersInPlayoffs.length} / {totalPlayers}
              </span>
            </div>

            {players.length > 0 ? (
              <div className="mt-3">
                <h4 className="text-sm font-medium mb-2">Top 5 Skater</h4>
                <div className="flex flex-col gap-2">
                  {players.slice(0, 5).map((player) => {
                    const isInPlayoffs = isTeamInPlayoffs(player.nhlTeam);

                    return (
                      <div
                        key={player.name}
                        className={`flex items-center" ${!isInPlayoffs ? "opacity-25" : ""}`}
                      >
                        <a
                          href={`https://www.nhl.com/player/${player.nhlId}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-gray-900 hover:text-[#2563EB] hover:underline flex items-center font-medium"
                        >
                          {player.imageUrl ? (
                            <img
                              src={player.imageUrl}
                              alt={player.name}
                              className="w-6 h-6 rounded-none mr-2"
                            />
                          ) : (
                            <div className="w-6 h-6 bg-gray-200 rounded-none flex items-center justify-center mr-2">
                              <span className="text-xs font-medium">
                                {player.name.substring(0, 2).toUpperCase()}
                              </span>
                            </div>
                          )}
                          <span className="text-sm pr-1">{player.name} </span>
                        </a>
                        <span className="ml-auto text-sm font-bold">
                          {player.totalPoints} pts
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : (
              <p className="text-gray-500 text-sm mt-2">
                No skaters in playoffs
              </p>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
