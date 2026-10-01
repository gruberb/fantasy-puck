import { useCallback, useState, ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useAuth } from "./use-auth";
import { LeagueContext } from "./use-league";
import type { LeagueMembership, MembershipRow } from "./use-league";
import type { League } from "@/types/league";
import type { DraftSession } from "@/features/draft";
import { api } from "@/api/client";
import { LAST_VIEWED_LEAGUE_KEY } from "@/config";
import { leagueKeys, membershipKeys } from "@/features/draft/hooks/use-leagues";
import { draftSessionQueryKey } from "@/features/draft/hooks/use-draft-session";

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

export const LeagueProvider = ({ children }: { children: ReactNode }) => {
  const { user } = useAuth();

  // Rehydrate from localStorage on first mount so global routes like
  // `/games/:date` (which don't run LeagueShell) still know the last-viewed
  // league across a hard refresh.
  const [activeLeagueId, setActiveLeagueIdState] = useState<string | null>(
    () => (typeof window === "undefined" ? null : localStorage.getItem(LAST_VIEWED_LEAGUE_KEY)),
  );

  // Set active league ID and persist to localStorage
  const setActiveLeagueId = useCallback(
    (id: string | null) => {
      setActiveLeagueIdState(id);
      if (user) {
        if (id) {
          localStorage.setItem(LAST_VIEWED_LEAGUE_KEY, id);
        } else {
          localStorage.removeItem(LAST_VIEWED_LEAGUE_KEY);
        }
      }
    },
    [user],
  );

  // Fetch leagues via React Query
  const leaguesQuery = useQuery({
    queryKey: leagueKeys.list(user?.id),
    queryFn: () => api.getLeagues(!user),
  });

  const allLeagues: League[] = leaguesQuery.data ?? [];

  // Fetch memberships via React Query (only when logged in)
  const membershipsQuery = useQuery({
    queryKey: membershipKeys.forUser(user?.id),
    queryFn: async () => {
      const data = (await api.getMemberships()) as MembershipRow[];
      return transformMemberships(data, user!.id);
    },
    enabled: !!user?.id,
  });

  const myMemberships: LeagueMembership[] = membershipsQuery.data ?? [];

  // Shares useDraftSession's key so WS updates propagate here too.
  const draftQuery = useQuery({
    queryKey: draftSessionQueryKey(activeLeagueId),
    queryFn: async () => {
      const data = (await api.getDraftByLeague(activeLeagueId!)) as {
        session: DraftSession;
      } | null;
      return data?.session ?? null;
    },
    enabled: !!activeLeagueId && !!user?.id,
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
