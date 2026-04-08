import { useMutation } from '@tanstack/react-query';
import { draftApi } from '../api/draft-api';

export function useFinalizeDraft() {
  const mutation = useMutation({
    mutationFn: (draftSessionId: string) => draftApi.finalizeDraft(draftSessionId),
  });

  const finalizeDraft = async (draftSessionId: string, _leagueId: string) => {
    await mutation.mutateAsync(draftSessionId);
    return 0; // Backend handles the count; return 0 for compatibility
  };

  return {
    finalizeDraft,
    finalizing: mutation.isPending,
  };
}
