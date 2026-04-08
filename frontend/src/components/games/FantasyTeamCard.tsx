import { useState } from "react";
import { Link } from "react-router-dom";
import { FantasyTeamInAction, PlayerInAction } from "@/types/matchDay";
import { useLeague } from "@/contexts/LeagueContext";
import PlayerWithStats from "./PlayerWithStats";
import { getTeamGradient, getTeamPrimaryColor } from "@/utils/teamStyles";

interface FantasyTeamCardProps {
  team: FantasyTeamInAction;
}

const FantasyTeamCard = ({ team }: FantasyTeamCardProps) => {
  const [expanded, setExpanded] = useState(false);
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const { gradient: teamGradient } = getTeamGradient(team.teamName);

  // Categorize players by NHL team
  const playersByTeam = team.playersInAction.reduce(
    (acc: Record<string, PlayerInAction[]>, player) => {
      if (!acc[player.nhlTeam]) {
        acc[player.nhlTeam] = [];
      }
      acc[player.nhlTeam].push(player);
      return acc;
    },
    {},
  );

  // Calculate team stats
  const teamStats = {
    totalPlayoffPoints: team.playersInAction.reduce(
      (sum, player) => sum + (player.playoffPoints || 0),
      0,
    ),
    totalPlayoffGoals: team.playersInAction.reduce(
      (sum, player) => sum + (player.playoffGoals || 0),
      0,
    ),
    totalPlayoffAssists: team.playersInAction.reduce(
      (sum, player) => sum + (player.playoffAssists || 0),
      0,
    ),
    // Calculate form points (last 5 games)
    totalFormPoints: team.playersInAction.reduce(
      (sum, player) => sum + (player.form?.points || 0),
      0,
    ),
  };

  // Get top performing player
  const topPlayer = [...team.playersInAction].sort(
    (a, b) => (b.playoffPoints || 0) - (a.playoffPoints || 0),
  )[0];

  return (
    <div className="bg-white rounded-none border border-gray-200 overflow-hidden">
      {/* Team header */}
      <div
        className="p-4 flex items-center justify-between"
        style={{ background: teamGradient }}
      >
        <div className="flex items-center">
          <div className="text-white">
            <h3 className="text-lg font-bold">
              <Link
                to={`${lp}/teams/${team.teamId}`}
                className="hover:underline"
              >
                {team.teamName}
              </Link>
            </h3>
            <div className="text-sm opacity-90">
              {team.totalPlayersToday} Players in Action Today
            </div>
          </div>
        </div>

        {/* Team stats badges */}
        <div className="hidden md:flex space-x-2">
          <div className="bg-white/20 text-white px-3 py-1 rounded-none text-sm font-medium">
            {teamStats.totalPlayoffPoints} Playoff Pts
          </div>
          <div className="bg-white/20 text-white px-3 py-1 rounded-none text-sm font-medium">
            {teamStats.totalFormPoints} Form Pts
          </div>
        </div>

        <button
          onClick={() => setExpanded(!expanded)}
          className="bg-white/30 text-white p-2 rounded-none hover:bg-white/40 transition-colors"
        >
          <svg
            className={`w-5 h-5 transform transition-transform ${
              expanded ? "rotate-180" : ""
            }`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>
      </div>

      {/* Team quick stats - visible even when collapsed */}
      <div className="p-4 border-b border-gray-200">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="text-center">
            <div className="text-sm text-gray-500">NHL Teams</div>
            <div className="font-bold text-lg">
              {Object.keys(playersByTeam).length}
            </div>
          </div>
          <div className="text-center">
            <div className="text-sm text-gray-500">Playoff Points</div>
            <div className="font-bold text-lg">
              {teamStats.totalPlayoffPoints}
            </div>
          </div>
          <div className="text-center">
            <div className="text-sm text-gray-500">Top Performer</div>
            <div className="font-bold text-sm truncate">
              {topPlayer && (
                <a
                  href={`https://www.nhl.com/player/${topPlayer.nhlId}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-[#2563EB]"
                >
                  {topPlayer.playerName}
                </a>
              )}
            </div>
          </div>
          <div className="text-center">
            <div className="text-sm text-gray-500">Form Points</div>
            <div className="font-bold text-lg">{teamStats.totalFormPoints}</div>
          </div>
        </div>
      </div>

      {/* Team players */}
      {expanded && (
        <div className="p-4 bg-gray-50">
          {Object.entries(playersByTeam).map(([nhlTeam, players]) => (
            <div key={nhlTeam} className="mb-4 last:mb-0">
              <div className="flex items-center mb-2">
                <div
                  className="w-3 h-3 rounded-none mr-2"
                  style={{ backgroundColor: getTeamPrimaryColor(nhlTeam) }}
                ></div>
                <h4 className="font-medium text-gray-700">{nhlTeam} Players</h4>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
                {players.map((player, index) => (
                  <PlayerWithStats key={index} player={player} />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default FantasyTeamCard;
