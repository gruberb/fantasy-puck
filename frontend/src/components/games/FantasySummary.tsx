import { Link } from "react-router-dom";
import { FantasyTeamInAction } from "@/types/matchDay";
import { useLeague } from "@/contexts/LeagueContext";

interface FantasySummaryProps {
  fantasyTeams: FantasyTeamInAction[];
}

const FantasySummary = ({ fantasyTeams }: FantasySummaryProps) => {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  if (!fantasyTeams || fantasyTeams.length === 0) {
    return null;
  }

  // Sort teams by number of players in action
  const sortedTeams = [...fantasyTeams].sort(
    (a, b) => b.totalPlayersToday - a.totalPlayersToday,
  );

  // Calculate some summary stats
  const totalPlayersInAction = fantasyTeams.reduce(
    (sum, team) => sum + team.totalPlayersToday,
    0,
  );
  const maxPlayers = Math.max(
    ...fantasyTeams.map((team) => team.totalPlayersToday),
  );
  const teamsWithMaxPlayers = fantasyTeams.filter(
    (team) => team.totalPlayersToday === maxPlayers,
  ).length;

  return (
    <div className="bg-white rounded-none border border-gray-200 p-6 mb-6">
      <h2 className="text-xl font-bold mb-4">Fantasy Summary</h2>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <div className="bg-blue-50 rounded-none p-4 text-center">
          <div className="text-sm text-blue-600 mb-1">Fantasy Teams Active</div>
          <div className="text-3xl font-bold text-blue-800">
            {fantasyTeams.length}
          </div>
        </div>

        <div className="bg-green-50 rounded-none p-4 text-center">
          <div className="text-sm text-green-600 mb-1">
            Total Players Active
          </div>
          <div className="text-3xl font-bold text-green-800">
            {totalPlayersInAction}
          </div>
        </div>

        <div className="bg-purple-50 rounded-none p-4 text-center">
          <div className="text-sm text-purple-600 mb-1">
            Max Players per Team
          </div>
          <div className="text-3xl font-bold text-purple-800">{maxPlayers}</div>
        </div>

        <div className="bg-yellow-50 rounded-none p-4 text-center">
          <div className="text-sm text-yellow-600 mb-1">
            Teams with Max Players
          </div>
          <div className="text-3xl font-bold text-yellow-800">
            {teamsWithMaxPlayers}
          </div>
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="min-w-full bg-white">
          <thead className="bg-gray-50">
            <tr>
              <th className="py-3 px-4 text-left text-xs font-medium text-gray-500 uppercase w-0.5">
                Rank
              </th>
              <th className="py-3 px-4 text-left text-xs font-medium text-gray-500 uppercase w-1">
                Team
              </th>
              <th className="py-3 px-4 text-left text-xs font-medium text-gray-500 uppercase w-2">
                Players
              </th>
              <th className="py-3 px-4 text-left text-xs font-medium text-gray-500 uppercase tracking-wider"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {sortedTeams.map((team, index) => {
              // Get counts of players per NHL team
              const nhlTeamCounts = team.playersInAction.reduce(
                (acc: Record<string, number>, player) => {
                  acc[player.nhlTeam] = (acc[player.nhlTeam] || 0) + 1;
                  return acc;
                },
                {},
              );

              return (
                <tr key={team.teamId} className="hover:bg-gray-50">
                  <td className="py-3 px-4 text-sm whitespace-nowrap">
                    <div className="text-center w-8 h-8 bg-gray-100 rounded-none flex items-center justify-center font-bold">
                      {index + 1}
                    </div>
                  </td>
                  <td className="py-3 px-4 text-sm whitespace-nowrap">
                    <Link
                      to={`${lp}/teams/${team.teamId}`}
                      className="font-medium text-[#2563EB] hover:underline"
                    >
                      {team.teamName}
                    </Link>
                  </td>
                  <td className="py-3 px-4 text-sm whitespace-nowrap">
                    <div className="bg-[#2563EB]/10 text-[#2563EB] text-sm font-medium px-2 py-1 rounded-none inline-flex items-center">
                      {team.totalPlayersToday}
                    </div>
                  </td>
                  <td className="py-3 px-4 text-sm">
                    <div className="flex flex-wrap gap-1">
                      {Object.entries(nhlTeamCounts).map(([nhlTeam, count]) => (
                        <span
                          key={nhlTeam}
                          className="px-2 py-1 bg-gray-100 text-gray-800 text-xs font-medium rounded"
                        >
                          {nhlTeam}: {count}
                        </span>
                      ))}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default FantasySummary;
