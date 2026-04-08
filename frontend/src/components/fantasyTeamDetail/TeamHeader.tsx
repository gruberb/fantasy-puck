import { NHLTeam } from "@/types/fantasyTeams";
import { FantasyTeamPoints } from "@/types/fantasyTeams";
import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";

interface TeamHeaderProps {
  team: NHLTeam;
  teamPoints: FantasyTeamPoints;
}

export default function TeamHeader({ team, teamPoints }: TeamHeaderProps) {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return (
    <>
      <div className="mb-6">
        <Link to={`${lp}/teams`} className="btn btn-secondary">
          &larr; Fantasy Teams Overview
        </Link>
      </div>

      <div className="flex items-center space-x-4 mb-4">
        {team.teamLogo ? (
          <img
            src={team.teamLogo}
            alt={`${team.name} logo`}
            className="w-24 h-24 object-contain"
          />
        ) : (
          <div className="w-24 h-24 bg-gray-200 flex items-center justify-center rounded-none">
            <span className="text-3xl font-bold text-gray-500">
              {team.name.substring(0, 2).toUpperCase()}
            </span>
          </div>
        )}
        <div>
          <h1 className="text-3xl font-bold">{team.name}</h1>
          <p className="text-xl text-gray-600">Fantasy Team</p>
          <p className="text-lg text-gray-500">
            Total Points: {teamPoints.teamTotals.totalPoints}
          </p>
        </div>
      </div>
    </>
  );
}
