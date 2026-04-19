import { useMemo } from "react";
import { Link } from "react-router-dom";
import { ErrorMessage, LoadingSpinner } from "@gruberb/fun-ui";
import { useTeams } from "@/features/teams";
import { useLeagueMembers } from "@/features/draft";
import { useLeague } from "@/contexts/LeagueContext";
import { getTeamGradient } from "@/utils/teamStyles";

const FantasyTeamsPage = () => {
  const { activeLeagueId } = useLeague();
  const { teams, isLoading, error } = useTeams(activeLeagueId);
  const { members } = useLeagueMembers(activeLeagueId);

  // Map team ID -> owner display name
  const ownerMap = useMemo(() => {
    const map = new Map<number, string>();
    for (const m of members) {
      if (m.fantasyTeamId && m.displayName) {
        map.set(m.fantasyTeamId, m.displayName);
      }
    }
    return map;
  }, [members]);

  if (isLoading) {
    return <LoadingSpinner size="large" message="Loading teams..." />;
  }

  if (error) {
    return <ErrorMessage message="Failed to load teams. Please try again." />;
  }

  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
      {teams.map((team) => {
        const { gradient } = getTeamGradient(team.name);
        const owner = ownerMap.get(team.id);

        return (
          <Link
            key={team.id}
            to={`${lp}/teams/${team.id}`}
            className="group block bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden hover:translate-y-[-2px] hover:shadow-[6px_6px_0px_0px_#1A1A1A] transition-all duration-200"
          >
            {/* Gradient accent bar */}
            <div className="h-3" style={{ background: gradient }} />

            <div className="p-5">
              <h2 className="text-xl font-extrabold text-[#1A1A1A] uppercase tracking-wider group-hover:text-[#2563EB] transition-colors">
                {team.name}
              </h2>
              {owner && (
                <p className="text-sm text-gray-500 mt-1">{owner}</p>
              )}

              <div className="flex items-center justify-between mt-4 pt-3 border-t border-gray-100">
                <span className="text-xs text-gray-400 uppercase tracking-wider">View Details</span>
                <svg className="w-4 h-4 text-gray-400 group-hover:text-[#2563EB] group-hover:translate-x-1 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              </div>
            </div>
          </Link>
        );
      })}
    </div>
  );
};

export default FantasyTeamsPage;
