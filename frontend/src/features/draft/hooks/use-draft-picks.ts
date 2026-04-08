import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import { draftSessionQueryKey } from './use-draft-session';
import { realtimeService } from '@/lib/realtime';
import type { DraftPick } from '../types';

export const draftPicksQueryKey = (draftSessionId: string | null) =>
  ['draft', 'picks', draftSessionId] as const;

export function useDraftPicks(draftSessionId: string | null, leagueId?: string | null) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: draftPicksQueryKey(draftSessionId),
    queryFn: async () => {
      const data = await draftApi.getDraftState(draftSessionId!);
      return (data?.picks ?? []) as DraftPick[];
    },
    enabled: !!draftSessionId,
  });

  // Subscribe to realtime pick inserts via WebSocket
  useEffect(() => {
    if (!draftSessionId) return;

    const unsubscribe = realtimeService.subscribeToDraft(draftSessionId, {
      onPickMade: (pick) => {
        queryClient.setQueryData<DraftPick[]>(
          draftPicksQueryKey(draftSessionId),
          (prev) => {
            if (!prev) return [pick];
            if (prev.some((p) => p.id === pick.id)) return prev;
            return [...prev, pick];
          },
        );
        // Also invalidate session so currentRound/currentPickIndex update
        if (leagueId) {
          queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(leagueId) });
        }
      },
    });

    return () => {
      unsubscribe();
    };
  }, [draftSessionId, leagueId, queryClient]);

  return {
    picks: query.data ?? [],
    loading: query.isLoading,
    fetchPicks: query.refetch,
  };
}
