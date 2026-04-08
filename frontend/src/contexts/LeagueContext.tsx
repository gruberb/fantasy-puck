import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  ReactNode,
} from "react";
import { useAuth } from "./AuthContext";
import { api } from "@/api/client";
import type { League } from "@/types/league";
import type { DraftSession } from "@/features/draft";

// ── Types ──────────────────────────────────────────────────────────────────

export interface MembershipRow {
  leagueId: string;
  leagueName: string;
  leagueSeason: string;
  fantasyTeamId: number | null;
  teamName: string | null;
  draftOrder: number;
}

export interface LeagueMembership {
  id: string;
  league_id: string;
  user_id: string;
  fantasy_team_id: number;
  draft_order: number;
  leagues: League;
  fantasy_teams: { id: number; name: string } | null;
}

interface LeagueContextType {
  // The currently viewed league (from URL or selection)
  activeLeagueId: string | null;
  setActiveLeagueId: (id: string | null) => void;
  activeLeague: League | null;

  // All available leagues (fetched from backend)
  allLeagues: League[];
  leaguesLoading: boolean;

  // Auth-specific (only when logged in)
  myMemberships: LeagueMembership[];
  myLeagues: League[];

  // Draft session for the active league
  draftSession: DraftSession | null;

  loading: boolean;
}

const STORAGE_KEY = "lastViewedLeagueId";

const LeagueContext = createContext<LeagueContextType | undefined>(undefined);

// ── Provider ───────────────────────────────────────────────────────────────

export const LeagueProvider = ({ children }: { children: ReactNode }) => {
  const { user } = useAuth();

  const [allLeagues, setAllLeagues] = useState<League[]>([]);
  const [leaguesLoading, setLeaguesLoading] = useState(true);
  const [activeLeagueId, setActiveLeagueIdState] = useState<string | null>(
    null,
  );
  const [myMemberships, setMyMemberships] = useState<LeagueMembership[]>([]);
  const [draftSession, setDraftSession] = useState<DraftSession | null>(null);
  const [membershipsLoading, setMembershipsLoading] = useState(false);

  // Derived: list of leagues from memberships
  const myLeagues = myMemberships.map((m) => m.leagues);

  // Derived: active league object
  const activeLeague = allLeagues.find((l) => l.id === activeLeagueId) ?? null;

  // Set active league ID and persist to localStorage only when logged in
  const setActiveLeagueId = useCallback(
    (id: string | null) => {
      setActiveLeagueIdState(id);
      if (user) {
        if (id) {
          localStorage.setItem(STORAGE_KEY, id);
        } else {
          localStorage.removeItem(STORAGE_KEY);
        }
      }
    },
    [user],
  );

  // Fetch leagues - public only when not logged in, all when logged in
  useEffect(() => {
    let cancelled = false;

    const fetchLeagues = async () => {
      setLeaguesLoading(true);
      try {
        const publicOnly = !user;
        const leagues = await api.getLeagues(publicOnly);
        if (!cancelled) {
          setAllLeagues(leagues);
        }
      } catch (err) {
        console.error("Error fetching leagues:", err);
      } finally {
        if (!cancelled) {
          setLeaguesLoading(false);
        }
      }
    };

    fetchLeagues();
    return () => {
      cancelled = true;
    };
  }, [user]);

  // Fetch memberships when user is authenticated
  useEffect(() => {
    if (!user?.id) {
      setMyMemberships([]);
      return;
    }

    let cancelled = false;

    const fetchMemberships = async () => {
      setMembershipsLoading(true);
      try {
        const data = (await api.getMemberships()) as MembershipRow[];

        if (cancelled) return;

        // Transform the flat membership response into the expected shape
        const memberships: LeagueMembership[] = (data ?? []).map((m) => ({
          id: m.leagueId,
          league_id: m.leagueId,
          user_id: user.id,
          fantasy_team_id: m.fantasyTeamId ?? 0,
          draft_order: m.draftOrder,
          leagues: {
            id: m.leagueId,
            name: m.leagueName,
            season: m.leagueSeason,
          },
          fantasy_teams: m.fantasyTeamId
            ? { id: m.fantasyTeamId, name: m.teamName ?? "No team" }
            : null,
        }));

        setMyMemberships(memberships);
      } catch (err) {
        console.error("Error fetching league memberships:", err);
      } finally {
        if (!cancelled) {
          setMembershipsLoading(false);
        }
      }
    };

    fetchMemberships();
    return () => {
      cancelled = true;
    };
  }, [user?.id]);

  // Fetch draft session when active league changes
  useEffect(() => {
    if (!activeLeagueId) {
      setDraftSession(null);
      return;
    }

    let cancelled = false;

    const fetchDraft = async () => {
      try {
        const data = (await api.getDraftByLeague(activeLeagueId)) as {
          session: DraftSession;
        } | null;

        if (cancelled) return;

        if (data?.session) {
          setDraftSession(data.session);
        } else {
          setDraftSession(null);
        }
      } catch {
        if (!cancelled) {
          setDraftSession(null);
        }
      }
    };

    fetchDraft();
    return () => {
      cancelled = true;
    };
  }, [activeLeagueId]);

  const loading = leaguesLoading || membershipsLoading;

  return (
    <LeagueContext.Provider
      value={{
        activeLeagueId,
        setActiveLeagueId,
        activeLeague,
        allLeagues,
        leaguesLoading,
        myMemberships,
        myLeagues,
        draftSession,
        loading,
      }}
    >
      {children}
    </LeagueContext.Provider>
  );
};

// ── Hook ───────────────────────────────────────────────────────────────────

export const useLeague = (): LeagueContextType => {
  const context = useContext(LeagueContext);
  if (context === undefined) {
    throw new Error("useLeague must be used within a LeagueProvider");
  }
  return context;
};
