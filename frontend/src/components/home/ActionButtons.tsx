import { Link } from "react-router-dom";
import { getFixedAnalysisDateString } from "@/utils/timezone";
import { useLeague } from "@/contexts/LeagueContext";

export default function ActionButtons() {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      {activeLeagueId && (
        <Link
          to={`${lp}/teams`}
          className="bg-[#16A34A] text-white py-3 px-4 rounded-none font-bold uppercase tracking-wider text-center border-2 border-[#1A1A1A] shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
        >
          View All Teams
        </Link>
      )}
      <Link
        to={`/games/${getFixedAnalysisDateString()}`}
        className="bg-[#2563EB] text-white py-3 px-4 rounded-none font-bold uppercase tracking-wider text-center border-2 border-[#1A1A1A] shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
      >
        Game Center
      </Link>
      {activeLeagueId && (
        <Link
          to={`${lp}/rankings`}
          className="bg-[#EF4444] text-white py-3 px-4 rounded-none font-bold uppercase tracking-wider text-center border-2 border-[#1A1A1A] shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
        >
          View Full Rankings
        </Link>
      )}
    </div>
  );
}
