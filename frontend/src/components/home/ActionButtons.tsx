import { Link } from "react-router-dom";
import { getFixedAnalysisDateString } from "@/utils/timezone";
import { useLeague } from "@/contexts/LeagueContext";

/**
 * Dashboard quick-links. Four targets:
 *   Pulse / Insights — league-scoped when a league is active, no-op otherwise.
 *   Today's Games   — global, always visible.
 *   Detailed Stats  — league-scoped; hidden when no league is selected.
 */
export default function ActionButtons() {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const buttons: { label: string; to: string; color: string; hide?: boolean }[] = [
    {
      label: "Pulse",
      to: `${lp}/pulse`,
      color: "#FACC15",
      hide: !activeLeagueId,
    },
    {
      label: "Insights",
      to: `${lp}/insights`,
      color: "#2563EB",
      hide: !activeLeagueId,
    },
    {
      label: "Today's Games",
      to: `/games/${getFixedAnalysisDateString()}`,
      color: "#16A34A",
    },
    {
      label: "Detailed Stats",
      to: `${lp}/rankings`,
      color: "#EF4444",
      hide: !activeLeagueId,
    },
  ];

  const visible = buttons.filter((b) => !b.hide);

  return (
    <div
      className={`grid grid-cols-1 sm:grid-cols-2 ${
        visible.length >= 4 ? "lg:grid-cols-4" : "lg:grid-cols-3"
      } gap-4 mb-6`}
    >
      {visible.map((b) => (
        <Link
          key={b.label}
          to={b.to}
          className="py-3 px-4 rounded-none font-bold uppercase tracking-wider text-center border-2 border-[#1A1A1A] shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
          style={{
            backgroundColor: b.color,
            color: b.color === "#FACC15" ? "#1A1A1A" : "#FFFFFF",
          }}
        >
          {b.label}
        </Link>
      ))}
    </div>
  );
}
