import type { MyGoalieSignal, GoalieStartStatus } from "@/features/pulse";

interface Props {
  goalie: MyGoalieSignal;
}

const STATUS_STYLES: Record<GoalieStartStatus, string> = {
  confirmed: "bg-[#16A34A] text-white",
  probable: "bg-[#FACC15] text-[#1A1A1A]",
  backup: "bg-[#E5E7EB] text-[#1A1A1A]",
  unknown: "bg-gray-200 text-gray-600",
};

const STATUS_LABEL: Record<GoalieStartStatus, string> = {
  confirmed: "Confirmed",
  probable: "Probable",
  backup: "Backup",
  unknown: "TBD",
};

export default function MyGoalieCard({ goalie }: Props) {
  const hasGame = goalie.opponentAbbrev.length > 0;
  return (
    <article className="border-2 border-[#1A1A1A] bg-white">
      <header
        className={`px-3 py-1.5 text-[10px] uppercase tracking-wider font-extrabold ${
          STATUS_STYLES[goalie.startStatus]
        }`}
      >
        {STATUS_LABEL[goalie.startStatus]}
      </header>
      <div className="p-3 flex items-start gap-3">
        <img
          src={goalie.headshotUrl}
          alt=""
          className="w-12 h-12 bg-gray-100 flex-shrink-0"
          onError={(e) => {
            (e.target as HTMLImageElement).style.display = "none";
          }}
        />
        <div className="flex-1 min-w-0">
          <h4 className="font-bold text-sm truncate">{goalie.playerName}</h4>
          <div className="flex items-center gap-1 text-xs text-gray-500 mt-0.5">
            <img src={goalie.nhlTeamLogo} alt="" className="w-4 h-4" />
            <span>{goalie.nhlTeam}</span>
          </div>
          {hasGame ? (
            <div className="flex items-center gap-1 text-xs text-gray-500 mt-1">
              <span>vs</span>
              <img src={goalie.opponentLogo} alt="" className="w-4 h-4" />
              <span>{goalie.opponentAbbrev}</span>
              {goalie.gameStartUtc && (
                <span className="text-gray-400 ml-1">
                  · {formatTime(goalie.gameStartUtc)}
                </span>
              )}
            </div>
          ) : (
            <p className="text-xs text-gray-400 mt-1">Not playing tonight</p>
          )}
        </div>
      </div>
    </article>
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
