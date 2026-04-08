import { Link } from "react-router-dom";
import { NHLTeam } from "@/types/fantasyTeams";
import { useLeague } from "@/contexts/LeagueContext";
import { getTeamGradient } from "@/utils/teamStyles";

interface FantasyTeamCardProps {
  team: NHLTeam;
}

export default function FantasyTeamCard({ team }: FantasyTeamCardProps) {
  const { activeLeagueId } = useLeague();
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const { gradient, textColor } = getTeamGradient(team.name);

  return (
    <Link
      to={`${lp}/teams/${team.id}`}
      className="block overflow-hidden rounded-none transition-all duration-300 group hover:translate-y-[-2px] border-2 border-[#1A1A1A]"
    >
      <div className="relative">
        {/* Card header with team color or gradient */}
        <div
          className="h-24 w-full rounded-none flex items-center justify-center relative overflow-hidden"
          style={{ background: gradient }}
        >
          {/* Animated hover effect */}
          <div className="absolute top-0 left-0 right-0 bottom-0 bg-white opacity-0 group-hover:opacity-10 transition-opacity duration-300"></div>

          {/* Team logo */}
          {team.teamLogo ? (
            <img
              src={team.teamLogo}
              alt={`${team.name} logo`}
              className="h-16 w-16 object-contain z-10 transform group-hover:scale-110 transition-transform duration-300"
            />
          ) : (
            <div
              className="text-3xl font-bold z-10 transform group-hover:scale-110 transition-transform duration-300"
              style={{ color: textColor }}
            >
              {team.name.substring(0, 2).toUpperCase()}
            </div>
          )}
        </div>

        {/* Card content */}
        <div className="bg-white p-5 border-t border-b border-r border-l border-gray-200 rounded-none">
          <h2 className="text-xl font-bold text-gray-800 mb-1 group-hover:text-[#2563EB] transition-colors">
            {team.name}
          </h2>

          <div className="flex items-center justify-between mt-2">
            <span className="text-[#2563EB] text-sm font-medium">
              Fantasy Team
            </span>

            <span className="text-xs inline-flex items-center text-gray-500 group-hover:text-[#2563EB] transition-colors">
              <svg
                className="w-3 h-3 mr-1 group-hover:translate-x-[-2px] transition-transform"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                />
              </svg>
              View Details
              <svg
                className="w-3 h-3 ml-1 transition-transform group-hover:translate-x-1"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M9 5l7 7-7 7"
                />
              </svg>
            </span>
          </div>
        </div>
      </div>
    </Link>
  );
}
