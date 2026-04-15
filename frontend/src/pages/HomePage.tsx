import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import ActionButtons from "@/components/home/ActionButtons";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import RankingTable from "@/components/common/RankingTable";
import { useHomePageData } from "@/hooks/useHomePageData";
import { useSleepersRankingsColumns } from "@/components/rankingsPageTableColumns/sleepersColumns";
import { useSeasonRankingsColumns } from "@/components/rankingsPageTableColumns/seasonColumns";
import { useDailyRankingsColumns } from "@/components/rankingsPageTableColumns/dailysColumns";
import { useAuth } from "@/contexts/AuthContext";
import { useLeague } from "@/contexts/LeagueContext";
import { api } from "@/api/client";
import { formatSeason } from "@/utils/format";
import PageHeader from "@/components/common/PageHeader";
import type { League } from "@/types/league";

// ── League Members List (for pre-draft state) ─────────────────────────────

interface MemberRow {
  id: string;
  draftOrder: number;
  displayName: string;
  teamName: string;
}

function LeagueMembersList({ leagueId }: { leagueId: string }) {
  const [members, setMembers] = useState<MemberRow[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = await api.getLeagueMembers(leagueId) as MemberRow[];
        if (!cancelled) {
          setMembers(data ?? []);
        }
      } catch {
        // ignore
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [leagueId]);

  if (loading) return <LoadingSpinner size="small" message="Loading members..." />;
  if (members.length === 0) return <p className="text-gray-500 text-sm">No members yet.</p>;

  return (
    <div className="divide-y divide-gray-100">
      {members.map((m) => (
        <div key={m.id} className="flex items-center justify-between py-3">
          <div>
            <p className="text-sm font-medium text-gray-900">
              {m.displayName ?? "Unknown"}
            </p>
            {m.teamName && (
              <p className="text-xs text-gray-500">{m.teamName}</p>
            )}
          </div>
          {m.draftOrder > 0 && (
            <span className="text-xs text-gray-400">Pick #{m.draftOrder}</span>
          )}
        </div>
      ))}
    </div>
  );
}

// ── Join League Banner (for logged-in non-members) ────────────────────────

function JoinLeagueBanner({ league }: { league: League }) {
  const [teamName, setTeamName] = useState("");
  const [joining, setJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const handleJoin = async () => {
    if (!teamName.trim()) {
      setError("Please enter a fantasy team name.");
      return;
    }
    setJoining(true);
    setError(null);
    try {
      await api.joinLeague(league.id, teamName.trim());
      queryClient.invalidateQueries();
      navigate(0);
    } catch (e: any) {
      setError(e.message || "Failed to join league");
    } finally {
      setJoining(false);
    }
  };

  if (league.visibility !== "public") return null;

  return (
    <div className="bg-[#2563EB]/5 rounded-none border-2 border-[#2563EB] p-5 mb-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-bold text-[#1A1A1A] uppercase tracking-wider">Join {league.name}</h2>
          <p className="text-sm text-gray-500 mt-0.5">This league is open. Create a team to start playing.</p>
        </div>
        {!showForm && (
          <button
            onClick={() => setShowForm(true)}
            className="px-5 py-2 bg-[#2563EB] text-white font-bold uppercase text-sm border-2 border-[#1A1A1A] rounded-none shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100 cursor-pointer"
          >
            Join League
          </button>
        )}
      </div>
      {showForm && (
        <div className="mt-4 space-y-3">
          <input
            type="text"
            value={teamName}
            onChange={(e) => setTeamName(e.target.value)}
            placeholder="Your team name..."
            className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none transition-all bg-white"
            onKeyDown={(e) => e.key === "Enter" && handleJoin()}
            autoFocus
          />
          <div className="flex gap-3">
            <button
              onClick={handleJoin}
              disabled={joining || !teamName.trim()}
              className="flex-1 px-5 py-2 bg-[#2563EB] text-white font-bold uppercase text-sm border-2 border-[#1A1A1A] rounded-none disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
            >
              {joining ? "Joining..." : "Confirm"}
            </button>
            <button
              onClick={() => { setShowForm(false); setTeamName(""); setError(null); }}
              className="px-3 py-2 text-sm text-gray-500 font-bold uppercase hover:text-[#1A1A1A] cursor-pointer"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
      {error && <p className="text-sm text-red-600 mt-2">{error}</p>}
    </div>
  );
}

// ── Main HomePage ──────────────────────────────────────────────────────────

const HomePage = () => {
  const { user, profile } = useAuth();
  const { activeLeagueId, activeLeague, draftSession, myLeagues, loading: leagueLoading } = useLeague();
  const isMember = myLeagues.some((l) => l.id === activeLeagueId);

  const {
    yesterdayDate,
    rankings,
    rankingsLoading,
    yesterdayRankings,
    yesterdayRankingsLoading,
    yesterdayRankingsError,
    sleepersData,
    sleepersLoading,
    sleepersError,
    rankingsError,
  } = useHomePageData(activeLeagueId);

  // Build league-prefixed paths
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";

  const dailyRankingsData = Array.isArray(yesterdayRankings) ? yesterdayRankings : [];

  const hasRankings = !rankingsError && Array.isArray(rankings) && rankings.length > 0;
  const hasDailyRankings = !yesterdayRankingsError && dailyRankingsData.length > 0;
  const hasSleepers = !sleepersError && sleepersData && sleepersData.length > 0;
  const hasAnyData = hasRankings || hasDailyRankings || hasSleepers;
  const isLoading = rankingsLoading || yesterdayRankingsLoading || sleepersLoading;

  // Waiting for league data
  if (leagueLoading) {
    return <LoadingSpinner message="Loading your league..." />;
  }

  // ── Non-member league preview (invite link destination) ────────────────

  if (activeLeague && !isMember) {
    const draftStatus = draftSession?.status;
    const draftLabel = !draftSession ? "No draft yet" :
      draftStatus === "completed" ? "Completed" :
      draftStatus === "active" ? "In Progress" :
      draftStatus === "paused" ? "Paused" :
      draftStatus === "picks_done" ? "Picks Done" :
      "Pending";
    const draftColor = !draftSession ? "bg-gray-100 text-gray-600" :
      draftStatus === "completed" ? "bg-blue-100 text-blue-700" :
      draftStatus === "active" ? "bg-green-100 text-green-700" :
      draftStatus === "paused" ? "bg-yellow-100 text-yellow-700" :
      "bg-gray-100 text-gray-600";

    return (
      <div>
        <PageHeader title={activeLeague.name} badge={formatSeason(activeLeague.season)} />

        {/* Join CTA */}
        {user ? (
          <JoinLeagueBanner league={activeLeague} />
        ) : (
          <div className="bg-[#2563EB]/5 rounded-none border-2 border-[#2563EB] p-5 mb-6">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-base font-bold text-[#1A1A1A] uppercase tracking-wider">Join {activeLeague.name}</h2>
                <p className="text-sm text-gray-500 mt-0.5">Sign in to create a team and start playing.</p>
              </div>
              <Link
                to={`/login?returnTo=/league/${activeLeague.id}`}
                className="px-5 py-2 bg-[#2563EB] text-white font-bold uppercase text-sm border-2 border-[#1A1A1A] rounded-none shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
              >
                Sign In to Join
              </Link>
            </div>
          </div>
        )}

        {/* League Info */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Members */}
          <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6">
            <h3 className="text-sm font-bold text-gray-500 uppercase tracking-wider mb-4">Members</h3>
            <LeagueMembersList leagueId={activeLeague.id} />
          </div>

          {/* Draft Status */}
          <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6">
            <h3 className="text-sm font-bold text-gray-500 uppercase tracking-wider mb-4">Draft Status</h3>
            <div className="space-y-3">
              <span className={`inline-block text-xs font-bold uppercase px-3 py-1 border-2 border-[#1A1A1A] rounded-none ${draftColor}`}>
                {draftLabel}
              </span>
              {draftSession && (
                <div className="space-y-2 text-sm text-gray-600">
                  <p>Rounds: <span className="font-bold text-[#1A1A1A]">{draftSession.currentRound} / {draftSession.totalRounds}</span></p>
                  <p>Type: <span className="font-bold text-[#1A1A1A]">{draftSession.snakeDraft ? "Snake" : "Linear"}</span></p>
                </div>
              )}
              {!draftSession && (
                <p className="text-sm text-gray-500">The league admin hasn't set up the draft yet.</p>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  }

  // ── Pre-draft states (logged-in members only) ──────────────────────────

  if (user && activeLeague && isMember) {
    const draftStatus = draftSession?.status;

    // Draft not started (no session or 'pending') — but only if there's no existing data
    if ((!draftSession || draftStatus === "pending") && !hasAnyData && !isLoading) {
      return (
        <div>
          <PageHeader title={activeLeague.name} badge={formatSeason(activeLeague.season)} />

          <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6 mb-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-none bg-[#FFB81C]/20 flex items-center justify-center">
                <svg className="w-5 h-5 text-[#FFB81C]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div>
                <h2 className="text-lg font-bold text-gray-900">Draft Hasn&apos;t Started Yet</h2>
                <p className="text-sm text-gray-500">
                  {draftSession
                    ? "The draft session is set up and waiting to begin."
                    : "No draft session has been created yet."}
                </p>
              </div>
            </div>
            {profile?.isAdmin && (
              <Link
                to="/admin"
                className="inline-flex items-center gap-2 text-sm font-medium text-[#2563EB] hover:text-[#1E40AF] transition-colors"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                Go to Admin to set up the draft
              </Link>
            )}
          </div>

          <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6">
            <h3 className="text-lg font-bold text-gray-900 mb-4">League Members</h3>
            <LeagueMembersList leagueId={activeLeague.id} />
          </div>
        </div>
      );
    }

    // Draft is active (or paused)
    if (draftStatus === "active" || draftStatus === "paused") {
      return (
        <div>
          <PageHeader title={activeLeague.name} badge={formatSeason(activeLeague.season)} />

          <div className={`rounded-none border-2 p-6 mb-6 ${draftStatus === "active" ? "bg-green-50 border-green-200" : "bg-yellow-50 border-yellow-200"}`}>
            <div className="flex items-center gap-3 mb-3">
              <div className={`w-10 h-10 rounded-none flex items-center justify-center ${draftStatus === "active" ? "bg-green-200" : "bg-yellow-200"}`}>
                <svg className={`w-5 h-5 ${draftStatus === "active" ? "text-green-700" : "text-yellow-700"}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
              <div>
                <h2 className="text-lg font-bold text-gray-900">
                  {draftStatus === "active" ? "Draft in Progress!" : "Draft Paused"}
                </h2>
                <p className="text-sm text-gray-600">
                  Round {draftSession.currentRound} of {draftSession.totalRounds}
                </p>
              </div>
            </div>
            <Link
              to={`${lp}/draft`}
              className="inline-flex items-center gap-2 px-5 py-2.5 bg-[#2563EB] text-white rounded-none font-medium transition-all text-sm border-2 border-[#1A1A1A]"
            >
              Go to Draft
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
              </svg>
            </Link>
          </div>

          <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6">
            <h3 className="text-lg font-bold text-gray-900 mb-4">League Members</h3>
            <LeagueMembersList leagueId={activeLeague.id} />
          </div>
        </div>
      );
    }
  }

  // ── Rankings Dashboard (draft completed or public view) ──────────────────

  if (!isLoading && !hasAnyData) {
    return (
      <div>
        {activeLeague && (
          <PageHeader title={activeLeague.name} badge={formatSeason(activeLeague.season)} />
        )}
        <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6 text-center">
          <p className="text-gray-500">
            Rankings and scores will appear once the season starts.
          </p>
        </div>

        <div className="mt-8">
          <ActionButtons />
        </div>
      </div>
    );
  }

  return (
    <div>
      <RankingsDashboard
        rankings={rankings}
        rankingsLoading={rankingsLoading}
        hasRankings={hasRankings}
        dailyRankingsData={dailyRankingsData}
        yesterdayRankingsLoading={yesterdayRankingsLoading}
        hasDailyRankings={hasDailyRankings}
        yesterdayDate={yesterdayDate}
        sleepersData={sleepersData}
        sleepersLoading={sleepersLoading}
        hasSleepers={hasSleepers}
        leaguePrefix={lp}
      />
    </div>
  );
};

// ── Rankings Dashboard (extracted for reuse) ──────────────────────────────

interface RankingsDashboardProps {
  rankings: unknown;
  rankingsLoading: boolean;
  hasRankings: boolean;
  dailyRankingsData: unknown[];
  yesterdayRankingsLoading: boolean;
  hasDailyRankings: boolean;
  yesterdayDate: string;
  sleepersData: unknown[];
  sleepersLoading: boolean;
  hasSleepers: boolean;
  leaguePrefix: string;
}

function RankingsDashboard({
  rankings,
  rankingsLoading,
  hasRankings,
  dailyRankingsData,
  yesterdayRankingsLoading,
  hasDailyRankings,
  yesterdayDate,
  sleepersData,
  sleepersLoading,
  hasSleepers,
  leaguePrefix,
}: RankingsDashboardProps) {
  const seasonRankingsColumns = useSeasonRankingsColumns();
  const dailyRankingsColumns = useDailyRankingsColumns();
  const sleepersRankingsColumns = useSleepersRankingsColumns();

  return (
    <div>
      {/* Overall Rankings */}
      {(rankingsLoading || hasRankings) && (
        <div className="mb-6">
          {rankingsLoading ? (
            <LoadingSpinner message="Loading overall rankings..." />
          ) : (
            <RankingTable
              columns={seasonRankingsColumns}
              data={Array.isArray(rankings) ? rankings : []}
              keyField="teamId"
              rankField="rank"
              title="Overall Rankings"
              limit={7}
              dateBadge="2025/2026 Playoffs"
              viewAllLink={`${leaguePrefix}/rankings`}
              initialSortKey="totalPoints"
              initialSortDirection="desc"
            />
          )}
        </div>
      )}

      {/* Yesterday's Rankings Section */}
      {(yesterdayRankingsLoading || hasDailyRankings) && (
        <div className="mb-6">
          {yesterdayRankingsLoading ? (
            <LoadingSpinner message="Loading yesterday's rankings..." />
          ) : (
            <RankingTable
              columns={dailyRankingsColumns}
              data={dailyRankingsData}
              keyField="teamId"
              rankField="rank"
              title="Yesterday's Rankings"
              limit={7}
              dateBadge={yesterdayDate}
              initialSortKey="dailyPoints"
              initialSortDirection="desc"
              emptyMessage="No rankings data available for yesterday."
            />
          )}
        </div>
      )}

      {(sleepersLoading || hasSleepers) && (
        <div className="mt-8">
          <RankingTable
            columns={sleepersRankingsColumns}
            data={sleepersData}
            keyField="id"
            rankField="rank"
            title="Sleepers"
            dateBadge="2025/2026 Playoffs"
            isLoading={sleepersLoading}
            emptyMessage="No sleeper players available"
            initialSortKey="totalPoints"
            initialSortDirection="desc"
            showRankColors={false}
          />
        </div>
      )}

      {/* Action buttons */}
      <div className="mt-8">
        <ActionButtons />
      </div>
    </div>
  );
}

export default HomePage;
