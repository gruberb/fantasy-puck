import { useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

const JoinLeaguePage = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  useEffect(() => {
    const leagueId = searchParams.get("league");
    if (leagueId) {
      navigate(`/league/${leagueId}`, { replace: true });
    } else {
      navigate("/", { replace: true });
    }
  }, [navigate, searchParams]);

  return null;
};

export default JoinLeaguePage;
