import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import { realtimeService } from '@/lib/realtime';
import { draftPicksQueryKey } from './use-draft-picks';
import type { DraftSession, DraftPick } from '../types';

export const draftSessionQueryKey = (leagueId: string | null) =>
  ['draft', 'session', leagueId] as const;

export function useDraftSession(leagueId: string | null) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: draftSessionQueryKey(leagueId),
    queryFn: async () => {
      const data = await draftApi.getDraftByLeague(leagueId!);
      return data?.session ?? null;
    },
    enabled: !!leagueId,
  });

  const session = query.data ?? null;

  // Single WebSocket subscription handling ALL draft events (session, picks, sleepers)
  useEffect(() => {
    if (!session?.id) return;
    const sessionId = session.id;

    const unsubscribe = realtimeService.subscribeToDraft(sessionId, {
      onSessionUpdated: (partial) => {
        queryClient.setQueryData<DraftSession | null>(
          draftSessionQueryKey(leagueId),
          (prev) => {
            if (!prev) return prev;
            return { ...prev, ...partial };
          },
        );
        // Also invalidate to force a full refetch — ensures all fields are current
        queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(leagueId) });
      },
      onPickMade: (pick: DraftPick) => {
        queryClient.setQueryData<DraftPick[]>(
          draftPicksQueryKey(sessionId),
          (prev) => {
            if (!prev) return [pick];
            if (prev.some((p) => p.id === pick.id)) return prev;
            return [...prev, pick];
          },
        );
        if (leagueId) {
          queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(leagueId) });
        }
      },
      onSleeperUpdated: () => {
        queryClient.invalidateQueries({ queryKey: ['draft', 'eligibleSleepers', sessionId] });
        queryClient.invalidateQueries({ queryKey: ['draft', 'sleeperPicks', sessionId] });
        if (leagueId) {
          queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(leagueId) });
        }
      },
    });

    return () => {
      unsubscribe();
    };
  }, [session?.id, leagueId, queryClient]);

  return {
    session,
    loading: query.isLoading,
    fetchSession: query.refetch,
    setSession: (s: DraftSession | null) =>
      queryClient.setQueryData(draftSessionQueryKey(leagueId), s),
  };
}
