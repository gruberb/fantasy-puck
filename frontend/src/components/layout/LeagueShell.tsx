import { useEffect } from "react";
import { useParams, Outlet } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import { ErrorMessage, LoadingSpinner } from "@gruberb/fun-ui";

const LeagueShell = () => {
  const { leagueId } = useParams<{ leagueId: string }>();
  const { setActiveLeagueId, activeLeague, allLeagues, leaguesLoading } = useLeague();

  useEffect(() => {
    if (leagueId) {
      setActiveLeagueId(leagueId);
    }
  }, [leagueId, setActiveLeagueId]);

  if (leaguesLoading) {
    return <LoadingSpinner message="Loading league..." />;
  }

  if (!leagueId) {
    return <ErrorMessage message="No league specified." />;
  }

  // If leagues loaded but this ID isn't in the list
  if (allLeagues.length > 0 && !activeLeague) {
    return <ErrorMessage message="League not found." />;
  }

  return <Outlet />;
};

export default LeagueShell;
