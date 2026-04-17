import { Link } from "react-router-dom";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import Sparkbars from "@/components/common/Sparkbars";
import SeriesForecastHero from "@/components/pulse/SeriesForecastHero";
import { usePulse } from "@/features/pulse";
import { useLeague } from "@/contexts/LeagueContext";
import { getNHLTeamLogoUrl, getNHLTeamShortName } from "@/utils/nhlTeams";
import type { MyGameTonight } from "@/features/pulse";

const PulsePage = () => {
  const { pulse, isLoading, error, hasLive } = usePulse();
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  if (isLoading) {
    return <LoadingSpinner size="large" message="Loading pulse..." />;
  }
  if (error || !pulse) {
    return <ErrorMessage message="Failed to load pulse data." />;
  }

  const { myTeam, seriesForecast, myGamesTonight, leagueBoard, hasGamesToday } =
    pulse;

  return (
    <div className="space-y-6">
      {hasLive && (
        <div className="bg-[#DC2626] text-white px-4 py-1.5 text-[10px] uppercase tracking-widest font-bold text-center">
          Live — auto-refreshing every 30s
        </div>
      )}

      {/* Flagship: Series Forecast */}
      <SeriesForecastHero
        forecasts={seriesForecast}
        myTeamId={myTeam?.teamId ?? null}
      />

      {/* Today's Pulse — quick stats */}
      {myTeam && (
        <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <header className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold uppercase tracking-wider text-sm">
              Today's Pulse
            </h2>
          </header>
          <div className="p-6 flex items-center justify-between">
            <div>
              <Link
                to={`${lp}/teams/${myTeam.teamId}`}
                className="text-xl font-extrabold uppercase tracking-wider hover:text-[#2563EB]"
              >
                {myTeam.teamName}
              </Link>
              <p className="text-sm text-gray-500 mt-1">
                {myTeam.playersActiveToday}/{myTeam.totalRosterSize} players active today
              </p>
            </div>
            <div className="flex items-center gap-6 text-right">
              <StatCol label="Rank" value={`#${myTeam.rank}`} />
              <StatCol label="Total" value={myTeam.totalPoints.toString()} />
              <StatCol
                label="Today"
                value={myTeam.pointsToday.toString()}
                accent
              />
            </div>
          </div>
        </section>
      )}

      {/* My Games Tonight */}
      {hasGamesToday && myGamesTonight.length > 0 && (
        <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <header className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold uppercase tracking-wider text-sm">
              My Players In Action
            </h2>
          </header>
          <div className="p-4 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {myGamesTonight.map((g) => (
              <GameTonightCard key={g.gameId} game={g} />
            ))}
          </div>
        </section>
      )}

      {!hasGamesToday && (
        <div className="bg-white border-2 border-gray-200 p-6 text-center">
          <p className="text-gray-500 text-sm uppercase tracking-wider">
            No games scheduled today
          </p>
        </div>
      )}

      {/* League Live Board with sparklines */}
      {leagueBoard.length > 0 && (
        <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <header className="bg-white px-6 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold uppercase tracking-wider text-sm">
              League Live Board
            </h2>
          </header>
          <div className="divide-y divide-gray-100">
            <div className="grid grid-cols-[2rem_1fr_4rem_4rem_4rem_5rem] gap-2 px-4 py-2 text-[10px] uppercase tracking-wider text-gray-400 font-bold">
              <span>#</span>
              <span>Team</span>
              <span className="text-right">Total</span>
              <span className="text-right">Active</span>
              <span className="text-right">Today</span>
              <span className="text-right">5-day</span>
            </div>
            {leagueBoard.map((team) => (
              <div
                key={team.teamId}
                className={`grid grid-cols-[2rem_1fr_4rem_4rem_4rem_5rem] gap-2 px-4 py-2.5 text-sm items-center ${
                  team.isMyTeam
                    ? "bg-[#FACC15]/10 border-l-4 border-[#FACC15]"
                    : ""
                }`}
              >
                <span
                  className={`font-bold ${
                    team.isMyTeam ? "" : "text-gray-400"
                  }`}
                >
                  {team.rank}
                </span>
                <Link
                  to={`${lp}/teams/${team.teamId}`}
                  className={`truncate hover:text-[#2563EB] ${
                    team.isMyTeam ? "font-bold" : "font-medium"
                  }`}
                >
                  {team.teamName}
                </Link>
                <span className="text-right tabular-nums font-bold">
                  {team.totalPoints}
                </span>
                <span className="text-right tabular-nums text-gray-500">
                  {team.playersActiveToday}
                </span>
                <span
                  className={`text-right tabular-nums ${
                    team.isMyTeam ? "font-bold text-[#2563EB]" : ""
                  }`}
                >
                  {team.pointsToday}
                </span>
                <span className="flex justify-end">
                  <Sparkbars values={team.sparkline} label="last 5 days" />
                </span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
};

function StatCol({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: boolean;
}) {
  return (
    <div>
      <div className="text-[10px] uppercase tracking-wider text-gray-400">
        {label}
      </div>
      <div
        className={`text-2xl font-extrabold ${
          accent ? "text-[#2563EB]" : ""
        }`}
      >
        {value}
      </div>
    </div>
  );
}

function GameTonightCard({ game }: { game: MyGameTonight }) {
  const isLive = game.gameState.toUpperCase() === "LIVE";
  const isFinal =
    game.gameState.toUpperCase() === "FINAL" ||
    game.gameState.toUpperCase() === "OFF";
  return (
    <div className="border-2 border-gray-300 p-3">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-1.5">
          <img
            src={getNHLTeamLogoUrl(game.awayTeam)}
            className="w-5 h-5"
            alt=""
          />
          <span className="font-bold text-xs uppercase">
            {getNHLTeamShortName(game.awayTeam)}
          </span>
          {game.awayScore !== null && (
            <span className="font-bold text-xs tabular-nums ml-1">
              {game.awayScore}
            </span>
          )}
          <span className="text-gray-400 text-xs">@</span>
          {game.homeScore !== null && (
            <span className="font-bold text-xs tabular-nums mr-1">
              {game.homeScore}
            </span>
          )}
          <span className="font-bold text-xs uppercase">
            {getNHLTeamShortName(game.homeTeam)}
          </span>
          <img
            src={getNHLTeamLogoUrl(game.homeTeam)}
            className="w-5 h-5"
            alt=""
          />
        </div>
        <span
          className={`text-[10px] tracking-widest uppercase ${
            isLive
              ? "text-[#DC2626] font-bold"
              : isFinal
                ? "text-gray-500"
                : "text-gray-400"
          }`}
        >
          {isLive && game.period ? game.period : isFinal ? "Final" : formatTime(game.startTimeUtc)}
        </span>
      </div>

      {game.seriesContext && (
        <p
          className={`text-[10px] uppercase tracking-wider mb-2 ${
            game.isElimination
              ? "text-[#DC2626] font-bold"
              : "text-gray-500"
          }`}
        >
          {game.seriesContext}
          {game.isElimination && " — ELIMINATION"}
        </p>
      )}

      <div className="space-y-1">
        {game.myPlayers.map((p) => (
          <div
            key={p.nhlId}
            className="flex items-center justify-between text-xs"
          >
            <span className="truncate flex-1">{p.name}</span>
            <span className="text-gray-400 ml-1 shrink-0">{p.position}</span>
            {(p.goals !== 0 || p.assists !== 0) && (
              <span className="ml-2 font-bold text-[#2563EB] tabular-nums">
                {p.goals}G {p.assists}A
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatTime(startTime: string): string {
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
