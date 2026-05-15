import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import ActionButtons from "@/components/home/ActionButtons";
import { LiveRankingsTable } from "@/components/home/LiveRankingsTable";
import { LoadingSpinner, PageHeader } from "@gruberb/fun-ui";
import RankingTable from "@/components/common/RankingTable";
import { useHomePageData } from "@/hooks/useHomePageData";
import { useSleepersRankingsColumns } from "@/components/rankingsPageTableColumns/sleepersColumns";
import { useSeasonRankingsColumns } from "@/components/rankingsPageTableColumns/seasonColumns";
import { useDailyRankingsColumns } from "@/components/rankingsPageTableColumns/dailysColumns";
import { useAuth } from "@/contexts/AuthContext";
import { useLeague } from "@/contexts/LeagueContext";
import { api } from "@/api/client";
import { formatSeason } from "@/utils/format";

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
  const isPublicLeagueView = Boolean(activeLeague && !isMember);

  // Waiting for league data
  if (leagueLoading) {
    return <LoadingSpinner message="Loading your league..." />;
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

        {!isPublicLeagueView && (
          <div className="mt-8">
            <ActionButtons />
          </div>
        )}
      </div>
    );
  }

  return (
    <div>
      {isPublicLeagueView && activeLeague && (
        <PageHeader title={activeLeague.name} badge={formatSeason(activeLeague.season)} />
      )}
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
        showActionButtons={!isPublicLeagueView}
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
  showActionButtons?: boolean;
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
  showActionButtons = true,
}: RankingsDashboardProps) {
  const seasonRankingsColumns = useSeasonRankingsColumns();
  const dailyRankingsColumns = useDailyRankingsColumns();
  const sleepersRankingsColumns = useSleepersRankingsColumns();

  return (
    <div>
      {/* Live Rankings — appears only while games are in flight. Hidden
          entirely on off-days, so the Overall Rankings naturally moves
          back to the top of the dashboard. */}
      <LiveRankingsTable />

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
            isLoading={sleepersLoading}
            emptyMessage="No sleeper players available"
            initialSortKey="totalPoints"
            initialSortDirection="desc"
            showRankColors={false}
          />
        </div>
      )}

      {showActionButtons && (
        <div className="mt-8">
          <ActionButtons />
        </div>
      )}
    </div>
  );
}

export default HomePage;
