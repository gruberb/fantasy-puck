import { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import { api } from "@/api/client";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import PageHeader from "@/components/common/PageHeader";
import { useLeagues } from "@/features/draft";
import { formatSeason } from "@/utils/format";

const JoinLeaguePage = () => {
  const { user, profile, loading: authLoading } = useAuth();
  const { leagues, loading: leaguesLoading } = useLeagues();

  // Track which leagues the user is already a member of
  const [myLeagueIds, setMyLeagueIds] = useState<Set<string>>(new Set());
  const [membershipLoaded, setMembershipLoaded] = useState(false);

  // Join form state
  const [joiningLeagueId, setJoiningLeagueId] = useState<string | null>(null);
  const [teamName, setTeamName] = useState("");
  const [joining, setJoining] = useState(false);

  // Messages
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Fetch user's memberships once we have the user
  useEffect(() => {
    if (!user?.id) return;
    (async () => {
      try {
        const data = await api.getMemberships() as Array<{ leagueId: string }>;
        if (data) {
          setMyLeagueIds(new Set(data.map((d) => d.leagueId)));
        }
      } catch {
        // ignore
      }
      setMembershipLoaded(true);
    })();
  }, [user?.id]);

  // ── Guards ───────────────────────────────────────────────────────────────

  if (authLoading) return <LoadingSpinner message="Loading..." />;
  if (!user) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="fantasy-card max-w-md w-full">
          <div className="card-header text-center">
            <h2 className="text-xl font-bold">Sign In Required</h2>
          </div>
          <div className="p-8 text-center text-gray-600">
            Please sign in to join a league.
          </div>
        </div>
      </div>
    );
  }

  // ── Helpers ──────────────────────────────────────────────────────────────

  const flash = (msg: string) => {
    setStatusMsg(msg);
    setErrorMsg(null);
    setTimeout(() => setStatusMsg(null), 4000);
  };

  const flashError = (msg: string) => {
    setErrorMsg(msg);
    setStatusMsg(null);
    setTimeout(() => setErrorMsg(null), 6000);
  };

  const handleJoin = async (leagueId: string) => {
    if (!teamName.trim()) {
      flashError("Please enter a fantasy team name.");
      return;
    }
    setJoining(true);
    try {
      await api.joinLeague(leagueId, teamName.trim());
      setMyLeagueIds((prev) => new Set(prev).add(leagueId));
      setJoiningLeagueId(null);
      setTeamName("");
      flash("You have joined the league!");
    } catch (e: any) {
      flashError(e.message || "Failed to join league");
    } finally {
      setJoining(false);
    }
  };

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div className="space-y-6 max-w-3xl mx-auto">
      <PageHeader
        title="Join a League"
        subtitle="Browse available leagues and create your fantasy team"
      />

      {/* Status Messages */}
      {statusMsg && (
        <div className="bg-green-50 border border-green-200 text-green-800 px-4 py-3 rounded-none text-sm">
          {statusMsg}
        </div>
      )}
      {errorMsg && <ErrorMessage message={errorMsg} />}

      {/* Leagues List */}
      {leaguesLoading || !membershipLoaded ? (
        <LoadingSpinner message="Loading leagues..." />
      ) : leagues.length === 0 ? (
        <div className="fantasy-card">
          <div className="p-12 text-center text-gray-500">
            <svg
              className="w-12 h-12 mx-auto mb-4 text-gray-300"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={1.5}
                d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
              />
            </svg>
            <p className="text-lg font-medium text-gray-700">No leagues available</p>
            <p className="text-sm mt-1">Check back later or ask an admin to create one.</p>
          </div>
        </div>
      ) : (
        <div className="space-y-4">
          {leagues.map((league) => {
            const alreadyJoined = myLeagueIds.has(league.id);
            const isJoiningThis = joiningLeagueId === league.id;

            return (
              <div key={league.id} className="fantasy-card">
                <div className="p-5">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-lg font-bold text-gray-900">{league.name}</h3>
                      <p className="text-sm text-gray-500 mt-0.5">Season: {formatSeason(league.season)}</p>
                    </div>
                    <div>
                      {alreadyJoined ? (
                        <Link
                          to={`/league/${league.id}/draft`}
                          className="btn-gradient text-sm inline-flex items-center gap-1"
                        >
                          Go to Draft
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
                          </svg>
                        </Link>
                      ) : isJoiningThis ? null : (
                        <button
                          onClick={() => {
                            setJoiningLeagueId(league.id);
                            setTeamName("");
                            setErrorMsg(null);
                          }}
                          className="btn-gradient text-sm"
                        >
                          Join League
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Join Form (expanded) */}
                  {isJoiningThis && !alreadyJoined && (
                    <div className="mt-4 pt-4 border-t border-gray-200">
                      <label className="block text-sm font-medium text-gray-700 mb-2">
                        Choose your fantasy team name
                      </label>
                      <div className="flex gap-3">
                        <input
                          type="text"
                          value={teamName}
                          onChange={(e) => setTeamName(e.target.value)}
                          placeholder="e.g. Icy Legends"
                          className="flex-1 px-4 py-2 border border-gray-300 rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none transition-all"
                          onKeyDown={(e) => e.key === "Enter" && handleJoin(league.id)}
                          autoFocus
                        />
                        <button
                          onClick={() => handleJoin(league.id)}
                          disabled={joining || !teamName.trim()}
                          className="btn-gradient disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                        >
                          {joining ? "Joining..." : "Confirm"}
                        </button>
                        <button
                          onClick={() => {
                            setJoiningLeagueId(null);
                            setTeamName("");
                          }}
                          className="btn-secondary-enhanced"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default JoinLeaguePage;
