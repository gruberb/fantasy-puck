import { useMemo, useCallback } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';
import { draftSessionQueryKey } from './use-draft-session';
import { getPickerForPick } from '../types';
import type { DraftSession, LeagueMember, PlayerPoolEntry } from '../types';

export interface SleeperPick {
  id: number;
  teamId: number;
  nhlId: number;
  name: string;
  position: string;
  nhlTeam: string;
}

export const eligibleSleepersQueryKey = (draftSessionId: string | null) =>
  ['draft', 'eligibleSleepers', draftSessionId] as const;

export const sleeperPicksQueryKey = (draftSessionId: string | null) =>
  ['draft', 'sleeperPicks', draftSessionId] as const;

export function useSleeperRound(
  session: DraftSession | null,
  leagueId: string | null,
  members: LeagueMember[],
) {
  const queryClient = useQueryClient();
  const draftSessionId = session?.id ?? null;
  const sleeperActive = session?.sleeperStatus === 'active';
  const sleeperComplete = session?.sleeperStatus === 'completed';

  // Players eligible for sleeper picks
  const eligibleQuery = useQuery({
    queryKey: eligibleSleepersQueryKey(draftSessionId),
    queryFn: () => draftApi.getEligibleSleepers(draftSessionId!),
    enabled: !!draftSessionId && !!leagueId && (sleeperActive || sleeperComplete),
  });

  // Sleeper picks from backend — always refetch when enabled changes
  const sleeperPicksQuery = useQuery({
    queryKey: sleeperPicksQueryKey(draftSessionId),
    queryFn: () => draftApi.getSleeperPicks(draftSessionId!),
    enabled: !!draftSessionId && (sleeperActive || sleeperComplete),
    staleTime: 0,
  });

  const invalidateAll = useCallback(() => {
    if (draftSessionId) {
      queryClient.invalidateQueries({ queryKey: eligibleSleepersQueryKey(draftSessionId) });
      queryClient.invalidateQueries({ queryKey: sleeperPicksQueryKey(draftSessionId) });
    }
    // Also invalidate the session so sleeperPickIndex/sleeperStatus update
    if (leagueId) {
      queryClient.invalidateQueries({ queryKey: draftSessionQueryKey(leagueId) });
    }
  }, [draftSessionId, leagueId, queryClient]);

  // WebSocket subscription is handled centrally by useDraftSession

  const startSleeperMutation = useMutation({
    mutationFn: (sessionId: string) => draftApi.startSleeperRound(sessionId),
  });

  const makeSleeperPickMutation = useMutation({
    mutationFn: (args: { sessionId: string; playerPoolId: string; teamId: number }) =>
      draftApi.makeSleeperPick(args.sessionId, args.playerPoolId, args.teamId),
    onSuccess: () => {
      invalidateAll();
    },
  });

  const startSleeperRound = async (sessionId: string) => {
    await startSleeperMutation.mutateAsync(sessionId);
  };

  const makeSleeperPick = async (
    sessionId: string,
    teamId: number,
    player: PlayerPoolEntry,
    _currentPickIndex: number,
    _memberCount: number,
  ) => {
    await makeSleeperPickMutation.mutateAsync({
      sessionId,
      playerPoolId: player.id,
      teamId,
    });
  };

  // Determine whose turn it is in the sleeper round
  const sleeperPicker = useMemo(() => {
    if (!session || session.sleeperStatus !== 'active' || members.length === 0) return undefined;
    const sorted = [...members].sort((a, b) => (a.draftOrder ?? 0) - (b.draftOrder ?? 0));
    return getPickerForPick(session.sleeperPickIndex, sorted, session.snakeDraft);
  }, [session, members]);

  return {
    eligiblePlayers: (eligibleQuery.data ?? []) as PlayerPoolEntry[],
    eligibleLoading: eligibleQuery.isLoading,
    sleeperPicks: (sleeperPicksQuery.data ?? []) as SleeperPick[],
    sleeperPicker,
    sleeperPicking: makeSleeperPickMutation.isPending,
    startSleeperRound,
    makeSleeperPick,
    fetchEligible: invalidateAll,
  };
}
