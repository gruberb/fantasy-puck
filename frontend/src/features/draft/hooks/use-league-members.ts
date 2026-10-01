import { useQuery } from '@tanstack/react-query';
import { draftApi } from '@/features/draft/api/draft-api';
import type { LeagueMember } from '@/features/draft/types';

export const leagueMembersQueryKey = (leagueId: string | null) =>
  ['draft', 'leagueMembers', leagueId] as const;

export function useLeagueMembers(leagueId: string | null) {
  const query = useQuery({
    queryKey: leagueMembersQueryKey(leagueId),
    queryFn: () => draftApi.getLeagueMembers(leagueId!),
    enabled: !!leagueId,
  });

  return {
    members: (query.data ?? []) as LeagueMember[],
    loading: query.isLoading,
    fetchMembers: query.refetch,
  };
}
