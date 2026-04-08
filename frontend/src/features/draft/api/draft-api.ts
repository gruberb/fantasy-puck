import { fetchApi } from '@/lib/api-client';
import type { DraftSession, DraftPick, PlayerPoolEntry, LeagueMember } from '../types';

// ── Response shapes ───────────────────────────────────────────────────────

export type DraftByLeagueResponse = DraftStateResponse | null;

export interface DraftStateResponse {
  session: DraftSession;
  picks: DraftPick[];
  playerPool: PlayerPoolEntry[];
  members: LeagueMember[];
}

// ── Draft API ─────────────────────────────────────────────────────────────

export const draftApi = {
  getDraftByLeague: (leagueId: string) =>
    fetchApi<DraftByLeagueResponse>(`leagues/${leagueId}/draft`),

  getDraftState: (draftId: string) =>
    fetchApi<DraftStateResponse>(`draft/${draftId}`),

  getLeagueMembers: (leagueId: string) =>
    fetchApi<LeagueMember[]>(`leagues/${leagueId}/members`, { fallback: [] }),

  createDraftSession: (leagueId: string, totalRounds: number, snakeDraft: boolean) =>
    fetchApi<DraftSession>(`leagues/${leagueId}/draft`, {
      method: 'POST',
      body: { totalRounds, snakeDraft },
    }),

  populatePlayerPool: (draftId: string) =>
    fetchApi<PlayerPoolEntry[]>(`draft/${draftId}/populate`, { method: 'POST' }),

  randomizeDraftOrder: (leagueId: string) =>
    fetchApi(`leagues/${leagueId}/draft/randomize-order`, { method: 'POST' }),

  startDraft: (draftId: string) =>
    fetchApi(`draft/${draftId}/start`, { method: 'POST' }),

  pauseDraft: (draftId: string) =>
    fetchApi(`draft/${draftId}/pause`, { method: 'POST' }),

  resumeDraft: (draftId: string) =>
    fetchApi(`draft/${draftId}/resume`, { method: 'POST' }),

  deleteDraftSession: (draftId: string) =>
    fetchApi(`draft/${draftId}`, { method: 'DELETE' }),

  makePick: (draftId: string, playerPoolId: string) =>
    fetchApi(`draft/${draftId}/pick`, {
      method: 'POST',
      body: { playerPoolId },
    }),

  finalizeDraft: (draftId: string) =>
    fetchApi(`draft/${draftId}/finalize`, { method: 'POST' }),

  completeDraft: (draftId: string) =>
    fetchApi(`draft/${draftId}/complete`, { method: 'POST' }),

  getEligibleSleepers: (draftId: string) =>
    fetchApi<PlayerPoolEntry[]>(`draft/${draftId}/sleepers`, { fallback: [] }),

  getSleeperPicks: (draftId: string) =>
    fetchApi<{ id: number; teamId: number; nhlId: number; name: string; position: string; nhlTeam: string }[]>(
      `draft/${draftId}/sleeper-picks`,
      { fallback: [] },
    ),

  startSleeperRound: (draftId: string) =>
    fetchApi(`draft/${draftId}/sleeper/start`, { method: 'POST' }),

  makeSleeperPick: (draftId: string, playerPoolId: string, teamId: number) =>
    fetchApi(`draft/${draftId}/sleeper/pick`, {
      method: 'POST',
      body: { playerPoolId, teamId },
    }),
};
