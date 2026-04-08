import { useMutation, useQueryClient } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import { draftSessionQueryKey } from './use-draft-session';
import { draftPicksQueryKey } from './use-draft-picks';
import type { PlayerPoolEntry } from '../types';

export function useMakePick() {
  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: (args: { draftSessionId: string; playerPoolId: string; leagueId: string }) =>
      draftApi.makePick(args.draftSessionId, args.playerPoolId),
    onSuccess: (_data, variables) => {
      // Invalidate session and picks so they refetch with updated state
      queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(variables.leagueId) });
      queryClient.invalidateQueries({ queryKey: draftPicksQueryKey(variables.draftSessionId) });
    },
  });

  const makePick = async (
    draftSessionId: string,
    _leagueMemberId: string,
    player: PlayerPoolEntry,
    _currentPickIndex: number,
    _memberCount: number,
    leagueId?: string,
  ) => {
    await mutation.mutateAsync({ draftSessionId, playerPoolId: player.id, leagueId: leagueId ?? '' });
  };

  return {
    makePick,
    picking: mutation.isPending,
  };
}
