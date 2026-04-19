import { useParams } from "react-router-dom";
import { ErrorMessage, LoadingSpinner } from "@gruberb/fun-ui";
import TeamHeader from "@/components/fantasyTeamDetail/TeamHeader";
import TeamStats from "@/components/fantasyTeamDetail/TeamStats";
import PlayoffStatus from "@/components/fantasyTeamDetail/PlayoffStatus";
import PlayerRoster from "@/components/fantasyTeamDetail/PlayerRoster";
import TeamBetsTable from "@/components/fantasyTeamDetail/TeamBetsTable";
import { useTeamDetail } from "@/features/teams";
import { useLeague } from "@/contexts/LeagueContext";
import { getNHLTeamLogoUrl } from "@/utils/nhlTeams";

const FantasyTeamDetailPage = () => {
  const { teamId } = useParams<{ teamId: string }>();
  const { activeLeagueId } = useLeague();
  const id = parseInt(teamId || "0", 10);

  const {
    team,
    teamPoints,
    processedPlayers,
    currentTeamBets,
    playoffStats,
    teamSleeper,
    isLoading,
    hasError,
  } = useTeamDetail(activeLeagueId, id);

  if (isLoading) {
    return <LoadingSpinner size="large" message="Loading team data..." />;
  }

  if (hasError || !team || !teamPoints) {
    return <ErrorMessage message="Team not found or data unavailable" />;
  }

  return (
    <div className="max-w-6xl mx-auto">
      {/* Team header with navigation */}
      <TeamHeader team={team} teamPoints={teamPoints} />

      {/* Row 1: Team Stats + Sleeper Pick */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        <TeamStats teamPoints={teamPoints} />

        {teamSleeper ? (
          <div className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
            <div className="card-header flex items-center gap-2">
              <h2 className="text-xl font-bold">Sleeper Pick</h2>
            </div>
            <div className="p-6 flex items-center gap-5">
              <img
                src={teamSleeper.imageUrl || `https://assets.nhle.com/mugs/nhl/latest/${teamSleeper.nhlId}.png`}
                alt={teamSleeper.name}
                className="w-20 h-20 rounded-none object-cover bg-gray-100 border-2 border-[#1A1A1A] flex-shrink-0"
                onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }}
              />
              <div className="flex-1 min-w-0">
                <p className="font-bold text-xl text-[#1A1A1A]">{teamSleeper.name}</p>
                <div className="flex items-center gap-2 mt-1.5">
                  <span className="text-xs font-semibold px-2 py-0.5 rounded-none border border-[#1A1A1A] bg-gray-100">{teamSleeper.position}</span>
                  <img src={getNHLTeamLogoUrl(teamSleeper.nhlTeam)} alt={teamSleeper.nhlTeam} className="w-5 h-5" />
                  <span className="text-sm text-gray-600">{teamSleeper.nhlTeam}</span>
                </div>
                <div className="flex gap-6 mt-4">
                  <div className="text-center">
                    <div className="text-[10px] text-gray-400 uppercase tracking-wider">Goals</div>
                    <div className="font-bold text-2xl">{teamSleeper.goals ?? 0}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-[10px] text-gray-400 uppercase tracking-wider">Assists</div>
                    <div className="font-bold text-2xl">{teamSleeper.assists ?? 0}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-[10px] text-gray-400 uppercase tracking-wider">Points</div>
                    <div className="font-bold text-2xl text-[#FACC15]">{(teamSleeper.goals ?? 0) + (teamSleeper.assists ?? 0)}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        ) : (
          <PlayoffStatus
            players={processedPlayers}
            teamsInPlayoffs={playoffStats.teamsInPlayoffs}
            playersInPlayoffs={playoffStats.playersInPlayoffs}
            totalTeams={currentTeamBets.length}
            totalPlayers={processedPlayers.length}
          />
        )}
      </div>

      {/* Row 2: Playoff Status (if sleeper exists, it goes here instead of row 1) */}
      {teamSleeper && (
        <div className="mt-8">
          <PlayoffStatus
            players={processedPlayers}
            teamsInPlayoffs={playoffStats.teamsInPlayoffs}
            playersInPlayoffs={playoffStats.playersInPlayoffs}
            totalTeams={currentTeamBets.length}
            totalPlayers={processedPlayers.length}
          />
        </div>
      )}

      {/* Row 3: Roster + NHL Teams */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8 mt-8">
        <PlayerRoster players={processedPlayers} />
        <TeamBetsTable teamBets={currentTeamBets} />
      </div>
    </div>
  );
};

export default FantasyTeamDetailPage;
