import { useState } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/contexts/AuthContext";
import { api } from "@/api/client";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import { formatSeason } from "@/utils/format";
import { APP_CONFIG } from "@/config";
import PageHeader from "@/components/common/PageHeader";
import { useLeagues } from "@/features/draft";

const MyLeaguesPage = () => {
  const { user, loading: authLoading } = useAuth();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [newLeagueName, setNewLeagueName] = useState("");
  const [newTeamName, setNewTeamName] = useState("");
  const [creating, setCreating] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Always scope to the signed-in user's leagues — the cross-league
  // "All Leagues" view lives on the admin dashboard now.
  const { leagues, loading: leaguesLoading, createLeague } = useLeagues(user?.id, false);

  const flashError = (msg: string) => {
    setErrorMsg(msg);
    setTimeout(() => setErrorMsg(null), 6000);
  };

  if (authLoading) return <LoadingSpinner message="Checking access..." />;
  if (!user) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="fantasy-card max-w-md w-full">
          <div className="card-header text-center">
            <h2 className="text-xl font-bold">Sign In Required</h2>
          </div>
          <div className="p-8 text-center text-gray-600">
            Please sign in to manage your leagues.
          </div>
        </div>
      </div>
    );
  }

  const handleCreateLeague = async () => {
    if (!newLeagueName.trim()) return;
    if (!newTeamName.trim()) {
      flashError("Please enter your team name");
      return;
    }
    setCreating(true);
    try {
      const league = await createLeague(
        newLeagueName.trim(),
        APP_CONFIG.DEFAULT_SEASON,
        user.id,
      );
      await api.joinLeague(league.id, newTeamName.trim());
      await queryClient.invalidateQueries({ queryKey: ["leagues"] });
      await queryClient.invalidateQueries({ queryKey: ["memberships"] });
      setNewLeagueName("");
      setNewTeamName("");
      navigate(`/league/${league.id}`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      flashError(msg || "Failed to create league");
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title="My Leagues"
        subtitle="Create, join, and manage your leagues."
      />

      {errorMsg && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-[#1A1A1A] text-white px-6 py-3 border-2 border-[#EF4444] text-sm font-bold uppercase tracking-wider shadow-[4px_4px_0px_0px_#EF4444] max-w-lg text-center">
          {errorMsg}
        </div>
      )}

      {/* Create New League */}
      <div className="fantasy-card">
        <div className="card-header">
          <h2 className="text-xl font-bold">Create New League</h2>
        </div>
        <div className="p-6">
          <div className="grid grid-cols-1 sm:grid-cols-[1fr_1fr_auto] gap-3">
            <input
              type="text"
              value={newLeagueName}
              onChange={(e) => setNewLeagueName(e.target.value)}
              placeholder="League name..."
              className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none transition-all"
            />
            <input
              type="text"
              value={newTeamName}
              onChange={(e) => setNewTeamName(e.target.value)}
              placeholder="Your team name..."
              className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none transition-all"
              onKeyDown={(e) => e.key === "Enter" && handleCreateLeague()}
            />
            <button
              onClick={handleCreateLeague}
              disabled={creating || !newLeagueName.trim() || !newTeamName.trim()}
              className="btn-gradient disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap w-full sm:w-auto"
            >
              {creating ? "Creating..." : "Create & Join"}
            </button>
          </div>
        </div>
      </div>

      {/* Your Leagues */}
      <div className="fantasy-card">
        <div className="card-header">
          <h2 className="text-xl font-bold">Your Leagues</h2>
        </div>
        <div className="p-6">
          {leaguesLoading ? (
            <LoadingSpinner size="small" message="Loading leagues..." />
          ) : leagues.length === 0 ? (
            <p className="text-gray-500 text-sm">No leagues yet. Create one above.</p>
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {leagues.map((league) => (
                <Link
                  key={league.id}
                  to={`/league/${league.id}/settings`}
                  className="group block p-5 rounded-none border-2 border-[#1A1A1A] bg-white shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
                >
                  <p className="font-extrabold text-[#1A1A1A] uppercase tracking-wider group-hover:text-[#2563EB] transition-colors">
                    {league.name}
                  </p>
                  <p className="text-xs text-gray-500 mt-1">{formatSeason(league.season)}</p>
                  <div className="mt-3 flex items-center text-xs font-bold uppercase text-gray-400 group-hover:text-[#2563EB] transition-colors">
                    Manage
                    <svg
                      className="w-3 h-3 ml-1 transform group-hover:translate-x-1 transition-transform"
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
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default MyLeaguesPage;
