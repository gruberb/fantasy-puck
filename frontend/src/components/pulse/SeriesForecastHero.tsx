import type {
  FantasyTeamForecast,
  PlayerForecastCell,
  SeriesStateCode,
} from "@/features/pulse";

interface SeriesForecastHeroProps {
  forecasts: FantasyTeamForecast[];
  myTeamId: number | null;
}

const STATE_STYLES: Record<SeriesStateCode, string> = {
  eliminated: "bg-[#7F1D1D] text-white",
  facingElim: "bg-[#DC2626] text-white",
  trailing: "bg-[#FB923C] text-[#1A1A1A]",
  tied: "bg-[#E5E7EB] text-[#1A1A1A]",
  leading: "bg-[#86EFAC] text-[#1A1A1A]",
  aboutToAdvance: "bg-[#16A34A] text-white",
  advanced: "bg-[#14532D] text-white",
};

const STATE_LEGEND: { code: SeriesStateCode; label: string }[] = [
  { code: "eliminated", label: "Out" },
  { code: "facingElim", label: "Facing elim" },
  { code: "trailing", label: "Trailing" },
  { code: "tied", label: "Tied" },
  { code: "leading", label: "Leading" },
  { code: "aboutToAdvance", label: "Closing in" },
  { code: "advanced", label: "Advanced" },
];

export default function SeriesForecastHero({
  forecasts,
  myTeamId,
}: SeriesForecastHeroProps) {
  // Put my team first for scanability.
  const sorted = [...forecasts].sort((a, b) => {
    if (a.teamId === myTeamId) return -1;
    if (b.teamId === myTeamId) return 1;
    return a.teamName.localeCompare(b.teamName);
  });

  if (sorted.length === 0) {
    return (
      <section className="bg-white border-2 border-[#1A1A1A] p-6">
        <p className="text-gray-500 text-sm uppercase tracking-wider">
          Series forecast will appear once the bracket is set.
        </p>
      </section>
    );
  }

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[#1A1A1A] text-white px-6 py-3 flex items-center justify-between">
        <h2 className="font-extrabold uppercase tracking-wider text-sm">
          Series Forecast
        </h2>
        <div className="hidden md:flex items-center gap-2 text-[10px] uppercase tracking-wider">
          {STATE_LEGEND.map((s) => (
            <span key={s.code} className="flex items-center gap-1">
              <span
                className={`inline-block w-2.5 h-2.5 ${STATE_STYLES[s.code]}`}
              />
              <span>{s.label}</span>
            </span>
          ))}
        </div>
      </header>

      <div className="divide-y-2 divide-[#1A1A1A]">
        {sorted.map((team) => (
          <TeamForecastRow
            key={team.teamId}
            team={team}
            isMine={team.teamId === myTeamId}
          />
        ))}
      </div>
    </section>
  );
}

function TeamForecastRow({
  team,
  isMine,
}: {
  team: FantasyTeamForecast;
  isMine: boolean;
}) {
  const headlineParts: string[] = [];
  if (team.playersFacingElimination > 0)
    headlineParts.push(`${team.playersFacingElimination} facing elim`);
  if (team.playersEliminated > 0)
    headlineParts.push(`${team.playersEliminated} out`);
  if (team.playersTrailing > 0)
    headlineParts.push(`${team.playersTrailing} trailing`);
  if (team.playersLeading > 0)
    headlineParts.push(`${team.playersLeading} leading`);
  if (team.playersAdvanced > 0)
    headlineParts.push(`${team.playersAdvanced} advanced`);

  const hasRisk =
    team.playersFacingElimination > 0 || team.playersEliminated > 0;

  return (
    <div
      className={`p-4 md:p-5 ${
        isMine ? "bg-[#FACC15]/10 border-l-4 border-[#FACC15]" : ""
      }`}
    >
      <div className="flex items-start justify-between gap-4 mb-3">
        <div>
          <h3 className="text-base md:text-lg font-extrabold uppercase tracking-wider">
            {team.teamName}
            {isMine && (
              <span className="ml-2 text-[10px] bg-[#FACC15] text-[#1A1A1A] px-1.5 py-0.5 tracking-widest">
                YOU
              </span>
            )}
          </h3>
          <p
            className={`text-xs mt-1 ${
              hasRisk ? "text-[#DC2626] font-bold" : "text-gray-500"
            }`}
          >
            {team.totalPlayers} player{team.totalPlayers !== 1 ? "s" : ""}
            {headlineParts.length > 0 && ` — ${headlineParts.join(", ")}`}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-2">
        {team.cells.map((cell, i) => (
          <PlayerCell key={`${team.teamId}-${i}`} cell={cell} />
        ))}
      </div>
    </div>
  );
}

function PlayerCell({ cell }: { cell: PlayerForecastCell }) {
  const pct = Math.round(cell.oddsToAdvance * 100);
  return (
    <div className="border border-gray-300">
      <div
        className={`px-2 py-1 text-[10px] uppercase tracking-wider font-bold ${
          STATE_STYLES[cell.seriesState]
        }`}
      >
        {cell.seriesLabel}
      </div>
      <div className="p-2 flex items-start gap-2">
        <img
          src={cell.headshotUrl}
          alt=""
          className="w-8 h-8 bg-gray-100 flex-shrink-0"
          onError={(e) => {
            (e.target as HTMLImageElement).style.display = "none";
          }}
        />
        <div className="min-w-0 flex-1">
          <p className="text-xs font-bold truncate">{cell.playerName}</p>
          <p className="text-[10px] text-gray-500 mt-0.5">
            {cell.nhlTeam}
            {cell.opponentAbbrev ? ` vs ${cell.opponentAbbrev}` : ""} · {cell.position}
          </p>
          <p className="text-[10px] text-gray-500 mt-0.5 tabular-nums">
            {pct}% adv · {cell.gamesRemaining} left
          </p>
        </div>
      </div>
    </div>
  );
}
