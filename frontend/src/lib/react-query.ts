import { DefaultOptions, QueryClient } from '@tanstack/react-query';
import { QUERY_INTERVALS } from '@/config';

const queryConfig: DefaultOptions = {
  queries: {
    refetchOnWindowFocus: false,
    retry: false,
    staleTime: QUERY_INTERVALS.DEFAULT_STALE_MS,
  },
};

export const queryClient = new QueryClient({ defaultOptions: queryConfig });

// Utility type: extract query options from a query-options factory, omitting queryKey/queryFn
export type QueryConfig<T extends (...args: any[]) => any> = Omit<
  ReturnType<T>,
  'queryKey' | 'queryFn'
>;
