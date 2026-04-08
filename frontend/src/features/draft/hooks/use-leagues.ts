import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { fetchApi } from '@/lib/api-client';
import type { League } from '@/types/league';

const leaguesQueryKey = ['leagues'] as const;

export function useLeagues(ownerId?: string | null, isSuperAdmin?: boolean) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: leaguesQueryKey,
    queryFn: () => fetchApi<League[]>('leagues', { fallback: [] }),
  });

  // Filter client-side if not super admin
  const leagues = (() => {
    const data = query.data ?? [];
    if (ownerId && !isSuperAdmin) {
      return data.filter((l) => l.created_by === ownerId);
    }
    return data;
  })();

  const createLeagueMutation = useMutation({
    mutationFn: (args: { name: string; season: string }) =>
      fetchApi<League>('leagues', { method: 'POST', body: { name: args.name, season: args.season } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: leaguesQueryKey });
    },
  });

  const createLeague = async (name: string, season: string, _userId: string) => {
    return createLeagueMutation.mutateAsync({ name, season });
  };

  return {
    leagues,
    loading: query.isLoading,
    fetchLeagues: query.refetch,
    createLeague,
  };
}
