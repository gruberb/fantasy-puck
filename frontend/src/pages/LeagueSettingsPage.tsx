import { useState, useEffect, useCallback, useRef } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/contexts/use-auth";
import { useLeague } from "@/contexts/use-league";
import { api } from "@/api/client";
import { Button, LoadingSpinner, PageHeader } from "@gruberb/fun-ui";
import RankingTable from "@/components/common/RankingTable";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { Toast } from "@/components/common/Toast";
import { getLeagueMembersColumns } from "@/components/rankingsPageTableColumns/leagueMembersColumns";
import { useFlash } from "@/hooks/use-flash";
import { formatSeason } from "@/utils/format";
import { APP_CONFIG } from "@/config";
import {
  useLeagueMembers,
  useDraftSession,
  usePlayerPool,
  useAdminDraftActions,
  leagueKeys,
  membershipKeys,
  type LeagueMember,
} from "@/features/draft";

// ── Types ─────────────────────────────────────────────────────────────────

interface FantasyPlayer {
  id: number;
  team_id: number;
  nhl_id: number;
  name: string;
  position: string;
  nhl_team: string;
}

interface TeamPlayers {
  memberId: string;
  memberName: string;
  teamId: number;
  teamName: string;
  players: FantasyPlayer[];
  sleeper: FantasyPlayer | null;
}

interface NhlSkaterLeader {
  id: number;
  firstName: { default: string };
  lastName: { default: string };
  teamAbbrev: string;
  position: string;
  value: number;
}

interface PendingConfirm {
  title: string;
  body: string;
  confirmLabel: string;
  action: () => Promise<void>;
}

// ── Component ─────────────────────────────────────────────────────────────

const LeagueSettingsPage = () => {
  const { leagueId } = useParams<{ leagueId: string }>();
  const { user, profile, loading: authLoading } = useAuth();
  const { activeLeague } = useLeague();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Draft controls
  const [totalRounds, setTotalRounds] = useState(10);
  const [snakeDraft, setSnakeDraft] = useState(true);
  const [startingDraft, setStartingDraft] = useState(false);

  const { flash: toast, showFlash } = useFlash();
  const [pendingConfirm, setPendingConfirm] = useState<PendingConfirm | null>(null);

  // Team name editing
  const [editingTeamId, setEditingTeamId] = useState<number | null>(null);
  const [editingTeamName, setEditingTeamName] = useState("");

  // Player management
  const [teamPlayers, setTeamPlayers] = useState<TeamPlayers[]>([]);
  const [playersLoading, setPlayersLoading] = useState(false);
  const [expandedTeams, setExpandedTeams] = useState<Set<number>>(new Set());
  const [addPlayerTeamId, setAddPlayerTeamId] = useState<number>(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [nhlPlayersCache, setNhlPlayersCache] = useState<NhlSkaterLeader[] | null>(null);
  const [nhlPlayersFetching, setNhlPlayersFetching] = useState(false);
  const [searchFocused, setSearchFocused] = useState(false);
  const [addingPlayer, setAddingPlayer] = useState(false);
  const searchRef = useRef<HTMLDivElement>(null);

  const isSuperAdmin = !!profile?.isAdmin;
  const isOwner = activeLeague?.created_by === user?.id;
  const canManage = isSuperAdmin || isOwner;

  const { members, loading: membersLoading, fetchMembers } = useLeagueMembers(leagueId ?? null);
  const { session, loading: sessionLoading, fetchSession, setSession } = useDraftSession(leagueId ?? null);
  const { players } = usePlayerPool(session?.id ?? null);
  const { createDraftSession, randomizeDraftOrder, startDraft, pauseDraft, resumeDraft } = useAdminDraftActions();

  // ── Helpers ──────────────────────────────────────────────────────────────

  const flashError = (msg: string) => {
    let friendly = msg;
    if (msg.includes("foreign key constraint")) friendly = "Can't delete: there are draft picks or other data linked to this. Delete the draft session first.";
    if (msg.includes("violates unique constraint")) friendly = "This already exists.";
    if (msg.includes("row-level security") || msg.includes("RLS")) friendly = "Permission denied. Make sure you own this league.";
    if (msg.includes("Not enough") || msg.includes("at least")) friendly = msg;
    showFlash(friendly, "error");
  };

  // ── Player fetching ─────────────────────────────────────────────────────

  const fetchTeamPlayersAuto = useCallback(async () => {
    if (!leagueId || members.length === 0) { setTeamPlayers([]); return; }
    const teamIds = members.filter((m) => m.fantasyTeamId).map((m) => m.fantasyTeamId);
    if (teamIds.length === 0) { setTeamPlayers([]); return; }
    setPlayersLoading(true);
    try {
      const rawGroups = await api.getFantasyPlayers(leagueId) as Array<{
        nhlTeam: string;
        players: Array<{
          id: number; nhlId: number; name: string; fantasyTeamId: number;
          fantasyTeamName: string; position: string; nhlTeam: string;
        }>;
      }>;
      const allPlayers: FantasyPlayer[] = (rawGroups ?? []).flatMap((group) =>
        group.players.map((p) => ({
          id: p.id, team_id: p.fantasyTeamId, nhl_id: p.nhlId,
          name: p.name, position: p.position, nhl_team: p.nhlTeam,
        })),
      );
      const playersByTeam = new Map<number, FantasyPlayer[]>();
      for (const p of allPlayers) {
        if (!playersByTeam.has(p.team_id)) playersByTeam.set(p.team_id, []);
        playersByTeam.get(p.team_id)!.push(p);
      }
      const sleepersByTeam = new Map<number, FantasyPlayer>();
      try {
        const sleepersRaw = await api.getSleepers(leagueId) as Array<{
          id: number; nhlId: number; name: string; position: string; nhlTeam: string;
          fantasyTeamId: number | null;
        }>;
        for (const s of sleepersRaw ?? []) {
          if (s.fantasyTeamId) {
            sleepersByTeam.set(s.fantasyTeamId, {
              id: s.id, team_id: s.fantasyTeamId, nhl_id: s.nhlId,
              name: s.name, position: s.position, nhl_team: s.nhlTeam,
            });
          }
        }
      } catch { /* sleepers optional */ }
      const results: TeamPlayers[] = members
        .filter((m) => m.fantasyTeamId)
        .map((m) => ({
          memberId: m.id,
          memberName: m.displayName ?? "Unknown",
          teamId: m.fantasyTeamId,
          teamName: m.teamName ?? "No team",
          players: playersByTeam.get(m.fantasyTeamId) ?? [],
          sleeper: sleepersByTeam.get(m.fantasyTeamId) ?? null,
        }));
      setTeamPlayers(results);
    } catch (e: any) {
      console.error("Failed to auto-fetch team players:", e.message);
    } finally { setPlayersLoading(false); }
  }, [leagueId, members]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { fetchTeamPlayersAuto(); }, [leagueId, members.length]);

  const fetchNhlPlayersCache = useCallback(async () => {
    if (nhlPlayersCache || nhlPlayersFetching) return;
    setNhlPlayersFetching(true);
    try {
      const skaters = await api.getTopSkaters(800, parseInt(APP_CONFIG.DEFAULT_SEASON), APP_CONFIG.DEFAULT_GAME_TYPE, 0);
      const players = skaters.map((p) => ({
        id: p.id, firstName: { default: p.firstName }, lastName: { default: p.lastName },
        sweaterNumber: p.sweaterNumber, teamAbbrev: p.teamAbbrev, position: p.position,
        headshot: p.headshot, value: p.stats?.points ?? 0,
      }));
      setNhlPlayersCache(players);
    } catch (e) {
      console.error("Failed to fetch NHL players:", e);
      setNhlPlayersCache([]);
    } finally { setNhlPlayersFetching(false); }
  }, [nhlPlayersCache, nhlPlayersFetching]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setSearchFocused(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // ── Handlers ─────────────────────────────────────────────────────────────

  const handleRemoveMember = (member: LeagueMember) => {
    const memberName = member.displayName ?? "Unknown";
    setPendingConfirm({
      title: "Remove member?",
      body: `Remove "${memberName}" from this league? This cannot be undone.`,
      confirmLabel: "Remove",
      action: async () => {
        try { await api.removeLeagueMember(leagueId!, member.id); await fetchMembers(); showFlash(`Member "${memberName}" removed.`); } catch (e: any) { flashError(e.message || "Failed to remove member"); }
      },
    });
  };

  const handleStartEditTeamName = (teamId: number, currentName: string) => { setEditingTeamId(teamId); setEditingTeamName(currentName); };
  const handleCancelEditTeamName = () => { setEditingTeamId(null); setEditingTeamName(""); };
  const handleSaveTeamName = async (teamId: number) => {
    if (!editingTeamName.trim()) return;
    try { await api.updateTeamName(teamId, editingTeamName.trim()); setEditingTeamId(null); setEditingTeamName(""); await fetchMembers(); showFlash("Team name updated."); } catch (e: any) { flashError(e.message || "Failed to update team name"); }
  };

  const handleStartFullDraft = async () => {
    if (!leagueId) return;
    setStartingDraft(true);
    try {
      const sess = await createDraftSession(leagueId, totalRounds, snakeDraft);
      await randomizeDraftOrder(leagueId);
      await startDraft(sess.id);
      await fetchSession();
      await fetchMembers();
      showFlash("Draft started!");
    } catch (e: any) {
      flashError(e.message || "Failed to start draft");
    } finally { setStartingDraft(false); }
  };

  const handlePauseResume = async () => {
    if (!session?.id) return;
    try {
      if (session.status === "active") { await pauseDraft(session.id); await fetchSession(); showFlash("Draft paused"); }
      else if (session.status === "paused") { await resumeDraft(session.id); await fetchSession(); showFlash("Draft resumed"); }
    } catch (e: any) { flashError(e.message || "Failed to update draft status"); }
  };

  const handleDeleteDraftSession = () => {
    if (!session?.id || !leagueId) return;
    const sessionId = session.id;
    setPendingConfirm({
      title: "Delete draft session?",
      body: "This will DELETE all draft picks, the player pool, the draft session itself, and all fantasy players created from this draft. This cannot be undone. Are you sure?",
      confirmLabel: "Delete Draft",
      action: async () => {
        try {
          await api.deleteDraftSession(sessionId);
          setSession(null); setTeamPlayers([]); await fetchSession(); showFlash("Draft session and all related data deleted.");
        } catch (e: any) { flashError(e.message || "Failed to delete draft session"); }
      },
    });
  };

  const handleDeleteLeague = () => {
    if (!leagueId || !activeLeague) return;
    setPendingConfirm({
      title: "Delete league?",
      body: `Delete league "${activeLeague.name}"? This will also delete all members, draft sessions, and related data.`,
      confirmLabel: "Delete League",
      action: async () => {
        try {
          await api.deleteLeague(leagueId);
          await queryClient.invalidateQueries({ queryKey: leagueKeys.all });
          await queryClient.invalidateQueries({ queryKey: membershipKeys.all });
          navigate("/");
        } catch (e: any) { flashError(e.message || "Failed to delete league"); }
      },
    });
  };

  const toggleTeamExpanded = (teamId: number) => { setExpandedTeams((prev) => { const next = new Set(prev); if (next.has(teamId)) next.delete(teamId); else next.add(teamId); return next; }); };

  const handleDeletePlayer = (playerId: number, playerName: string) => {
    setPendingConfirm({
      title: "Remove player?",
      body: `Remove "${playerName}" from their team?`,
      confirmLabel: "Remove",
      action: async () => {
        try { await api.removePlayer(playerId); await fetchTeamPlayersAuto(); showFlash(`Player "${playerName}" removed.`); } catch (e: any) { flashError(e.message || "Failed to remove player"); }
      },
    });
  };

  const handleDeleteSleeper = (sleeperId: number, sleeperName: string) => {
    setPendingConfirm({
      title: "Remove sleeper?",
      body: `Remove sleeper "${sleeperName}"?`,
      confirmLabel: "Remove",
      action: async () => {
        try { await api.removeSleeper(sleeperId); await fetchTeamPlayersAuto(); showFlash(`Sleeper "${sleeperName}" removed.`); } catch (e: any) { flashError(e.message || "Failed to remove sleeper"); }
      },
    });
  };

  const handleConfirmPending = () => {
    const pending = pendingConfirm;
    setPendingConfirm(null);
    if (pending) void pending.action();
  };

  const handleAddPlayerFromSearch = async (nhlPlayer: NhlSkaterLeader) => {
    if (!addPlayerTeamId) { flashError("Select a team first."); return; }
    setAddingPlayer(true);
    const playerName = `${nhlPlayer.firstName.default} ${nhlPlayer.lastName.default}`;
    try {
      await api.addPlayerToTeam(addPlayerTeamId, {
        nhlId: nhlPlayer.id, name: playerName, position: nhlPlayer.position, nhlTeam: nhlPlayer.teamAbbrev,
      });
      setSearchQuery(""); setSearchFocused(false);
      await fetchTeamPlayersAuto();
      showFlash(`${playerName} added successfully.`);
    } catch (e: any) { flashError(e.message || "Failed to add player"); } finally { setAddingPlayer(false); }
  };

  // ── Computed ──────────────────────────────────────────────────────────────

  const existingNhlIds = new Set(teamPlayers.flatMap((tp) => tp.players.map((p) => p.nhl_id)));

  const searchResults = (() => {
    if (!nhlPlayersCache || searchQuery.length < 2) return [];
    const q = searchQuery.toLowerCase();
    return nhlPlayersCache
      .filter((p) => {
        if (existingNhlIds.has(p.id)) return false;
        const fullName = `${p.firstName.default} ${p.lastName.default}`.toLowerCase();
        return fullName.includes(q);
      })
      .slice(0, 15);
  })();

  // Total picks calculation (fix for off-by-one and sleeper counts)
  const regularTotal = members.length * (session?.totalRounds ?? 0);
  const regularDone = session && (session.status === "completed" || session.status === "picks_done")
    ? regularTotal
    : (session?.currentPickIndex ?? 0);
  const sleeperTotal = members.length;
  const sleeperDone = session?.sleeperStatus === "completed"
    ? sleeperTotal
    : (session?.sleeperPickIndex ?? 0);

  // ── Guards ──────────────────────────────────────────────────────────────

  if (authLoading) return <LoadingSpinner message="Checking access..." />;
  if (!user) return <div className="text-center py-16 text-gray-500">Please sign in.</div>;
  if (!activeLeague) return <LoadingSpinner message="Loading league..." />;
  if (!canManage) {
    return (
      <div className="text-center py-16">
        <p className="text-gray-500">You don't have permission to manage this league.</p>
        <Link to={`/league/${leagueId}`} className="text-[#2563EB] font-bold text-sm uppercase mt-4 inline-block">Back to League</Link>
      </div>
    );
  }

  // ── Render ──────────────────────────────────────────────────────────────

  return (
    <div className="space-y-8">
      <PageHeader
        title={`${activeLeague.name} Settings`}
        subtitle={formatSeason(activeLeague.season)}
      />

      <Toast flash={toast} />
      <ConfirmDialog
        open={!!pendingConfirm}
        title={pendingConfirm?.title ?? ""}
        body={pendingConfirm?.body}
        confirmLabel={pendingConfirm?.confirmLabel}
        onConfirm={handleConfirmPending}
        onCancel={() => setPendingConfirm(null)}
      />

      {/* ── Members ────────────────────────────────────────────────────────── */}
      <RankingTable<LeagueMember>
        data={members}
        columns={getLeagueMembersColumns({
          editingTeamId,
          editingTeamName,
          onEditingTeamNameChange: setEditingTeamName,
          onStartEdit: handleStartEditTeamName,
          onSaveEdit: handleSaveTeamName,
          onCancelEdit: handleCancelEditTeamName,
          onRemove: handleRemoveMember,
        })}
        rankField="draftOrder"
        initialSortKey="draftOrder"
        initialSortDirection="asc"
        showRankColors={false}
        isLoading={membersLoading}
        emptyMessage="No members yet. Share the invite link above!"
        customHeader={
          <div className="card-header">
            <div className="flex items-center justify-between">
              <h2 className="text-xl font-bold">Members ({members.length})</h2>
              <Button
                size="sm"
                variant="secondary"
                className="bg-[#FACC15]"
                onClick={() => {
                  navigator.clipboard.writeText(`${window.location.origin}/league/${leagueId}`);
                  showFlash("Invite link copied to clipboard!");
                }}
              >
                Copy Invite Link
              </Button>
            </div>
          </div>
        }
      />

      {/* ── Draft Management ───────────────────────────────────────────────── */}
      <div className="fantasy-card">
        <div className="card-header">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-bold">Draft</h2>
            {session && <span className={`text-xs px-3 py-1 rounded-none font-medium ${session.status === "active" ? "bg-green-400/20 text-green-300" : session.status === "paused" ? "bg-yellow-400/20 text-yellow-300" : session.status === "completed" ? "bg-blue-400/20 text-blue-300" : "bg-white/20 text-white/80"}`}>{session.status.toUpperCase()}</span>}
          </div>
        </div>
        <div className="p-6 space-y-6">
          {sessionLoading ? <LoadingSpinner size="small" message="Loading draft..." /> : !session ? (
            <div className="space-y-4">
              <div className="flex flex-wrap gap-4 items-end">
                <div><label className="block text-xs text-gray-500 mb-1">Total Rounds</label><input type="number" value={totalRounds} onChange={(e) => setTotalRounds(Math.max(1, parseInt(e.target.value) || 1))} min={1} max={30} className="w-24 px-3 py-2 border-2 border-[#1A1A1A] rounded-none focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none text-center" /></div>
                <div className="flex items-center gap-2"><input type="checkbox" id="snakeDraft" checked={snakeDraft} onChange={(e) => setSnakeDraft(e.target.checked)} className="w-4 h-4 text-[#2563EB] border-gray-300 rounded-none focus:ring-[#2563EB]" /><label htmlFor="snakeDraft" className="text-sm text-gray-700">Snake Draft</label></div>
                <Button onClick={handleStartFullDraft} disabled={startingDraft || members.length < 2}>
                  {startingDraft ? (
                    <span className="flex items-center gap-2">
                      <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24"><circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" /><path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" /></svg>
                      Setting up draft...
                    </span>
                  ) : "Start Draft"}
                </Button>
              </div>
              {members.length < 2 && (
                <p className="text-sm text-yellow-700">Need at least 2 league members to start ({members.length} currently)</p>
              )}
            </div>
          ) : (
            <>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                <div className="stat-card-enhanced text-center"><p className="stat-label">Round</p><p className="stat-value text-[#2563EB]">{session.currentRound} / {session.totalRounds}</p></div>
                <div className="stat-card-enhanced text-center"><p className="stat-label">Total Picks</p><p className="stat-value text-[#1A1A1A]">{regularDone + sleeperDone}</p></div>
                <div className="stat-card-enhanced text-center"><p className="stat-label">Players in Pool</p><p className="stat-value text-[#2563EB]">{players.length}</p></div>
                <div className="stat-card-enhanced text-center"><p className="stat-label">Draft Type</p><p className="text-lg font-bold text-[#1A1A1A]">{session.snakeDraft ? "Snake" : "Linear"}</p></div>
              </div>
              <div className="flex flex-wrap gap-3">
                {(session.status === "active" || session.status === "paused") && (
                  <>
                    <Button variant="secondary" onClick={handlePauseResume}>{session.status === "active" ? "Pause Draft" : "Resume Draft"}</Button>
                    <Link to={`/league/${leagueId}/draft`} className="brutal-btn brutal-btn-primary px-5 py-2.5 text-sm inline-flex items-center">Open Draft Board<svg className="w-4 h-4 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" /></svg></Link>
                  </>
                )}
              </div>
              <div className="mt-3">
                <Button variant="danger" size="sm" onClick={handleDeleteDraftSession}>Delete Draft Session</Button>
              </div>
            </>
          )}
        </div>
      </div>

      {/* ── Player Management ──────────────────────────────────────────────── */}
      {members.length > 0 && (
        <div className="fantasy-card">
          <div className="card-header">
            <div className="flex items-center justify-between">
              <h2 className="text-xl font-bold">Player Management</h2>
              {playersLoading && <span className="text-white/60 text-xs uppercase tracking-wider">Loading...</span>}
            </div>
          </div>
          <div className="p-6 space-y-6">
            {/* Add Player via NHL Search */}
            <div className="border-2 border-[#1A1A1A] rounded-none p-4 space-y-3 overflow-visible">
              <h3 className="text-xs font-bold text-gray-700 uppercase tracking-wider">Add Player</h3>
              <div className="flex flex-col sm:flex-row gap-3">
                <select
                  value={addPlayerTeamId}
                  onChange={(e) => setAddPlayerTeamId(parseInt(e.target.value))}
                  className="px-3 py-2 border-2 border-[#1A1A1A] rounded-none text-sm sm:w-56"
                >
                  <option value={0}>Select Team...</option>
                  {teamPlayers.map((tp) => (
                    <option key={tp.teamId} value={tp.teamId}>{tp.teamName} ({tp.memberName})</option>
                  ))}
                </select>
                <div ref={searchRef} className="relative flex-1">
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    onFocus={() => { setSearchFocused(true); fetchNhlPlayersCache(); }}
                    placeholder={nhlPlayersFetching ? "Loading NHL players..." : "Search NHL player..."}
                    disabled={!addPlayerTeamId}
                    className="w-full px-3 py-2 border-2 border-[#1A1A1A] rounded-none text-sm disabled:opacity-40 disabled:cursor-not-allowed"
                  />
                  {searchFocused && searchQuery.length >= 2 && searchResults.length > 0 && (
                    <div className="absolute z-40 left-0 right-0 top-full mt-0 bg-white border-2 border-[#1A1A1A] border-t-0 max-h-64 overflow-y-auto shadow-[4px_4px_0px_0px_rgba(0,0,0,0.1)]">
                      {searchResults.map((p) => (
                        <button
                          key={p.id}
                          onClick={() => handleAddPlayerFromSearch(p)}
                          disabled={addingPlayer}
                          className="w-full text-left px-3 py-2 hover:bg-[#2563EB]/10 transition-colors flex items-center gap-2 text-sm border-b border-gray-100 last:border-b-0 disabled:opacity-50"
                        >
                          <span className="font-medium text-gray-900">{p.firstName.default} {p.lastName.default}</span>
                          <span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 bg-gray-200 text-gray-600">{p.position}</span>
                          <span className="text-xs text-gray-500 ml-auto">{p.teamAbbrev}</span>
                        </button>
                      ))}
                    </div>
                  )}
                  {searchFocused && searchQuery.length >= 2 && searchResults.length === 0 && nhlPlayersCache && (
                    <div className="absolute z-40 left-0 right-0 top-full mt-0 bg-white border-2 border-[#1A1A1A] border-t-0 px-3 py-3 text-sm text-gray-500">
                      No available players match &quot;{searchQuery}&quot;
                    </div>
                  )}
                </div>
              </div>
              {!addPlayerTeamId && (
                <p className="text-xs text-gray-400">Select a team above to enable player search.</p>
              )}
            </div>

            {/* Team Rosters */}
            {playersLoading ? (
              <LoadingSpinner size="small" message="Loading team players..." />
            ) : teamPlayers.length === 0 ? (
              <p className="text-gray-500 text-sm">No teams with rosters found in this league.</p>
            ) : (
              <div className="space-y-3">
                {teamPlayers.map((tp) => (
                  <div key={tp.teamId} className="border-2 border-[#1A1A1A] rounded-none overflow-hidden">
                    <button onClick={() => toggleTeamExpanded(tp.teamId)} className="w-full flex items-center justify-between px-4 py-3 bg-gray-50 hover:bg-gray-100 transition-colors text-left">
                      <div>
                        <span className="font-semibold text-gray-900">{tp.teamName}</span>
                        <span className="text-gray-500 text-sm ml-2">({tp.memberName})</span>
                      </div>
                      <div className="flex items-center gap-3">
                        <span className="text-xs bg-[#2563EB]/10 text-[#2563EB] px-2 py-1 rounded-none font-medium">{tp.players.length + (tp.sleeper ? 1 : 0)} player{tp.players.length + (tp.sleeper ? 1 : 0) !== 1 ? "s" : ""}</span>
                        <svg className={`w-4 h-4 text-gray-400 transition-transform ${expandedTeams.has(tp.teamId) ? "rotate-180" : ""}`} fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" /></svg>
                      </div>
                    </button>
                    {expandedTeams.has(tp.teamId) && (
                      <div className="p-4 space-y-2 border-t border-[#1A1A1A]">
                        {tp.players.length === 0 && !tp.sleeper ? (
                          <p className="text-gray-500 text-sm">No players on this team.</p>
                        ) : (
                          <>
                            {tp.players.map((p) => (
                              <div key={p.id} className="flex items-center justify-between py-2 px-3 bg-gray-50 rounded-none border border-gray-200">
                                <div className="flex items-center gap-2">
                                  <span className="font-medium text-gray-900 text-sm">{p.name}</span>
                                  <span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 bg-gray-200 text-gray-600">{p.position}</span>
                                  <span className="text-xs text-gray-400">{p.nhl_team}</span>
                                </div>
                                <button onClick={() => handleDeletePlayer(p.id, p.name)} className="text-red-400 hover:text-red-600 hover:bg-red-50 rounded-none px-2 py-1 text-xs font-medium transition-colors">Remove</button>
                              </div>
                            ))}
                            {tp.sleeper && (
                              <div className="flex items-center justify-between py-2 px-3 bg-[#FACC15]/10 rounded-none border border-[#FACC15]/30 mt-2">
                                <div className="flex items-center gap-2">
                                  <span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 bg-[#FACC15] text-[#1A1A1A]">Sleeper</span>
                                  <span className="font-medium text-gray-900 text-sm">{tp.sleeper.name}</span>
                                  <span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 bg-gray-200 text-gray-600">{tp.sleeper.position}</span>
                                  <span className="text-xs text-gray-400">{tp.sleeper.nhl_team}</span>
                                </div>
                                <button onClick={() => handleDeleteSleeper(tp.sleeper!.id, tp.sleeper!.name)} className="text-red-400 hover:text-red-600 hover:bg-red-50 rounded-none px-2 py-1 text-xs font-medium transition-colors">Remove</button>
                              </div>
                            )}
                          </>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* ── Danger Zone ────────────────────────────────────────────────────── */}
      <div className="fantasy-card">
        <div className="card-header"><h2 className="text-xl font-bold text-red-400">Danger Zone</h2></div>
        <div className="p-6">
          <Button variant="danger" size="sm" onClick={handleDeleteLeague}>
            Delete League
          </Button>
          <p className="text-xs text-gray-400 mt-2">This will permanently delete the league and all associated data.</p>
        </div>
      </div>
    </div>
  );
};

export default LeagueSettingsPage;
