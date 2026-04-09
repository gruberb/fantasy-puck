import { useState, useMemo, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import { useLeague } from "@/contexts/LeagueContext";
import { getNHLTeamFullName, getNHLTeamLogoUrl } from "@/utils/nhlTeams";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import ErrorMessage from "@/components/common/ErrorMessage";
import PageHeader from "@/components/common/PageHeader";
import {
  useLeagueMembers,
  useDraftSession,
  usePlayerPool,
  useDraftPicks,
  useMakePick,
  useFinalizeDraft,
  useSleeperRound,
  getPickerForPick,
  draftApi,
  type LeagueMember,
  type PlayerPoolEntry,
} from "@/features/draft";

// ── Position badge colors ──────────────────────────────────────────────────

const positionColor: Record<string, string> = {
  C: "bg-blue-100 text-blue-800",
  LW: "bg-green-100 text-green-800",
  RW: "bg-emerald-100 text-emerald-800",
  D: "bg-purple-100 text-purple-800",
  G: "bg-amber-100 text-amber-800",
};

function PositionBadge({ position }: { position: string }) {
  const cls = positionColor[position] || "bg-gray-100 text-gray-800";
  return (
    <span className={`${cls} text-xs font-semibold px-2 py-0.5 rounded-none border border-[#1A1A1A]`}>
      {position}
    </span>
  );
}

function TeamDropdown({ teams, value, onChange, playerCount }: { teams: string[]; value: string; onChange: (v: string) => void; playerCount: number }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const label = value === "All"
    ? `All Teams (${playerCount} players)`
    : `${getNHLTeamFullName(value)} (${value})`;

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 border-2 border-[#1A1A1A] rounded-none text-sm bg-white text-left"
      >
        {value !== "All" && <img src={getNHLTeamLogoUrl(value)} alt="" className="w-5 h-5" />}
        <span className="flex-1 truncate">{label}</span>
        <svg className={`w-4 h-4 transition-transform ${open ? "rotate-180" : ""}`} fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" /></svg>
      </button>
      {open && (
        <div className="absolute z-30 w-full mt-1 bg-white border-2 border-[#1A1A1A] max-h-64 overflow-y-auto shadow-lg">
          <button
            onClick={() => { onChange("All"); setOpen(false); }}
            className={`w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-gray-100 text-left ${value === "All" ? "bg-[#2563EB]/10 font-medium" : ""}`}
          >
            All Teams ({playerCount})
          </button>
          {teams.map((t) => (
            <button
              key={t}
              onClick={() => { onChange(t); setOpen(false); }}
              className={`w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-gray-100 text-left ${value === t ? "bg-[#2563EB]/10 font-medium" : ""}`}
            >
              <img src={getNHLTeamLogoUrl(t)} alt={t} className="w-5 h-5 flex-shrink-0" />
              <span>{getNHLTeamFullName(t)}</span>
              <span className="text-gray-400 text-xs">({t})</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ── DraftPage ──────────────────────────────────────────────────────────────

const DraftPage = () => {
  const { leagueId } = useParams<{ leagueId: string }>();
  const navigate = useNavigate();
  const { user, profile, loading: authLoading } = useAuth();
  const { activeLeague } = useLeague();
  const isLeagueOwner = !!(user && activeLeague?.created_by === user.id);

  const [searchQuery, setSearchQuery] = useState("");
  const [positionFilter, setPositionFilter] = useState<string>("All");
  const [teamFilter, setTeamFilter] = useState<string>("All");
  const [confirmPlayer, setConfirmPlayer] = useState<PlayerPoolEntry | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [mobileTab, setMobileTab] = useState<"players" | "board">("players");

  const { members, loading: membersLoading } = useLeagueMembers(leagueId ?? null);
  const { session, loading: sessionLoading, fetchSession } = useDraftSession(leagueId ?? null);
  const { players, loading: playersLoading } = usePlayerPool(session?.id ?? null);
  const { picks, loading: picksLoading, fetchPicks } = useDraftPicks(session?.id ?? null, leagueId);
  const { makePick, picking } = useMakePick();
  const { finalizeDraft, finalizing } = useFinalizeDraft();
  const [finalized, setFinalized] = useState(false);

  const sortedMembers = useMemo(
    () => [...members].sort((a, b) => (a.draftOrder ?? 0) - (b.draftOrder ?? 0)),
    [members],
  );

  const myMember = useMemo(
    () => members.find((m) => m.userId === user?.id),
    [members, user?.id],
  );

  const currentPicker = useMemo(() => {
    if (!session || sortedMembers.length === 0) return undefined;
    return getPickerForPick(session.currentPickIndex, sortedMembers, session.snakeDraft);
  }, [session, sortedMembers]);

  const isMyTurn = !!(currentPicker && myMember && currentPicker.id === myMember.id);

  const pickedPlayerPoolIds = useMemo(
    () => new Set(picks.map((p) => p.playerPoolId)),
    [picks],
  );

  const pickByPlayerPoolId = useMemo(() => {
    const map = new Map<string, (typeof picks)[0]>();
    picks.forEach((p) => map.set(p.playerPoolId, p));
    return map;
  }, [picks]);

  const memberTeamName = useMemo(() => {
    const map = new Map<string, string>();
    members.forEach((m) => {
      map.set(m.id, m.teamName ?? m.displayName ?? "Team");
    });
    return map;
  }, [members]);

  const allTeams = useMemo(() => {
    const teams = new Set(players.map((p) => p.nhlTeam));
    return Array.from(teams).sort();
  }, [players]);

  const filteredPlayers = useMemo(() => {
    return players.filter((p) => {
      if (positionFilter !== "All") {
        if (positionFilter === "F") {
          if (!["C", "L", "R"].includes(p.position)) return false;
        } else if (p.position !== positionFilter) {
          return false;
        }
      }
      if (teamFilter !== "All" && p.nhlTeam !== teamFilter) return false;
      if (searchQuery) {
        const q = searchQuery.toLowerCase();
        if (!p.name.toLowerCase().includes(q) && !p.nhlTeam.toLowerCase().includes(q))
          return false;
      }
      return true;
    });
  }, [players, positionFilter, teamFilter, searchQuery]);

  const sortedFilteredPlayers = useMemo(() => {
    const getLastName = (name: string) => {
      const parts = name.trim().split(/\s+/);
      return parts.length > 1 ? parts.slice(1).join(" ") : name;
    };
    const available = filteredPlayers
      .filter((p) => !pickedPlayerPoolIds.has(p.id))
      .sort((a, b) => getLastName(a.name).localeCompare(getLastName(b.name)));
    const picked = filteredPlayers
      .filter((p) => pickedPlayerPoolIds.has(p.id))
      .sort((a, b) => getLastName(a.name).localeCompare(getLastName(b.name)));
    return [...available, ...picked];
  }, [filteredPlayers, pickedPlayerPoolIds]);

  // Timeline-based pick slots (correct snake draft ordering)
  const pickSlots = useMemo(() => {
    if (!session || sortedMembers.length === 0) return [];
    const total = session.totalRounds * sortedMembers.length;
    return Array.from({ length: total }, (_, i) => ({
      index: i,
      round: Math.floor(i / sortedMembers.length),
      picker: getPickerForPick(i, sortedMembers, session.snakeDraft),
      pick: picks.find((p) => p.pickNumber === i) ?? null,
    }));
  }, [session, sortedMembers, picks]);

  const currentPickRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to current pick when it changes
  useEffect(() => {
    if (currentPickRef.current) {
      currentPickRef.current.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [session?.currentPickIndex]);

  useEffect(() => {
    if (session?.status !== "active") { setElapsed(0); return; }
    const interval = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(interval);
  }, [session?.status, session?.currentPickIndex]);

  useEffect(() => { setElapsed(0); }, [session?.currentPickIndex]);

  // Lock body scroll when confirm modal is open
  useEffect(() => {
    if (confirmPlayer) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => { document.body.style.overflow = ""; };
  }, [confirmPlayer]);

  const handlePick = async (player: PlayerPoolEntry) => {
    if (!session || !myMember || !isMyTurn || picking) return;
    try {
      await makePick(session.id, myMember.id, player, session.currentPickIndex, sortedMembers.length, leagueId);
      setConfirmPlayer(null);
      // WebSocket delivers the update to all clients (including this one)
    } catch (e: any) { console.error("Pick failed:", e); }
  };

  // ── Sleeper round hooks (must be before any early returns) ────────────────

  const {
    eligiblePlayers: sleeperEligible,
    eligibleLoading: sleeperEligibleLoading,
    sleeperPicks,
    sleeperPicker,
    sleeperPicking,
    startSleeperRound,
    makeSleeperPick,
  } = useSleeperRound(session, leagueId ?? null, members);

  const [sleeperSearch, setSleeperSearch] = useState("");
  const [sleeperPosFilter, setSleeperPosFilter] = useState("All");
  const [sleeperTeamFilter, setSleeperTeamFilter] = useState("All");
  const [confirmSleeper, setConfirmSleeper] = useState<PlayerPoolEntry | null>(null);

  const sleeperTeams = useMemo(() => {
    const teams = new Set(sleeperEligible.map((p) => p.nhlTeam));
    return Array.from(teams).sort();
  }, [sleeperEligible]);

  const filteredSleeperPlayers = useMemo(() => {
    const getLastName = (name: string) => {
      const parts = name.trim().split(/\s+/);
      return parts.length > 1 ? parts.slice(1).join(" ") : name;
    };
    return sleeperEligible
      .filter((p) => {
        if (sleeperPosFilter !== "All") {
          if (sleeperPosFilter === "F") {
            if (!["C", "L", "R"].includes(p.position)) return false;
          } else if (p.position !== sleeperPosFilter) return false;
        }
        if (sleeperTeamFilter !== "All" && p.nhlTeam !== sleeperTeamFilter) return false;
        if (sleeperSearch) {
          const q = sleeperSearch.toLowerCase();
          if (!p.name.toLowerCase().includes(q) && !p.nhlTeam.toLowerCase().includes(q)) return false;
        }
        return true;
      })
      .sort((a, b) => getLastName(a.name).localeCompare(getLastName(b.name)));
  }, [sleeperEligible, sleeperPosFilter, sleeperTeamFilter, sleeperSearch]);

  const isLoading = authLoading || membersLoading || sessionLoading || playersLoading || picksLoading;

  if (isLoading) return <LoadingSpinner message="Loading draft..." />;
  if (!leagueId) return <ErrorMessage message="No league specified." />;
  if (!session) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="fantasy-card max-w-md w-full">
          <div className="card-header text-center"><h2 className="text-xl font-bold">No Active Draft</h2></div>
          <div className="p-8 text-center text-gray-600">There is no draft session for this league yet. An admin needs to create and start one.</div>
        </div>
      </div>
    );
  }

  const picksComplete = session.status === "picks_done" || session.status === "completed" || session.currentPickIndex >= session.totalRounds * sortedMembers.length;
  const sleeperRoundActive = session.sleeperStatus === "active";
  const sleeperRoundComplete = session.sleeperStatus === "completed";

  const handleFinalize = async () => {
    if (!session || !leagueId || finalizing) return;
    try {
      await finalizeDraft(session.id, leagueId);
      setFinalized(true);
      await fetchSession(); // Refresh to get sleeper_status = "active"
    } catch (e: unknown) { console.error("Finalize failed:", e); }
  };

  const handleFinalizeSleepers = async () => {
    if (!session || !leagueId) return;
    try {
      await draftApi.completeDraft(session.id);
    } catch (e) {
      console.error("Failed to complete draft:", e);
    }
    navigate(`/league/${leagueId}`);
  };

  const isMySleeperTurn = !!(sleeperPicker && myMember && sleeperPicker.id === myMember.id);

  const handleSleeperPick = async (player: PlayerPoolEntry) => {
    if (!session || !myMember || !isMySleeperTurn || sleeperPicking) return;
    try {
      await makeSleeperPick(
        session.id,
        myMember.fantasyTeamId,
        player,
        session.sleeperPickIndex,
        sortedMembers.length,
      );
      setConfirmSleeper(null);
      // WebSocket delivers the update to all clients
    } catch (e) {
      console.error("Sleeper pick failed:", e);
    }
  };

  // ── Step 1: All picks done — admin needs to finalize ──────────────────────
  if (picksComplete && !sleeperRoundActive && !sleeperRoundComplete) {
    return (
      <div className="max-w-2xl mx-auto space-y-6">
        <PageHeader title="All Rounds Complete!" subtitle={`${session.totalRounds} rounds of drafting are done`} />

        <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6 text-center space-y-4">
          <p className="text-gray-600">All {session.totalRounds * sortedMembers.length} picks have been made. The league owner needs to finalize the draft before moving to the sleeper round.</p>

          {isLeagueOwner ? (
            <button
              onClick={handleFinalize}
              disabled={finalizing}
              className="btn-gradient text-lg px-8 py-3 disabled:opacity-50"
            >
              {finalizing ? "Finalizing..." : "Finalize Draft & Start Sleeper Round"}
            </button>
          ) : (
            <p className="text-sm text-gray-500">Waiting for the league owner to finalize the draft...</p>
          )}
        </div>

        {/* Team rosters preview */}
        {sortedMembers.map((member) => {
          const teamName = memberTeamName.get(member.id) ?? "Team";
          const teamPicks = picks.filter((p) => p.leagueMemberId === member.id);
          return (
            <div key={member.id} className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
              <div className="bg-[#2563EB] px-4 py-3">
                <h3 className="text-white font-bold">{teamName}</h3>
                <p className="text-white/70 text-xs">{teamPicks.length} players</p>
              </div>
              <div className="divide-y divide-gray-100">
                {teamPicks.map((pick) => (
                  <div key={pick.id} className="flex items-center gap-3 px-4 py-2">
                    <span className="text-xs text-gray-400 w-6">R{pick.round}</span>
                    <span className="font-medium text-sm text-gray-900 flex-1">{pick.playerName}</span>
                    <PositionBadge position={pick.position} />
                    <span className="text-xs text-gray-500">{pick.nhlTeam}</span>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  // ── Step 2: Sleeper round active ──────────────────────────────────────────
  if (sleeperRoundActive) {
    return (
      <div className="max-w-2xl mx-auto space-y-6">
        <PageHeader title="Sleeper Round" subtitle="Each team picks 1 dark horse bet — tracked separately from your main roster" />

        <div className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
          {/* Whose turn */}
          <div className="flex items-center justify-between bg-[#FACC15]/10 border-b-2 border-[#1A1A1A] p-4">
            <div>
              <p className="text-xs text-gray-500 uppercase tracking-wider">Now Picking</p>
              <p className="font-bold text-lg">{sleeperPicker ? memberTeamName.get(sleeperPicker.id) ?? "Unknown" : "..."}</p>
            </div>
            <div className="text-right">
              <p className="text-xs text-gray-500">Pick {(session.sleeperPickIndex ?? 0) + 1} of {sortedMembers.length}</p>
              {isMySleeperTurn && (
                <span className="inline-block mt-1 bg-[#FFB81C] text-[#1A1A1A] text-xs font-bold px-3 py-1 rounded-none animate-pulse">YOUR TURN!</span>
              )}
            </div>
          </div>

          {/* Already picked sleepers */}
          {sleeperPicks.length > 0 && (
            <div className="p-4 border-b border-gray-200">
              <p className="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">Sleepers Picked</p>
              <div className="divide-y divide-gray-100 border border-gray-200">
                {sleeperPicks.map((sp) => {
                  const owner = members.find((m) => m.fantasyTeamId === sp.teamId);
                  return (
                    <div key={sp.id} className="flex items-center gap-3 px-3 py-2 text-sm">
                      <span className="font-bold text-xs uppercase tracking-wider text-[#1A1A1A] w-28 truncate">{owner ? memberTeamName.get(owner.id) ?? "Team" : "Team"}</span>
                      <span className="font-medium text-gray-900 flex-1">{sp.name}</span>
                      <PositionBadge position={sp.position} />
                      <span className="text-xs text-gray-500">{sp.nhlTeam}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Player search and selection */}
          <div className="p-4 space-y-3">
            <input type="text" value={sleeperSearch} onChange={(e) => setSleeperSearch(e.target.value)} placeholder="Search available players..." className="w-full px-3 py-2 border-2 border-[#1A1A1A] rounded-none text-sm focus:ring-2 focus:ring-[#FACC15]/40 focus:border-[#FACC15] outline-none" />
            <div className="flex flex-wrap gap-1.5">
              {["All", "F", "C", "L", "R", "D"].map((pos) => (
                <button key={pos} onClick={() => setSleeperPosFilter(pos)} className={`px-3 py-1 rounded-none text-xs font-medium transition-all ${sleeperPosFilter === pos ? "bg-[#FACC15] text-[#1A1A1A]" : "bg-gray-100 text-gray-600 hover:bg-gray-200"}`}>{pos}</button>
              ))}
            </div>
            <TeamDropdown teams={sleeperTeams} value={sleeperTeamFilter} onChange={setSleeperTeamFilter} playerCount={sleeperEligible.length} />
            {sleeperEligibleLoading ? (
              <LoadingSpinner message="Loading eligible players..." />
            ) : (
              <div className="max-h-[40vh] overflow-y-auto space-y-2 pr-1">
                {filteredSleeperPlayers.length === 0 ? (
                  <p className="text-gray-500 text-sm text-center py-6">No players match filters</p>
                ) : filteredSleeperPlayers.map((player) => {
                  const canPick = isMySleeperTurn && !sleeperPicking;
                  return (
                    <div key={player.id} onClick={() => { if (canPick) setConfirmSleeper(player); }} className={`flex items-center gap-3 p-3 rounded-none border-2 transition-all ${canPick ? "border-[#FACC15]/30 bg-white hover:bg-[#FACC15]/5 hover:border-[#FACC15]/50 cursor-pointer" : "border-gray-200 bg-white"}`}>
                      <img src={player.headshotUrl} alt={player.name} className="w-10 h-10 rounded-none object-cover bg-gray-200 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }} />
                      <div className="flex-1 min-w-0">
                        <p className="font-medium text-sm text-gray-900 truncate">{player.name}</p>
                        <div className="flex items-center gap-2 mt-0.5"><PositionBadge position={player.position} /><span className="text-xs text-gray-500">{player.nhlTeam}</span></div>
                      </div>
                      {canPick && <span className="bg-[#FACC15] text-[#1A1A1A] px-3 py-1.5 rounded-none text-xs font-semibold border border-[#1A1A1A]">Pick</span>}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Sleeper confirm modal */}
        {confirmSleeper && createPortal(
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" onClick={() => setConfirmSleeper(null)}>
            <div className="fantasy-card max-w-sm w-full border-2 border-[#1A1A1A]" onClick={(e) => e.stopPropagation()}>
              <div className="card-header text-center" style={{ background: "#FACC15", color: "#1A1A1A" }}><h3 className="text-lg font-bold">Confirm Sleeper Pick</h3></div>
              <div className="p-6 text-center space-y-4">
                <img src={confirmSleeper.headshotUrl} alt={confirmSleeper.name} className="w-20 h-20 rounded-none object-cover mx-auto bg-gray-200" onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }} />
                <div>
                  <p className="text-lg font-bold text-gray-900">{confirmSleeper.name}</p>
                  <div className="flex items-center justify-center gap-2 mt-1"><PositionBadge position={confirmSleeper.position} /><span className="text-sm text-gray-500">{confirmSleeper.nhlTeam}</span></div>
                </div>
                <p className="text-sm text-gray-600">Pick this player as your sleeper? Tracked separately from your main roster.</p>
                <div className="flex gap-3 justify-center">
                  <button onClick={() => setConfirmSleeper(null)} className="btn-secondary-enhanced">Cancel</button>
                  <button onClick={() => handleSleeperPick(confirmSleeper)} disabled={sleeperPicking} className="bg-[#FACC15] text-[#1A1A1A] font-bold px-6 py-2 rounded-none border-2 border-[#1A1A1A] shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all disabled:opacity-50">{sleeperPicking ? "Picking..." : "Confirm Sleeper"}</button>
                </div>
              </div>
            </div>
          </div>,
          document.body
        )}
      </div>
    );
  }

  // ── Step 3: Sleeper round complete — finalize and redirect ────────────────
  if (sleeperRoundComplete) {
    return (
      <div className="max-w-2xl mx-auto space-y-6">
        <PageHeader title="Draft & Sleeper Selection Complete!" subtitle="All teams are ready for the 2025/2026 Playoffs" />

        <div className="bg-white rounded-none border-2 border-[#1A1A1A] p-6 text-center space-y-4">
          <p className="text-gray-600">Every team has drafted their roster and picked a sleeper.</p>
          {isLeagueOwner ? (
            <button
              onClick={handleFinalizeSleepers}
              className="btn-gradient text-lg px-8 py-3"
            >
              Finalize & Go to League Overview
            </button>
          ) : (
            <Link to={`/league/${leagueId}`} className="inline-block btn-gradient text-lg px-8 py-3">
              Go to League Overview
            </Link>
          )}
        </div>

        {/* Sleeper Overview */}
        <div className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
          <div className="bg-[#FACC15] px-4 py-3 border-b-2 border-[#1A1A1A]">
            <h2 className="font-extrabold text-[#1A1A1A] uppercase tracking-wider">Sleeper Picks</h2>
            <p className="text-[#1A1A1A]/70 text-xs mt-0.5">Each team's dark horse bet for the playoffs</p>
          </div>
          <div className="divide-y divide-gray-200">
            {sortedMembers.map((member) => {
              const teamName = memberTeamName.get(member.id) ?? "Team";
              const teamSleeper = sleeperPicks.find((sp) => sp.teamId === member.fantasyTeamId);
              return (
                <div key={member.id} className="flex items-center gap-3 px-4 py-3">
                  <span className="font-bold text-sm uppercase tracking-wider text-[#1A1A1A] w-32 truncate">{teamName}</span>
                  {teamSleeper ? (
                    <>
                      <img src={`https://assets.nhle.com/mugs/nhl/latest/${teamSleeper.nhlId}.png`} alt="" className="w-10 h-10 rounded-none object-cover bg-gray-200 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }} />
                      <span className="font-medium text-sm text-gray-900 flex-1">{teamSleeper.name}</span>
                      <PositionBadge position={teamSleeper.position} />
                      <img src={getNHLTeamLogoUrl(teamSleeper.nhlTeam)} alt="" className="w-5 h-5" />
                      <span className="text-xs text-gray-500">{teamSleeper.nhlTeam}</span>
                    </>
                  ) : (
                    <span className="text-sm text-gray-400 italic">No sleeper picked</span>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Team rosters */}
        {sortedMembers.map((member) => {
          const teamName = memberTeamName.get(member.id) ?? "Team";
          const teamPicks = picks.filter((p) => p.leagueMemberId === member.id);
          const teamSleeper = sleeperPicks.find((sp) => sp.teamId === member.fantasyTeamId);
          return (
            <div key={member.id} className="bg-white rounded-none border-2 border-[#1A1A1A] overflow-hidden">
              <div className="bg-[#2563EB] px-4 py-3">
                <h3 className="text-white font-bold">{teamName}</h3>
                <p className="text-white/70 text-xs">{teamPicks.length} players{teamSleeper ? " + 1 sleeper" : ""}</p>
              </div>
              <div className="divide-y divide-gray-100">
                {teamPicks.map((pick) => (
                  <div key={pick.id} className="flex items-center gap-3 px-4 py-2">
                    <span className="text-xs text-gray-400 w-6">R{pick.round}</span>
                    <span className="font-medium text-sm text-gray-900 flex-1">{pick.playerName}</span>
                    <PositionBadge position={pick.position} />
                    <span className="text-xs text-gray-500">{pick.nhlTeam}</span>
                  </div>
                ))}
                {teamSleeper && (
                  <div className="flex items-center gap-3 px-4 py-2 bg-[#FACC15]/10">
                    <span className="text-xs text-[#FACC15] font-bold w-6">S</span>
                    <span className="font-medium text-sm text-gray-900 flex-1">{teamSleeper.name}</span>
                    <PositionBadge position={teamSleeper.position} />
                    <span className="text-xs text-gray-500">{teamSleeper.nhlTeam}</span>
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  // ── Active draft — main draft board ──────────────────────────────────────

  const formatTime = (seconds: number) => { const m = Math.floor(seconds / 60); const s = seconds % 60; return `${m}:${s.toString().padStart(2, "0")}`; };

  return (
    <div className="space-y-4">
      <PageHeader title="Draft Board" badge={`Round ${session.currentRound} of ${session.totalRounds}`}>
        <div className="flex flex-wrap items-center gap-3">
          <span className={`text-xs px-3 py-1 rounded-none font-medium ${session.status === "active" ? "bg-green-400/20 text-green-800" : session.status === "paused" ? "bg-yellow-400/20 text-yellow-800" : session.status === "completed" || session.status === "picks_done" ? "bg-blue-400/20 text-blue-800" : "bg-gray-100 text-gray-800"}`}>{session.status === "picks_done" ? "COMPLETED" : session.status.toUpperCase()}</span>
          {session.status === "active" && <span className="text-gray-500 text-xs font-mono">{formatTime(elapsed)}</span>}
          {!picksComplete && currentPicker ? (
            <div className="text-right">
              <p className="text-gray-500 text-xs">Now Picking</p>
              <p className="text-lg font-bold">{memberTeamName.get(currentPicker.id) ?? "Unknown"}</p>
              {isMyTurn && <span className="inline-block mt-1 bg-[#FFB81C] text-[#1A1A1A] text-xs font-bold px-3 py-1 rounded-none animate-pulse">YOUR TURN!</span>}
            </div>
          ) : picksComplete ? (
            <div className="text-center">
              <p className="text-lg font-bold text-yellow-600">Draft Complete!</p>
              {isLeagueOwner && !finalized && (
                <button onClick={handleFinalize} disabled={finalizing} className="mt-2 bg-[#FFB81C] text-[#1A1A1A] text-sm font-bold px-4 py-2 rounded-none hover:bg-yellow-400 transition-all disabled:opacity-50 cursor-pointer">{finalizing ? "Syncing..." : "Finalize & Save Teams"}</button>
              )}
            </div>
          ) : null}
        </div>
      </PageHeader>

      {/* Mobile tab switcher */}
      <div className="lg:hidden flex border-2 border-[#1A1A1A] mb-4">
        <button
          onClick={() => setMobileTab("players")}
          className={`flex-1 py-2.5 text-xs font-bold uppercase tracking-wider transition-colors ${
            mobileTab === "players"
              ? "bg-[#1A1A1A] text-white"
              : "bg-white text-[#1A1A1A] hover:bg-gray-100"
          }`}
        >
          Available Players
        </button>
        <button
          onClick={() => setMobileTab("board")}
          className={`flex-1 py-2.5 text-xs font-bold uppercase tracking-wider border-l-2 border-[#1A1A1A] transition-colors ${
            mobileTab === "board"
              ? "bg-[#1A1A1A] text-white"
              : "bg-white text-[#1A1A1A] hover:bg-gray-100"
          }`}
        >
          Draft Board
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
        <div className={`lg:col-span-2 ${mobileTab !== "players" ? "hidden lg:block" : ""}`}>
          <div className="fantasy-card">
            <div className="card-header">
              <h3 className="text-lg font-bold">Available Players</h3>
              <p className="text-white/70 text-xs mt-0.5">{players.length - pickedPlayerPoolIds.size} available / {players.length} total</p>
            </div>
            <div className="p-4 space-y-3">
              <input type="text" value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="Search players..." className="w-full px-3 py-2 border-2 border-[#1A1A1A] rounded-none text-sm focus:ring-2 focus:ring-[#2563EB]/40 focus:border-[#2563EB] outline-none" />
              <div className="flex flex-wrap gap-1.5">
                {["All", "F", "C", "L", "R", "D"].map((pos) => (
                  <button key={pos} onClick={() => setPositionFilter(pos)} className={`px-3 py-1 rounded-none text-xs font-medium transition-all ${positionFilter === pos ? "bg-[#2563EB] text-white" : "bg-gray-100 text-gray-600 hover:bg-gray-200"}`}>{pos}</button>
                ))}
              </div>
              <TeamDropdown teams={allTeams} value={teamFilter} onChange={setTeamFilter} playerCount={players.length} />
              <div className="lg:max-h-[60vh] lg:overflow-y-auto custom-scrollbar space-y-2 pr-1">
                {sortedFilteredPlayers.length === 0 ? (
                  <p className="text-gray-500 text-sm text-center py-6">No players match filters</p>
                ) : sortedFilteredPlayers.map((player) => {
                  const isPicked = pickedPlayerPoolIds.has(player.id);
                  const pick = pickByPlayerPoolId.get(player.id);
                  const pickedByTeam = pick ? memberTeamName.get(pick.leagueMemberId) : null;
                  const canPick = !isPicked && session.status === "active" && isMyTurn && !picking;
                  return (
                    <div key={player.id} onClick={() => { if (canPick) setConfirmPlayer(player); }} className={`flex items-stretch rounded-none border-2 overflow-hidden transition-all ${isPicked ? "border-gray-200 bg-gray-50 opacity-60" : canPick ? "border-[#2563EB]/30 bg-white hover:bg-[#2563EB]/5 hover:border-[#2563EB]/50 cursor-pointer" : "border-gray-200 bg-white"}`}>
                      <img src={player.headshotUrl} alt={player.name} className="w-14 h-auto object-cover bg-gray-200 flex-shrink-0" onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }} />
                      <div className="flex-1 min-w-0 p-2.5">
                        <p className={`font-medium text-sm truncate ${isPicked ? "line-through text-gray-500" : "text-gray-900"}`}>{player.name}</p>
                        <div className="flex items-center gap-2 mt-0.5">
                          <PositionBadge position={player.position} />
                          <img src={getNHLTeamLogoUrl(player.nhlTeam)} alt={player.nhlTeam} className="w-4 h-4" />
                          <span className="text-xs text-gray-500">{player.nhlTeam}</span>
                        </div>
                        {isPicked && pickedByTeam && <p className="text-xs text-gray-400 mt-0.5">Picked by {pickedByTeam}</p>}
                      </div>
                      {!isPicked && session.status === "active" && (
                        <div className="flex-shrink-0 flex items-center px-2">
                          {canPick ? <span className="bg-[#2563EB] text-white px-3 py-1.5 rounded-none text-xs font-semibold border border-[#1A1A1A]">Pick</span> : <span className="text-xs text-gray-400">Waiting...</span>}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>

        <div className={`lg:col-span-3 ${mobileTab !== "board" ? "hidden lg:block" : ""}`}>
          <div className="fantasy-card">
            <div className="card-header">
              <h3 className="text-lg font-bold">Draft Board</h3>
              <p className="text-white/70 text-xs mt-0.5">{picks.length} picks made</p>
            </div>
            <div className="lg:max-h-[65vh] lg:overflow-y-auto custom-scrollbar">
              {Array.from({ length: session.totalRounds }, (_, roundIdx) => {
                const roundSlots = pickSlots.filter((s) => s.round === roundIdx);
                return (
                  <div key={roundIdx}>
                    <div className="sticky top-0 z-10 flex items-center gap-2 py-2 px-3 bg-[#F5F0E8] border-b-2 border-[#1A1A1A]">
                      <span className="text-xs font-bold uppercase tracking-wider text-[#1A1A1A]">Round {roundIdx + 1}</span>
                      {session.snakeDraft && roundIdx % 2 === 1 && (
                        <span className="text-[10px] text-gray-500 font-mono">&#8617; SNAKE</span>
                      )}
                    </div>
                    <div className="divide-y divide-[#1A1A1A]/10">
                      {roundSlots.map((slot) => {
                        const isCurrent = slot.index === session.currentPickIndex && session.status === "active";
                        const teamName = memberTeamName.get(slot.picker?.id ?? "") ?? "Unknown";
                        return (
                          <div
                            key={slot.index}
                            ref={isCurrent ? currentPickRef : undefined}
                            className={`flex items-center gap-3 px-3 py-2 text-sm ${
                              isCurrent
                                ? "bg-[#2563EB]/10 ring-2 ring-inset ring-[#2563EB]/40"
                                : slot.pick
                                  ? "bg-white"
                                  : "bg-gray-50/30"
                            }`}
                          >
                            <span className="text-xs text-gray-400 w-6 font-mono flex-shrink-0">#{slot.index + 1}</span>
                            <span className={`font-bold text-xs uppercase tracking-wider truncate w-28 flex-shrink-0 ${isCurrent ? "text-[#2563EB]" : "text-[#1A1A1A]"}`}>
                              {teamName}
                            </span>
                            {slot.pick ? (
                              <>
                                <span className="font-semibold text-gray-900 flex-1 truncate">{slot.pick.playerName}</span>
                                <PositionBadge position={slot.pick.position} />
                                <span className="text-xs text-gray-400 flex-shrink-0">{slot.pick.nhlTeam}</span>
                              </>
                            ) : isCurrent ? (
                              <div className="w-3 h-3 rounded-full bg-[#2563EB] animate-pulse" />
                            ) : (
                              <span className="text-gray-300 text-xs">&mdash;</span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {confirmPlayer && createPortal(
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" onClick={() => setConfirmPlayer(null)}>
          <div className="fantasy-card max-w-sm w-full border-2 border-[#1A1A1A]" onClick={(e) => e.stopPropagation()}>
            <div className="card-header text-center"><h3 className="text-lg font-bold">Confirm Pick</h3></div>
            <div className="p-6 text-center space-y-4">
              <img src={confirmPlayer.headshotUrl} alt={confirmPlayer.name} className="w-20 h-20 rounded-none object-cover mx-auto bg-gray-200" onError={(e) => { (e.target as HTMLImageElement).src = "https://assets.nhle.com/mugs/nhl/latest/default.png"; }} />
              <div>
                <p className="text-lg font-bold text-gray-900">{confirmPlayer.name}</p>
                <div className="flex items-center justify-center gap-2 mt-1">
                  <PositionBadge position={confirmPlayer.position} />
                  <span className="text-sm text-gray-500">{confirmPlayer.nhlTeam}</span>
                </div>
              </div>
              <p className="text-sm text-gray-600">Are you sure you want to draft this player?</p>
              <div className="flex gap-3 justify-center">
                <button onClick={() => setConfirmPlayer(null)} className="btn-secondary-enhanced">Cancel</button>
                <button onClick={() => handlePick(confirmPlayer)} disabled={picking} className="btn-gradient disabled:opacity-50">{picking ? "Picking..." : "Confirm Pick"}</button>
              </div>
            </div>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
};

export default DraftPage;
