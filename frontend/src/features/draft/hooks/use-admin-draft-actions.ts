import { useMutation } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import type { DraftSession } from '../types';

export function useAdminDraftActions() {
  const createSessionMutation = useMutation({
    mutationFn: (args: { leagueId: string; totalRounds: number; snakeDraft: boolean }) =>
      draftApi.createDraftSession(args.leagueId, args.totalRounds, args.snakeDraft),
  });

  const populatePoolMutation = useMutation({
    mutationFn: (draftSessionId: string) => draftApi.populatePlayerPool(draftSessionId),
  });

  const randomizeOrderMutation = useMutation({
    mutationFn: (leagueId: string) => draftApi.randomizeDraftOrder(leagueId),
  });

  const startDraftMutation = useMutation({
    mutationFn: (sessionId: string) => draftApi.startDraft(sessionId),
  });

  const pauseDraftMutation = useMutation({
    mutationFn: (sessionId: string) => draftApi.pauseDraft(sessionId),
  });

  const resumeDraftMutation = useMutation({
    mutationFn: (sessionId: string) => draftApi.resumeDraft(sessionId),
  });

  const completeDraftMutation = useMutation({
    mutationFn: (sessionId: string) => draftApi.finalizeDraft(sessionId),
  });

  // Expose the same imperative API shape the pages expect
  const createDraftSession = async (
    leagueId: string,
    totalRounds: number,
    snakeDraft: boolean,
  ): Promise<DraftSession> => {
    return createSessionMutation.mutateAsync({ leagueId, totalRounds, snakeDraft });
  };

  const populatePlayerPool = async (draftSessionId: string): Promise<number> => {
    const data = await populatePoolMutation.mutateAsync(draftSessionId);
    return (data as unknown[] ?? []).length;
  };

  const randomizeDraftOrder = async (leagueId: string): Promise<void> => {
    await randomizeOrderMutation.mutateAsync(leagueId);
  };

  const startDraft = async (sessionId: string): Promise<void> => {
    await startDraftMutation.mutateAsync(sessionId);
  };

  const pauseDraft = async (sessionId: string): Promise<void> => {
    await pauseDraftMutation.mutateAsync(sessionId);
  };

  const resumeDraft = async (sessionId: string): Promise<void> => {
    await resumeDraftMutation.mutateAsync(sessionId);
  };

  const completeDraft = async (sessionId: string): Promise<void> => {
    await completeDraftMutation.mutateAsync(sessionId);
  };

  return {
    createDraftSession,
    populatePlayerPool,
    randomizeDraftOrder,
    startDraft,
    pauseDraft,
    resumeDraft,
    completeDraft,
  };
}
