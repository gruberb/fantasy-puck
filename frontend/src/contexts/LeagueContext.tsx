import {
  createContext,
  useContext,
  useCallback,
  ReactNode,
} from "react";
import { useQuery } from "@tanstack/react-query";
import { useAuth } from "./AuthContext";
import { api } from "@/api/client";
import type { League } from "@/types/league";
import type { DraftSession } from "@/features/draft";

// -- Types ------------------------------------------------------------------

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
  activeLeagueId: string | null;
  setActiveLeagueId: (id: string | null) => void;
  activeLeague: League | null;
  allLeagues: League[];
  leaguesLoading: boolean;
  myMemberships: LeagueMembership[];
  myLeagues: League[];
  draftSession: DraftSession | null;
  loading: boolean;
}

const STORAGE_KEY = "lastViewedLeagueId";

const LeagueContext = createContext<LeagueContextType | undefined>(undefined);

// -- Helper to transform membership rows ------------------------------------

function transformMemberships(data: MembershipRow[], userId: string): LeagueMembership[] {
  return (data ?? []).map((m) => ({
    id: m.leagueId,
    league_id: m.leagueId,
    user_id: userId,
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
}

// -- Provider ---------------------------------------------------------------

import { useState } from "react";

export const LeagueProvider = ({ children }: { children: ReactNode }) => {
  const { user } = useAuth();

  // Rehydrate from localStorage on first mount so global routes like
  // `/games/:date` (which don't run LeagueShell) still know the last-viewed
  // league across a hard refresh.
  const [activeLeagueId, setActiveLeagueIdState] = useState<string | null>(
    () => (typeof window === "undefined" ? null : localStorage.getItem(STORAGE_KEY)),
  );

  // Set active league ID and persist to localStorage
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

  // Fetch leagues via React Query
  const leaguesQuery = useQuery({
    queryKey: ["leagues", user?.id ?? "public"],
    queryFn: () => api.getLeagues(!user),
  });

  const allLeagues: League[] = leaguesQuery.data ?? [];

  // Fetch memberships via React Query (only when logged in)
  const membershipsQuery = useQuery({
    queryKey: ["memberships", user?.id],
    queryFn: async () => {
      const data = (await api.getMemberships()) as MembershipRow[];
      return transformMemberships(data, user!.id);
    },
    enabled: !!user?.id,
  });

  const myMemberships: LeagueMembership[] = membershipsQuery.data ?? [];

  // Fetch draft session — uses same query key as useDraftSession so WS updates propagate
  const draftQuery = useQuery({
    queryKey: ["draft", "session", activeLeagueId],
    queryFn: async () => {
      const data = (await api.getDraftByLeague(activeLeagueId!)) as {
        session: DraftSession;
      } | null;
      return data?.session ?? null;
    },
    enabled: !!activeLeagueId,
  });

  // Derived
  const activeLeague = allLeagues.find((l) => l.id === activeLeagueId) ?? null;
  const myLeagues = myMemberships.map((m) => m.leagues);
  const draftSession = draftQuery.data ?? null;
  const loading = leaguesQuery.isLoading || membershipsQuery.isLoading;

  return (
    <LeagueContext.Provider
      value={{
        activeLeagueId,
        setActiveLeagueId,
        activeLeague,
        allLeagues,
        leaguesLoading: leaguesQuery.isLoading,
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

// -- Hook -------------------------------------------------------------------

export const useLeague = (): LeagueContextType => {
  const context = useContext(LeagueContext);
  if (context === undefined) {
    throw new Error("useLeague must be used within a LeagueProvider");
  }
  return context;
};
