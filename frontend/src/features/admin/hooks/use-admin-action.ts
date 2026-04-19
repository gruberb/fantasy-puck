import { useState } from "react";

/**
 * Shared state holder for an admin action. We roll our own instead of
 * `useMutation` because these calls are one-shot, admin-only, and we
 * want uniform `ranAt` timing + error shape without touching the React
 * Query cache.
 */
export function useAdminAction<TArgs extends unknown[], TResult>(
  fn: (...args: TArgs) => Promise<TResult>,
) {
  const [data, setData] = useState<TResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, setPending] = useState(false);
  const [ranAt, setRanAt] = useState<Date | null>(null);

  const run = async (...args: TArgs) => {
    setPending(true);
    setError(null);
    try {
      const result = await fn(...args);
      setData(result);
      setRanAt(new Date());
      return result;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      setData(null);
      setRanAt(new Date());
      throw e;
    } finally {
      setPending(false);
    }
  };

  const reset = () => {
    setData(null);
    setError(null);
    setRanAt(null);
  };

  return { run, data, error, isPending, ranAt, reset };
}
