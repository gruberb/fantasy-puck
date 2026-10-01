import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/api/client';

export const leagueKeys = {
  all: ['leagues'] as const,
  // Anonymous visitors get the public-only listing, so the viewer is part of the key.
  list: (userId: string | null | undefined) => ['leagues', userId ?? 'public'] as const,
};

export const membershipKeys = {
  all: ['memberships'] as const,
  forUser: (userId: string | undefined) => ['memberships', userId] as const,
};

export function useLeagues(ownerId?: string | null, isSuperAdmin?: boolean) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: leagueKeys.list(ownerId),
    queryFn: () => api.getLeagues(!ownerId),
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
      api.createLeague(args.name, args.season),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: leagueKeys.all });
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
