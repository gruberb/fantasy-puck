import { fetchApi, withLeague } from '@/lib/api-client';
import { authService } from '@/features/auth';
import {
  NHLTeam,
  FantasyTeamPoints,
  NHLTeamBetsResponse,
} from '@/types/fantasyTeams';
import { GamesResponse } from '@/types/games';
import { Ranking, RankingItem, PlayoffFantasyTeamRanking } from '@/types/rankings';
import { TopSkater, Skater } from '@/types/skaters';
import { PlayoffsResponse } from '@/types/playoffs';
import { TeamStats } from '@/types/teamStats';
import { LeagueStatsResponse } from '@/types/leagueStats';
import { League } from '@/types/league';
import { APP_CONFIG } from '@/config';

// API client functions
export const api = {
  // ── Auth ───────────────────────────────────────────────────────────────

  async login(email: string, password: string) {
    return authService.login(email, password);
  },

  async register(email: string, password: string, displayName: string) {
    return authService.register(email, password, displayName);
  },

  async logout() {
    return authService.logout();
  },

  async updateProfile(displayName: string) {
    return fetchApi("auth/profile", { method: "PUT", body: { displayName } });
  },

  async deleteAccount() {
    return fetchApi("auth/account", { method: "DELETE" });
  },

  async getMemberships() {
    return fetchApi("auth/memberships");
  },

  // ── Leagues ────────────────────────────────────────────────────────────

  async getLeagues(publicOnly?: boolean): Promise<League[]> {
    const endpoint = publicOnly ? "leagues?visibility=public" : "leagues";
    return fetchApi<League[]>(endpoint, { fallback: [] });
  },

  async createLeague(name: string, season?: string) {
    return fetchApi("leagues", { method: "POST", body: { name, season } });
  },

  async deleteLeague(leagueId: string) {
    return fetchApi(`leagues/${leagueId}`, { method: "DELETE" });
  },

  async getLeagueMembers(leagueId: string) {
    return fetchApi(`leagues/${leagueId}/members`);
  },

  async joinLeague(leagueId: string, teamName: string) {
    return fetchApi(`leagues/${leagueId}/join`, {
      method: "POST",
      body: { teamName },
    });
  },

  async removeLeagueMember(leagueId: string, memberId: string) {
    return fetchApi(`leagues/${leagueId}/members/${memberId}`, {
      method: "DELETE",
    });
  },

  // ── Fantasy Teams ──────────────────────────────────────────────────────

  async getTeams(leagueId: string): Promise<NHLTeam[]> {
    return fetchApi<NHLTeam[]>(withLeague("fantasy/teams", leagueId), {
      fallback: [],
    });
  },

  async getTeamPoints(
    leagueId: string,
    teamId: number,
  ): Promise<FantasyTeamPoints> {
    return fetchApi<FantasyTeamPoints>(
      withLeague(`fantasy/teams/${teamId}`, leagueId),
    );
  },

  async updateTeamName(teamId: number, name: string) {
    return fetchApi(`fantasy/teams/${teamId}`, {
      method: "PUT",
      body: { name },
    });
  },

  async addPlayerToTeam(
    teamId: number,
    player: {
      nhlId: number;
      name: string;
      position: string;
      nhlTeam: string;
    },
  ) {
    return fetchApi(`fantasy/teams/${teamId}/players`, {
      method: "POST",
      body: player,
    });
  },

  async removePlayer(playerId: number) {
    return fetchApi(`fantasy/players/${playerId}`, { method: "DELETE" });
  },

  // ── Rankings ────────────────────────────────────────────────────────────

  async getRankings(leagueId: string): Promise<Ranking[]> {
    return fetchApi<Ranking[]>(withLeague("fantasy/rankings", leagueId), {
      fallback: [],
    });
  },

  async getDailyFantasySummary(
    leagueId: string,
    date: string,
  ): Promise<RankingItem[]> {
    // Server wraps the list in `{ date, rankings: [...] }`; unwrap here
    // so the TS signature (a flat `RankingItem[]`) matches reality and
    // every caller's `Array.isArray(...)` guard actually succeeds.
    const response = await fetchApi<
      RankingItem[] | { date: string; rankings: RankingItem[] }
    >(withLeague(`fantasy/rankings/daily?date=${date}`, leagueId), {
      fallback: [],
    });
    if (Array.isArray(response)) return response;
    return response?.rankings ?? [];
  },

  async getPlayoffRankings(
    leagueId: string,
  ): Promise<PlayoffFantasyTeamRanking[]> {
    return fetchApi<PlayoffFantasyTeamRanking[]>(
      withLeague("fantasy/rankings/playoffs", leagueId),
      { fallback: [] },
    );
  },

  // ── NHL Data ────────────────────────────────────────────────────────────

  async getTopSkaters(
    limit: number,
    season: number,
    gameType: number,
    formGames: number,
    leagueId?: string | null,
  ): Promise<TopSkater[]> {
    const params = new URLSearchParams({
      limit: String(limit),
      season: String(season),
      game_type: String(gameType),
      form_games: String(formGames),
    });
    if (leagueId) params.set("league_id", leagueId);

    return fetchApi<TopSkater[]>(
      `nhl/skaters/top?${params.toString()}`,
      { fallback: [] },
    );
  },

  async getPlayoffs(season: string = APP_CONFIG.DEFAULT_SEASON): Promise<PlayoffsResponse> {
    return fetchApi<PlayoffsResponse>(`nhl/playoffs?season=${season}`, {
      fallback: {
        currentRound: 0,
        rounds: [],
        eliminatedTeams: [],
        teamsInPlayoffs: [],
        advancedTeams: [],
      } as PlayoffsResponse,
    });
  },

  async getSleepers(leagueId: string): Promise<Skater[]> {
    return fetchApi<Skater[]>(withLeague("fantasy/sleepers", leagueId), {
      fallback: [],
    });
  },

  async getTeamStats(leagueId: string): Promise<TeamStats[]> {
    return fetchApi<TeamStats[]>(
      withLeague("fantasy/team-stats", leagueId),
      { fallback: [] },
    );
  },

  async getLeagueStats(leagueId: string): Promise<LeagueStatsResponse> {
    return fetchApi<LeagueStatsResponse>(
      withLeague("fantasy/league-stats", leagueId),
      {
        fallback: { nhlTeamsRostered: [], topRosteredSkaters: [] },
      },
    );
  },

  async getGames(date: string, leagueId?: string): Promise<GamesResponse> {
    let endpoint = `nhl/games?date=${date}`;
    if (leagueId) {
      endpoint += `&league_id=${leagueId}&detail=extended`;
    }
    return fetchApi<GamesResponse>(endpoint);
  },

  async getTeamBets(leagueId: string): Promise<NHLTeamBetsResponse[]> {
    return fetchApi<NHLTeamBetsResponse[]>(
      withLeague("fantasy/team-bets", leagueId),
      { fallback: [] },
    );
  },

  // ── Draft ──────────────────────────────────────────────────────────────

  async getDraftByLeague(leagueId: string) {
    return fetchApi(`leagues/${leagueId}/draft`);
  },

  async getFantasyPlayers(leagueId: string) {
    return fetchApi(withLeague("fantasy/players", leagueId));
  },

  async removeSleeper(sleeperId: number) {
    return fetchApi(`fantasy/sleepers/${sleeperId}`, { method: "DELETE" });
  },

  async createDraftSession(
    leagueId: string,
    totalRounds: number,
    snakeDraft: boolean,
  ) {
    return fetchApi(`leagues/${leagueId}/draft`, {
      method: "POST",
      body: { totalRounds, snakeDraft },
    });
  },

  async getDraftState(draftId: string) {
    return fetchApi(`draft/${draftId}`);
  },

  async populatePlayerPool(draftId: string) {
    return fetchApi(`draft/${draftId}/populate`, { method: "POST" });
  },

  async randomizeDraftOrder(leagueId: string) {
    return fetchApi(`leagues/${leagueId}/draft/randomize-order`, {
      method: "POST",
    });
  },

  async startDraft(draftId: string) {
    return fetchApi(`draft/${draftId}/start`, { method: "POST" });
  },

  async pauseDraft(draftId: string) {
    return fetchApi(`draft/${draftId}/pause`, { method: "POST" });
  },

  async resumeDraft(draftId: string) {
    return fetchApi(`draft/${draftId}/resume`, { method: "POST" });
  },

  async deleteDraftSession(draftId: string) {
    return fetchApi(`draft/${draftId}`, { method: "DELETE" });
  },

  async makePick(draftId: string, playerPoolId: string) {
    return fetchApi(`draft/${draftId}/pick`, {
      method: "POST",
      body: { playerPoolId },
    });
  },

  async finalizeDraft(draftId: string) {
    return fetchApi(`draft/${draftId}/finalize`, { method: "POST" });
  },

  async getEligibleSleepers(draftId: string) {
    return fetchApi(`draft/${draftId}/sleepers`);
  },

  async startSleeperRound(draftId: string) {
    return fetchApi(`draft/${draftId}/sleeper/start`, { method: "POST" });
  },

  async makeSleeperPick(
    draftId: string,
    playerPoolId: string,
    teamId: number,
  ) {
    return fetchApi(`draft/${draftId}/sleeper/pick`, {
      method: "POST",
      body: { playerPoolId, teamId },
    });
  },
};
