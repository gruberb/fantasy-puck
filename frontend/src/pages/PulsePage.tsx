import { Link } from "react-router-dom";
import { ErrorMessage, LoadingSpinner } from "@gruberb/fun-ui";
import SeriesForecastHero from "@/components/pulse/SeriesForecastHero";
import YourReadSection from "@/components/pulse/YourReadSection";
import RosterBreakdownSection from "@/components/pulse/RosterBreakdownSection";
import YourLeagueSection from "@/components/pulse/YourLeagueSection";
import { usePulse } from "@/features/pulse";
import { RaceOddsSection } from "@/features/race-odds/components/RaceOddsSection";
import { MyStakes } from "@/features/race-odds/components/MyStakes";
import { useLeague } from "@/contexts/LeagueContext";
import { getNHLTeamLogoUrl, getNHLTeamShortName, nhlPlayerProfileUrl } from "@/utils/nhlTeams";
import type { MyGameTonight } from "@/features/pulse";

const PulsePage = () => {
  const { pulse, isLoading, error } = usePulse();
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  if (isLoading) {
    return <LoadingSpinner size="large" message="Loading pulse..." />;
  }
  if (error || !pulse) {
    return <ErrorMessage message="Failed to load pulse data." />;
  }

  const {
    myTeam,
    seriesForecast,
    myGamesTonight,
    hasGamesToday,
    myTeamDiagnosis,
    leagueOutlook,
  } = pulse;

  return (
    <div className="space-y-6">
      {/* Tonight — merged "today's team snapshot" + "my players in action".
          First thing the caller sees: their standing + which of their players
          are playing today. */}
      {myTeam && (
        <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <header className="bg-[#1A1A1A] text-white px-6 py-3">
            <h2 className="font-extrabold uppercase tracking-wider text-sm">
              Tonight
            </h2>
          </header>
          <div className="p-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <Link
                to={`${lp}/teams/${myTeam.teamId}`}
                className="text-xl font-extrabold uppercase tracking-wider hover:text-[#2563EB]"
              >
                {myTeam.teamName}
              </Link>
              <p className="text-sm text-gray-500 mt-1">
                {myTeam.playersActiveToday}/{myTeam.totalRosterSize} players have an NHL game today
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
          {hasGamesToday && myGamesTonight.length > 0 ? (
            <div className="p-4 border-t-2 border-[#1A1A1A] grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              {myGamesTonight.map((g) => (
                <GameTonightCard key={g.gameId} game={g} />
              ))}
            </div>
          ) : (
            <div className="border-t-2 border-[#1A1A1A] p-6 text-center">
              <p className="text-gray-500 text-sm uppercase tracking-wider">
                No games scheduled today
              </p>
            </div>
          )}
        </section>
      )}

      {/* Your Read — descriptive diagnosis narrative + concentration
          chips. Hidden when the caller has no team in this league or
          when the projection pipeline couldn't build a bundle (e.g.
          regular season). */}
      {myTeamDiagnosis && <YourReadSection data={myTeamDiagnosis} />}

      {/* Roster Breakdown — full per-player table. Separated from
          Your Read so the narrative block doesn't inherit the
          table's 14-column horizontal scroll. */}
      {myTeamDiagnosis && (
        <RosterBreakdownSection players={myTeamDiagnosis.players} />
      )}

      {/* Your League — leader, points distribution, top-3 expected
          finishers. Hidden when the cache is cold or we don't yet
          have league totals. */}
      {leagueOutlook && <YourLeagueSection data={leagueOutlook} />}

      {/* Race Odds — per-league Monte Carlo projections. Includes the
          Head-to-Head card internally, so no separate rivalry section here. */}
      <RaceOddsSection myTeamId={myTeam?.teamId ?? null} />

      {/* My Stakes — "which NHL series am I rooting for?" — every NHL team
          the caller rosters, sorted by impact on their race. */}
      {myTeam && (
        <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
          <header className="bg-[#1A1A1A] text-white px-6 py-3">
            <h2 className="font-extrabold uppercase tracking-wider text-sm">
              My Stakes
            </h2>
          </header>
          <div className="p-6">
            <MyStakes
              myTeam={
                seriesForecast.find((f) => f.teamId === myTeam.teamId) ?? null
              }
            />
          </div>
        </section>
      )}

      {/* Series Rosters — each fantasy team's players grouped by NHL series.
          Non-mine teams are collapsed by default. */}
      <SeriesForecastHero
        forecasts={seriesForecast}
        myTeamId={myTeam?.teamId ?? null}
      />

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
            className="flex items-center text-xs gap-2"
          >
            <span className="text-[10px] font-bold uppercase text-gray-500 shrink-0 w-9 tabular-nums">
              {p.nhlTeam}
            </span>
            <a
              href={nhlPlayerProfileUrl(p.nhlId)}
              target="_blank"
              rel="noopener noreferrer"
              className="truncate flex-1 hover:text-[#2563EB] hover:underline min-w-0"
            >
              {p.name}
            </a>
            {/* Fixed-width slots so the position letter always lands in
                the same column, with or without a stat line. */}
            <span className="ml-2 w-14 text-right font-bold text-[#2563EB] tabular-nums shrink-0">
              {(p.goals !== 0 || p.assists !== 0) ? `${p.goals}G ${p.assists}A` : ""}
            </span>
            <span className="ml-1 w-3 text-right text-gray-400 shrink-0">{p.position}</span>
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
