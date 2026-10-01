import { createContext, useContext } from "react";
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

export interface LeagueContextType {
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

export const LeagueContext = createContext<LeagueContextType | undefined>(undefined);

export const useLeague = (): LeagueContextType => {
  const context = useContext(LeagueContext);
  if (context === undefined) {
    throw new Error("useLeague must be used within a LeagueProvider");
  }
  return context;
};
