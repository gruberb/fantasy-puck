import { useQuery } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import type { PlayerPoolEntry } from '../types';

export const playerPoolQueryKey = (draftSessionId: string | null) =>
  ['draft', 'playerPool', draftSessionId] as const;

export function usePlayerPool(draftSessionId: string | null) {
  const query = useQuery({
    queryKey: playerPoolQueryKey(draftSessionId),
    queryFn: async () => {
      const data = await draftApi.getDraftState(draftSessionId!);
      return (data?.playerPool ?? []) as PlayerPoolEntry[];
    },
    enabled: !!draftSessionId,
  });

  return {
    players: query.data ?? [],
    loading: query.isLoading,
    fetchPlayers: query.refetch,
  };
}
