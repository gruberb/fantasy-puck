import { useQuery } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import type { DraftPick } from '../types';

export const draftPicksQueryKey = (draftSessionId: string | null) =>
  ['draft', 'picks', draftSessionId] as const;

export function useDraftPicks(draftSessionId: string | null, _leagueId?: string | null) {
  const query = useQuery({
    queryKey: draftPicksQueryKey(draftSessionId),
    queryFn: async () => {
      const data = await draftApi.getDraftState(draftSessionId!);
      return (data?.picks ?? []) as DraftPick[];
    },
    enabled: !!draftSessionId,
  });

  // WebSocket subscription is handled centrally by useDraftSession

  return {
    picks: query.data ?? [],
    loading: query.isLoading,
    fetchPicks: query.refetch,
  };
}
