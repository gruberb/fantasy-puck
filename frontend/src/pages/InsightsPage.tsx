import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import { useInsights } from "@/features/insights";
import { getNHLTeamFullName, getNHLTeamShortName, getNHLTeamLogoUrl } from "@/utils/nhlTeams";
import type {
  HotPlayerSignal,
  ContenderSignal,
  TodaysGameSignal,
  FantasyRaceSignal,
  SleeperAlertSignal,
} from "@/features/insights";

const InsightsPage = () => {
  const { insights, isLoading, error, refetch } = useInsights();

  if (isLoading) {
    return <LoadingSpinner size="large" message="Generating insights..." />;
  }

  if (error || !insights) {
    return (
      <ErrorMessage
        message="Failed to load insights. Please try again."
        onRetry={() => refetch()}
      />
    );
  }

  const { narratives, signals } = insights;

  return (
    <div className="space-y-6">

      {/* What to Watch Today */}
      {(narratives.todaysWatch || signals.todaysGames.length > 0) && (
        <InsightCard accent="#2563EB" title="What to Watch Today">
          <Narrative text={narratives.todaysWatch} />
          {signals.todaysGames.length > 0 && (
            <div className="mt-4 grid grid-cols-1 lg:grid-cols-2 gap-4">
              {signals.todaysGames.map((game, i) => (
                <GameSignalCard
                  key={i}
                  game={game}
                  narrative={narratives.gameNarratives?.[i]}
                />
              ))}
            </div>
          )}
        </InsightCard>
      )}

      {/* Hot Hands */}
      {(narratives.hotPlayers || signals.hotPlayers.length > 0) && (
        <InsightCard accent="#EF4444" title="Hot Hands">
          <Narrative text={narratives.hotPlayers} />
          {signals.hotPlayers.length > 0 && (
            <div className="mt-4 flex gap-3 overflow-x-auto pb-2">
              {signals.hotPlayers.map((player, i) => (
                <HotPlayerCard key={i} player={player} rank={i + 1} />
              ))}
            </div>
          )}
        </InsightCard>
      )}

      {/* Cup Contenders */}
      {(narratives.cupContenders || signals.cupContenders.length > 0) && (
        <InsightCard accent="#16A34A" title="Cup Contenders">
          <Narrative text={narratives.cupContenders} />
          {signals.cupContenders.length > 0 && (
            <div className="mt-4 grid grid-cols-1 sm:grid-cols-3 gap-3">
              {signals.cupContenders.map((team, i) => (
                <ContenderCard key={i} contender={team} />
              ))}
            </div>
          )}
        </InsightCard>
      )}

      {/* Fantasy Race */}
      {(narratives.fantasyRace || signals.fantasyRace.length > 0) && (
        <InsightCard accent="#FACC15" title="Fantasy Race">
          <Narrative text={narratives.fantasyRace} />
          {signals.fantasyRace.length > 0 && (
            <div className="mt-4">
              <div className="divide-y divide-gray-100 border border-gray-200">
                {signals.fantasyRace.map((team) => (
                  <div key={team.teamName} className="flex items-center gap-3 px-3 py-2 text-sm">
                    <span className="w-6 h-6 flex items-center justify-center bg-gray-100 font-bold text-xs">{team.rank}</span>
                    <span className="font-bold text-[#1A1A1A] flex-1 uppercase tracking-wider text-xs">{team.teamName}</span>
                    <span className="font-mono text-sm font-bold">{team.totalPoints} pts</span>
                    {team.playersActiveToday > 0 && (
                      <span className="text-xs bg-green-100 text-green-800 px-2 py-0.5 font-medium">
                        {team.playersActiveToday} active
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}
        </InsightCard>
      )}

      {/* Sleeper Watch */}
      {(narratives.sleeperWatch || signals.sleeperAlerts.length > 0) && (
        <InsightCard accent="#8B5CF6" title="Sleeper Watch">
          <Narrative text={narratives.sleeperWatch} />
          {signals.sleeperAlerts.length > 0 && (
            <div className="mt-4 grid grid-cols-1 sm:grid-cols-2 gap-3">
              {signals.sleeperAlerts.map((sleeper, i) => (
                <SleeperCard key={i} sleeper={sleeper} />
              ))}
            </div>
          )}
        </InsightCard>
      )}

      {/* News Headlines */}
      {signals.newsHeadlines.length > 0 && (
        <InsightCard accent="#1A1A1A" title="Around the League">
          <ul className="space-y-2 mt-2">
            {signals.newsHeadlines.map((headline, i) => (
              <li key={i} className="text-sm text-gray-700 flex items-start gap-2">
                <span className="text-gray-400 mt-0.5 flex-shrink-0">&bull;</span>
                {headline}
              </li>
            ))}
          </ul>
        </InsightCard>
      )}
    </div>
  );
};

// -- Sub-components ----------------------------------------------------------

function InsightCard({
  accent,
  title,
  children,
}: {
  accent: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
      <div className="px-6 py-3 border-b-2 border-[#1A1A1A]" style={{ backgroundColor: accent }}>
        <h2 className="font-extrabold text-white uppercase tracking-wider text-sm">
          {title}
        </h2>
      </div>
      <div className="p-6">{children}</div>
    </div>
  );
}

/** Renders text with **bold** markers parsed into <strong> tags */
function Narrative({ text }: { text: string }) {
  if (!text) return null;
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return (
    <p className="text-sm leading-relaxed text-gray-800">
      {parts.map((part, i) => {
        if (part.startsWith("**") && part.endsWith("**")) {
          return <strong key={i} className="font-bold text-[#1A1A1A]">{part.slice(2, -2)}</strong>;
        }
        return <span key={i}>{part}</span>;
      })}
    </p>
  );
}

function GameSignalCard({ game, narrative }: { game: TodaysGameSignal; narrative?: string }) {
  const awayMeta = [game.awayStreak, game.awayL10 ? `L10: ${game.awayL10}` : null, game.awayLastResult].filter(Boolean);
  const homeMeta = [game.homeStreak, game.homeL10 ? `L10: ${game.homeL10}` : null, game.homeLastResult].filter(Boolean);

  return (
    <div className={`border-2 rounded-none overflow-hidden ${game.isElimination ? "border-red-400" : "border-gray-200"}`}>
      {/* Header: Teams */}
      <div className={`px-4 py-3 ${game.isElimination ? "bg-red-50" : "bg-gray-50"}`}>
        <div className="flex items-start">
          <div className="flex items-start gap-2 flex-1 min-w-0">
            <img src={getNHLTeamLogoUrl(game.awayTeam)} alt={game.awayTeam} className="w-7 h-7 flex-shrink-0 mt-0.5" />
            <div className="min-w-0">
              <p className="font-extrabold text-sm uppercase tracking-wider leading-tight">{getNHLTeamShortName(game.awayTeam)}</p>
              {game.awayRecord && <p className="text-[11px] text-gray-400 mt-0.5">{game.awayRecord}</p>}
              {awayMeta.length > 0 && (
                <div className="flex items-center gap-1.5 mt-0.5 flex-wrap">
                  {game.awayStreak && (
                    <span className={`text-[10px] font-bold px-1 py-px leading-none ${game.awayStreak.startsWith("W") ? "bg-green-100 text-green-700" : "bg-red-100 text-red-700"}`}>
                      {formatStreak(game.awayStreak)}
                    </span>
                  )}
                  {game.awayL10 && <span className="text-[10px] text-gray-400">L10: {game.awayL10}</span>}
                  {game.awayLastResult && <span className="text-[10px] text-gray-400">{game.awayLastResult}</span>}
                </div>
              )}
            </div>
          </div>
          <div className="text-center px-2 flex-shrink-0">
            <span className="text-[10px] text-gray-400 uppercase">@</span>
            {game.venue && <p className="text-[9px] text-gray-300 mt-0.5 max-w-[100px] leading-tight">{game.venue}</p>}
          </div>
          <div className="flex items-start gap-2 flex-1 min-w-0 justify-end text-right">
            <div className="min-w-0">
              <p className="font-extrabold text-sm uppercase tracking-wider leading-tight">{getNHLTeamShortName(game.homeTeam)}</p>
              {game.homeRecord && <p className="text-[11px] text-gray-400 mt-0.5">{game.homeRecord}</p>}
              {homeMeta.length > 0 && (
                <div className="flex items-center gap-1.5 mt-0.5 justify-end flex-wrap">
                  {game.homeLastResult && <span className="text-[10px] text-gray-400">{game.homeLastResult}</span>}
                  {game.homeL10 && <span className="text-[10px] text-gray-400">L10: {game.homeL10}</span>}
                  {game.homeStreak && (
                    <span className={`text-[10px] font-bold px-1 py-px leading-none ${game.homeStreak.startsWith("W") ? "bg-green-100 text-green-700" : "bg-red-100 text-red-700"}`}>
                      {formatStreak(game.homeStreak)}
                    </span>
                  )}
                </div>
              )}
            </div>
            <img src={getNHLTeamLogoUrl(game.homeTeam)} alt={game.homeTeam} className="w-7 h-7 flex-shrink-0 mt-0.5" />
          </div>
        </div>
        {game.seriesContext && <p className="text-[10px] text-gray-400 mt-1.5 text-center">{game.seriesContext}</p>}
        {game.isElimination && (
          <div className="text-center mt-1">
            <span className="inline-block text-[10px] font-bold uppercase bg-red-600 text-white px-2 py-0.5 leading-none">Elimination</span>
          </div>
        )}
      </div>

      {/* Per-game narrative */}
      {narrative && (
        <div className="px-4 py-3 border-t border-gray-100">
          <Narrative text={narrative} />
        </div>
      )}

      {/* Players to Watch + Goalies - compact two-column layout */}
      {(game.pointsLeaders || game.homeGoalie) && (
        <div className="px-4 py-3 border-t border-gray-100">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-3 gap-y-4">
            {/* Away side */}
            <div>
              {game.pointsLeaders && (
                <>
                  <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-1.5">
                    {getNHLTeamShortName(game.awayTeam)} - Last 5
                  </p>
                  {[
                    { label: "PTS", leaders: game.pointsLeaders },
                    { label: "G", leaders: game.goalsLeaders },
                    { label: "A", leaders: game.assistsLeaders },
                  ].filter(r => r.leaders).map(({ label, leaders }) => {
                    const player = leaders![0];
                    return (
                      <div key={label} className="flex items-center gap-1.5 mb-1 text-xs">
                        <img src={player.headshot} alt="" className="w-5 h-5 rounded-full bg-gray-100 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} />
                        <span className="font-medium truncate flex-1 min-w-0">{player.name}</span>
                        <span className="font-bold tabular-nums flex-shrink-0">{player.value}</span>
                        <span className="text-[10px] text-gray-400 w-6 flex-shrink-0">{label}</span>
                      </div>
                    );
                  })}
                </>
              )}
              {game.awayGoalie && (
                <div className="mt-2 pt-2 border-t border-gray-100">
                  <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-1">Goalie</p>
                  <p className="text-xs font-medium">{game.awayGoalie.name}</p>
                  <p className="text-[11px] text-gray-500 tabular-nums">{game.awayGoalie.record} &middot; {game.awayGoalie.gaa.toFixed(2)} GAA</p>
                  <p className="text-[11px] text-gray-500 tabular-nums">.{Math.round(game.awayGoalie.savePctg * 1000)} SV%</p>
                </div>
              )}
            </div>
            {/* Home side */}
            <div>
              {game.pointsLeaders && (
                <>
                  <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-1.5">
                    {getNHLTeamShortName(game.homeTeam)} - Last 5
                  </p>
                  {[
                    { label: "PTS", leaders: game.pointsLeaders },
                    { label: "G", leaders: game.goalsLeaders },
                    { label: "A", leaders: game.assistsLeaders },
                  ].filter(r => r.leaders).map(({ label, leaders }) => {
                    const player = leaders![1];
                    return (
                      <div key={label} className="flex items-center gap-1.5 mb-1 text-xs">
                        <img src={player.headshot} alt="" className="w-5 h-5 rounded-full bg-gray-100 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} />
                        <span className="font-medium truncate flex-1 min-w-0">{player.name}</span>
                        <span className="font-bold tabular-nums flex-shrink-0">{player.value}</span>
                        <span className="text-[10px] text-gray-400 w-6 flex-shrink-0">{label}</span>
                      </div>
                    );
                  })}
                </>
              )}
              {game.homeGoalie && (
                <div className="mt-2 pt-2 border-t border-gray-100">
                  <p className="text-[10px] font-bold uppercase tracking-wider text-gray-300 mb-1">Goalie</p>
                  <p className="text-xs font-medium">{game.homeGoalie.name}</p>
                  <p className="text-[11px] text-gray-500 tabular-nums">{game.homeGoalie.record} &middot; {game.homeGoalie.gaa.toFixed(2)} GAA</p>
                  <p className="text-[11px] text-gray-500 tabular-nums">.{Math.round(game.homeGoalie.savePctg * 1000)} SV%</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function HotPlayerCard({ player, rank }: { player: HotPlayerSignal; rank: number }) {
  return (
    <div className="flex-shrink-0 w-40 p-3 border-2 border-gray-200 rounded-none bg-white">
      <div className="flex items-center gap-2 mb-2">
        <span className="w-5 h-5 flex items-center justify-center bg-red-100 text-red-700 font-bold text-xs">{rank}</span>
        {player.imageUrl && (
          <img src={player.imageUrl} alt={player.name} className="w-8 h-8 rounded-none bg-gray-200" />
        )}
      </div>
      <p className="font-bold text-sm truncate">{player.name}</p>
      <div className="flex items-center gap-1 text-xs text-gray-500">
        <span>{player.position}</span>
        <span>&bull;</span>
        <img src={getNHLTeamLogoUrl(player.nhlTeam)} alt={player.nhlTeam} className="w-4 h-4" />
        <span>{getNHLTeamShortName(player.nhlTeam)}</span>
      </div>
      <div className="mt-2 grid grid-cols-3 gap-1 text-center">
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">G</div>
          <div className="font-bold text-xs">{player.formGoals}</div>
        </div>
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">A</div>
          <div className="font-bold text-xs">{player.formAssists}</div>
        </div>
        <div className="bg-gray-50 p-1">
          <div className="text-[10px] text-gray-400">PTS</div>
          <div className="font-bold text-xs">{player.formPoints}</div>
        </div>
      </div>
      {(player.topSpeed || player.topShotSpeed) && (
        <div className="mt-1.5 flex gap-1 text-center">
          {player.topSpeed && (
            <div className="flex-1 bg-blue-50 p-1">
              <div className="text-[10px] text-blue-400">SKATE</div>
              <div className="font-bold text-[10px] text-blue-700">{player.topSpeed.toFixed(1)}<span className="text-blue-400 font-normal"> mph</span></div>
            </div>
          )}
          {player.topShotSpeed && (
            <div className="flex-1 bg-blue-50 p-1">
              <div className="text-[10px] text-blue-400">SHOT</div>
              <div className="font-bold text-[10px] text-blue-700">{player.topShotSpeed.toFixed(1)}<span className="text-blue-400 font-normal"> mph</span></div>
            </div>
          )}
        </div>
      )}
      {player.fantasyTeam && (
        <p className="text-[10px] text-[#2563EB] font-bold uppercase mt-2 truncate">{player.fantasyTeam}</p>
      )}
    </div>
  );
}

function ContenderCard({ contender }: { contender: ContenderSignal }) {
  return (
    <div className="p-3 border-2 border-gray-200 rounded-none">
      <div className="flex items-center justify-between mb-1">
        <div className="flex items-center gap-2">
          <img src={getNHLTeamLogoUrl(contender.teamAbbrev)} alt={contender.teamAbbrev} className="w-8 h-8" />
          <span className="font-extrabold text-sm uppercase">{getNHLTeamFullName(contender.teamAbbrev)}</span>
        </div>
        <span className="text-xs text-gray-400">R{contender.round}</span>
      </div>
      <p className="text-xs text-gray-500 mb-2">{contender.seriesTitle}</p>
      <div className="flex items-center gap-1">
        <span className="font-bold text-lg text-green-700">{contender.wins}</span>
        <span className="text-gray-400">-</span>
        <span className="font-bold text-lg text-gray-500">{contender.opponentWins}</span>
        <span className="text-xs text-gray-400 ml-1">vs {getNHLTeamFullName(contender.opponentAbbrev)}</span>
      </div>
    </div>
  );
}

function SleeperCard({ sleeper }: { sleeper: SleeperAlertSignal }) {
  return (
    <div className="flex items-center gap-3 p-3 border-2 border-gray-200 rounded-none">
      <img src={getNHLTeamLogoUrl(sleeper.nhlTeam)} alt={sleeper.nhlTeam} className="w-6 h-6 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        <p className="font-bold text-sm">{sleeper.name}</p>
        <p className="text-xs text-gray-500">{getNHLTeamShortName(sleeper.nhlTeam)}</p>
        {sleeper.fantasyTeam && (
          <p className="text-[10px] text-[#8B5CF6] font-bold uppercase mt-0.5">{sleeper.fantasyTeam}</p>
        )}
      </div>
      <div className="text-right">
        <p className="font-bold text-lg">{sleeper.points}</p>
        <p className="text-[10px] text-gray-400">{sleeper.goals}G {sleeper.assists}A</p>
      </div>
    </div>
  );
}

/** Convert streak codes like "W2" -> "Won 2", "L5" -> "Lost 5", "OT1" -> "OT 1" */
function formatStreak(streak: string | null): string {
  if (!streak) return "";
  if (streak.startsWith("W")) return `Won ${streak.slice(1)}`;
  if (streak.startsWith("L")) return `Lost ${streak.slice(1)}`;
  if (streak.startsWith("OT")) return `OT ${streak.slice(2)}`;
  return streak;
}

export default InsightsPage;
