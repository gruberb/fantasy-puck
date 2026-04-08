import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import { realtimeService } from '@/lib/realtime';
import type { DraftSession } from '../types';

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

  // Subscribe to realtime updates via WebSocket
  useEffect(() => {
    if (!session?.id) return;

    const unsubscribe = realtimeService.subscribeToDraft(session.id, {
      onSessionUpdated: (partial) => {
        queryClient.setQueryData<DraftSession | null>(
          draftSessionQueryKey(leagueId),
          (prev) => {
            if (!prev) return prev;
            return { ...prev, ...partial };
          },
        );
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
