import { useEffect, useState, useRef } from "react";
import { useNavigate, Link, useLocation } from "react-router-dom";
import { useLeague } from "@/contexts/LeagueContext";
import PageHeader from "@/components/common/PageHeader";
import { useAuth } from "@/contexts/AuthContext";
import { api } from "@/api/client";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import { formatSeason } from "@/utils/format";

const STORAGE_KEY = "lastViewedLeagueId";

const LeaguePickerPage = () => {
  const navigate = useNavigate();
  const { user } = useAuth();
  const { allLeagues, leaguesLoading, myLeagues, setActiveLeagueId } =
    useLeague();

  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newLeagueName, setNewLeagueName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  const handleCreateLeague = async () => {
    if (!newLeagueName.trim() || !user) return;
    setCreating(true);
    setCreateError(null);
    try {
      await api.createLeague(newLeagueName.trim(), "20252026");
      setNewLeagueName("");
      setShowCreateForm(false);
      navigate("/admin");
    } catch (e: any) {
      setCreateError(e.message || "Failed to create league");
    } finally {
      setCreating(false);
    }
  };

  // Only auto-redirect on first mount (not when user explicitly navigates to /)
  const hasAutoRedirected = useRef(false);
  const location = useLocation();

  useEffect(() => {
    if (leaguesLoading || hasAutoRedirected.current) return;

    // Only auto-redirect if this is a fresh page load (not explicit navigation)
    // Check if the user came from another page in the app
    const cameFromApp = location.key !== "default";
    if (cameFromApp) {
      setActiveLeagueId(null); // Clear league when explicitly navigating to picker
      return;
    }

    if (user && myLeagues.length > 0) {
      const lastViewed = localStorage.getItem(STORAGE_KEY);
      const match = lastViewed
        ? myLeagues.find((l) => l.id === lastViewed)
        : null;
      if (match) {
        hasAutoRedirected.current = true;
        navigate(`/league/${match.id}`, { replace: true });
      }
    }
  }, [
    user,
    myLeagues,
    allLeagues,
    leaguesLoading,
    navigate,
    location.key,
    setActiveLeagueId,
  ]);

  if (leaguesLoading) {
    return <LoadingSpinner message="Loading leagues..." />;
  }

  if (allLeagues.length === 0) {
    return (
      <div className="max-w-2xl mx-auto text-center py-16">
        <div className="bg-white border-2 border-[#1A1A1A] rounded-none p-12">
          <h1 className="text-3xl font-extrabold uppercase tracking-wider text-[#1A1A1A] mb-4">
            Fantasy NHL 2026
          </h1>
          <p className="text-gray-500 text-lg mb-8">
            No leagues available yet.{" "}
            {user
              ? "Create one to get started!"
              : "Check back soon or sign in to create one."}
          </p>
          {user ? (
            <Link
              to="/admin"
              className="inline-block bg-[#FACC15] text-[#1A1A1A] font-bold uppercase border-2 border-[#1A1A1A] rounded-none px-6 py-3 shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
            >
              Create League
            </Link>
          ) : (
            <Link
              to="/login"
              className="inline-block bg-[#FACC15] text-[#1A1A1A] font-bold uppercase border-2 border-[#1A1A1A] rounded-none px-6 py-3 shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
            >
              Sign In
            </Link>
          )}
        </div>
      </div>
    );
  }

  return (
    <div>
      <PageHeader title="Choose a League" subtitle="Select a league to view standings, stats, and match day action." />

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
        {allLeagues.map((league) => {
          const isMember = myLeagues.some((ml) => ml.id === league.id);
          return (
            <Link
              key={league.id}
              to={`/league/${league.id}`}
              className="group block bg-white border-2 border-[#1A1A1A] rounded-none p-6 shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
            >
              <div className="flex items-start justify-between mb-4">
                <h2 className="text-xl font-extrabold uppercase tracking-wider text-[#1A1A1A] group-hover:text-[#2563EB] transition-colors">
                  {league.name}
                </h2>
                <span
                  className={`text-xs font-bold uppercase px-2 py-1 border-2 border-[#1A1A1A] rounded-none ${
                    league.visibility === "public"
                      ? "bg-[#FACC15] text-[#1A1A1A]"
                      : "bg-gray-200 text-gray-600"
                  }`}
                >
                  {league.visibility}
                </span>
              </div>
              <p className="text-sm text-gray-500 uppercase tracking-wide mb-4">
                {formatSeason(league.season)}
              </p>
              {isMember ? (
                <span className="inline-block text-xs font-bold uppercase text-[#2563EB] border-2 border-[#2563EB] rounded-none px-2 py-1">
                  Your League
                </span>
              ) : user && league.visibility === "public" ? (
                <span className="inline-block text-xs font-bold uppercase text-[#16A34A] border-2 border-[#16A34A] rounded-none px-2 py-1">
                  Open to Join
                </span>
              ) : null}
              <div className="mt-4 flex items-center text-sm font-bold uppercase text-[#1A1A1A] group-hover:text-[#2563EB] transition-colors">
                {isMember ? "Enter League" : "View League"}
                <svg
                  className="w-4 h-4 ml-2 transform group-hover:translate-x-1 transition-transform"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M14 5l7 7m0 0l-7 7m7-7H3"
                  />
                </svg>
              </div>
            </Link>
          );
        })}

        {/* Create League Card - visible to all logged-in users */}
        {user && (
          <div className="bg-white border-2 border-dashed border-[#1A1A1A] rounded-none p-6">
            {showCreateForm ? (
              <div>
                <h2 className="text-xl font-extrabold uppercase tracking-wider text-[#1A1A1A] mb-4">
                  New League
                </h2>
                <input
                  type="text"
                  value={newLeagueName}
                  onChange={(e) => setNewLeagueName(e.target.value)}
                  placeholder="League name..."
                  className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none transition-all mb-3"
                  onKeyDown={(e) => e.key === "Enter" && handleCreateLeague()}
                  autoFocus
                />
                {createError && (
                  <p className="text-sm text-red-600 mb-3">{createError}</p>
                )}
                <div className="flex gap-2">
                  <button
                    onClick={handleCreateLeague}
                    disabled={creating || !newLeagueName.trim()}
                    className="btn-gradient disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                  >
                    {creating ? "Creating..." : "Create"}
                  </button>
                  <button
                    onClick={() => {
                      setShowCreateForm(false);
                      setNewLeagueName("");
                      setCreateError(null);
                    }}
                    className="text-sm text-gray-500 uppercase font-bold hover:text-[#1A1A1A] transition-colors"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <button
                onClick={() => setShowCreateForm(true)}
                className="w-full h-full flex flex-col items-center justify-center text-center py-4 cursor-pointer group"
              >
                <svg
                  className="w-10 h-10 text-gray-400 group-hover:text-[#2563EB] transition-colors mb-3"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 4v16m8-8H4"
                  />
                </svg>
                <span className="text-sm font-bold uppercase tracking-wider text-gray-500 group-hover:text-[#2563EB] transition-colors">
                  Create League
                </span>
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default LeaguePickerPage;
