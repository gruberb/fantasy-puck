import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import { usePulseData } from "@/hooks/use-pulse-data";
import { Link } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import { getNHLTeamLogoUrl, getNHLTeamShortName } from "@/utils/nhlTeams";
import type { PlayerInAction } from "@/types/matchDay";
import type { Game } from "@/types/games";

const PulsePage = () => {
  const { pulse, isLoading, error } = usePulseData();
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  if (isLoading) {
    return <LoadingSpinner size="large" message="Loading pulse..." />;
  }

  if (error) {
    return <ErrorMessage message="Failed to load pulse data." />;
  }

  const { myTeam, opponents, hasGamesToday, games } = pulse;
  const myPlayersByGame = groupPlayersByGame(myTeam?.players ?? [], games);

  return (
    <div className="space-y-6">
      {/* My Status */}
      {myTeam ? (
        <div className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <div className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold text-[#1A1A1A] uppercase tracking-wider text-sm">My Team</h2>
          </div>
          <div className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <Link to={`${lp}/teams/${myTeam.teamId}`} className="text-xl font-extrabold uppercase tracking-wider hover:text-[#2563EB]">
                  {myTeam.teamName}
                </Link>
                <p className="text-sm text-gray-500 mt-1">
                  {myTeam.playersActiveTonight} player{myTeam.playersActiveTonight !== 1 ? "s" : ""} active tonight
                </p>
              </div>
              <div className="flex items-center gap-6 text-right">
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-gray-400">Rank</div>
                  <div className="text-2xl font-extrabold">#{myTeam.rank}</div>
                </div>
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-gray-400">Total</div>
                  <div className="text-2xl font-extrabold">{myTeam.totalPoints}</div>
                </div>
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-gray-400">Today</div>
                  <div className="text-2xl font-extrabold text-[#2563EB]">{myTeam.pointsToday}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div className="bg-white border-2 border-[#1A1A1A] p-6 text-center">
          <p className="text-gray-500 text-sm">No team found for this league.</p>
        </div>
      )}

      {/* My Players Tonight — bigger cards */}
      {myTeam && hasGamesToday && myTeam.players.length > 0 && (
        <div className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <div className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A] flex items-center justify-between">
            <h2 className="font-extrabold text-[#1A1A1A] uppercase tracking-wider text-sm">
              My Players Tonight
            </h2>
            <span className="text-gray-400 text-sm">
              {myTeam.playersActiveTonight} across {myPlayersByGame.length} game{myPlayersByGame.length !== 1 ? "s" : ""}
            </span>
          </div>
          <div className="p-4 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {myTeam.players.map((player, i) => (
              <div key={i} className="border border-gray-200 p-3 flex items-start gap-3">
                {player.imageUrl ? (
                  <img src={player.imageUrl} alt="" className="w-12 h-12 bg-gray-100 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} />
                ) : (
                  <div className="w-12 h-12 bg-gray-100 flex items-center justify-center flex-shrink-0 text-xs font-bold text-gray-400">
                    {player.playerName.split(" ").map(n => n[0]).join("")}
                  </div>
                )}
                <div className="flex-1 min-w-0">
                  <p className="font-bold text-sm truncate">{player.playerName}</p>
                  <div className="flex items-center gap-1 text-xs text-gray-500 mt-0.5">
                    <img src={getNHLTeamLogoUrl(player.nhlTeam)} className="w-4 h-4" alt="" />
                    <span>{player.nhlTeam}</span>
                    <span>&middot;</span>
                    <span>{player.position}</span>
                  </div>
                  <div className="flex gap-2 mt-2">
                    <StatBadge label="G" value={player.playoffGoals} />
                    <StatBadge label="A" value={player.playoffAssists} />
                    <StatBadge label="PTS" value={player.playoffPoints} />
                    {player.form && (
                      <StatBadge label="L5" value={player.form.points} accent />
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* My Games Tonight — compact game cards */}
      {hasGamesToday && myPlayersByGame.length > 0 && (
        <div className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <div className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold text-[#1A1A1A] uppercase tracking-wider text-sm">
              My Games Tonight
            </h2>
          </div>
          <div className="p-4 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {myPlayersByGame.map(({ game, players }) => (
              <div key={game.id} className="border border-gray-200 p-3">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-1.5">
                    <img src={getNHLTeamLogoUrl(game.awayTeam)} className="w-5 h-5" alt="" />
                    <span className="font-bold text-xs uppercase">{getNHLTeamShortName(game.awayTeam)}</span>
                    <span className="text-gray-400 text-xs">@</span>
                    <span className="font-bold text-xs uppercase">{getNHLTeamShortName(game.homeTeam)}</span>
                    <img src={getNHLTeamLogoUrl(game.homeTeam)} className="w-5 h-5" alt="" />
                  </div>
                  <span className="text-[10px] text-gray-400">{formatGameTime(game.startTime)}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-xs text-gray-500">
                    {players.length} player{players.length !== 1 ? "s" : ""}
                  </span>
                  <div className="flex -space-x-1">
                    {players.slice(0, 4).map((p, i) => (
                      <img
                        key={i}
                        src={getNHLTeamLogoUrl(p.nhlTeam)}
                        className="w-4 h-4 border border-white rounded-full"
                        alt=""
                        title={p.playerName}
                      />
                    ))}
                    {players.length > 4 && (
                      <span className="w-4 h-4 bg-gray-200 border border-white rounded-full flex items-center justify-center text-[8px] font-bold">
                        +{players.length - 4}
                      </span>
                    )}
                  </div>
                </div>
                <div className="mt-2 space-y-1">
                  {players.map((p, i) => (
                    <div key={i} className="flex items-center text-xs">
                      <span className="truncate flex-1 text-gray-700">{p.playerName}</span>
                      <span className="text-gray-400 ml-1">{p.position}</span>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {!hasGamesToday && (
        <div className="bg-white border-2 border-gray-200 p-6 text-center">
          <p className="text-gray-500 text-sm uppercase tracking-wider">No games scheduled today</p>
        </div>
      )}

      {/* League Board */}
      {opponents.length > 0 && (
        <div className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <div className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold text-[#1A1A1A] uppercase tracking-wider text-sm">League Board</h2>
          </div>
          <div className="divide-y divide-gray-100">
            <div className="grid grid-cols-[2rem_1fr_4rem_4rem_4rem] gap-2 px-4 py-2 text-[10px] uppercase tracking-wider text-gray-400 font-bold">
              <span>#</span>
              <span>Team</span>
              <span className="text-right">Total</span>
              <span className="text-right">Active</span>
              <span className="text-right">Today</span>
            </div>
            {myTeam && (
              <div className="grid grid-cols-[2rem_1fr_4rem_4rem_4rem] gap-2 px-4 py-2.5 text-sm bg-[#FACC15]/10 border-l-4 border-[#FACC15]">
                <span className="font-bold">{myTeam.rank}</span>
                <Link to={`${lp}/teams/${myTeam.teamId}`} className="font-bold truncate hover:text-[#2563EB]">
                  {myTeam.teamName}
                </Link>
                <span className="text-right font-bold tabular-nums">{myTeam.totalPoints}</span>
                <span className="text-right tabular-nums">{myTeam.playersActiveTonight}</span>
                <span className="text-right font-bold tabular-nums text-[#2563EB]">{myTeam.pointsToday}</span>
              </div>
            )}
            {opponents.map((team) => (
              <div key={team.teamId} className="grid grid-cols-[2rem_1fr_4rem_4rem_4rem] gap-2 px-4 py-2.5 text-sm">
                <span className="text-gray-400 font-bold">{team.rank}</span>
                <Link to={`${lp}/teams/${team.teamId}`} className="font-medium truncate hover:text-[#2563EB]">
                  {team.teamName}
                </Link>
                <span className="text-right tabular-nums">{team.totalPoints}</span>
                <span className="text-right tabular-nums text-gray-400">{team.playersActiveTonight}</span>
                <span className="text-right tabular-nums">{team.pointsToday}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

// -- Sub-components -----------------------------------------------------------

function StatBadge({ label, value, accent }: { label: string; value: number; accent?: boolean }) {
  return (
    <div className={`text-center px-2 py-0.5 text-[10px] ${accent ? "bg-[#FACC15]/20" : "bg-gray-100"}`}>
      <span className="font-bold">{value}</span>
      <span className="text-gray-400 ml-0.5">{label}</span>
    </div>
  );
}

// -- Helpers ------------------------------------------------------------------

function groupPlayersByGame(players: PlayerInAction[], games: Game[]) {
  const result: { game: Game; players: PlayerInAction[] }[] = [];
  for (const game of games) {
    const matching = players.filter(
      (p) => p.nhlTeam === game.homeTeam || p.nhlTeam === game.awayTeam,
    );
    if (matching.length > 0) {
      result.push({ game, players: matching });
    }
  }
  result.sort((a, b) => b.players.length - a.players.length);
  return result;
}

function formatGameTime(startTime: string): string {
  try {
    return new Date(startTime).toLocaleTimeString("en-US", {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
  } catch {
    return startTime;
  }
}

export default PulsePage;
