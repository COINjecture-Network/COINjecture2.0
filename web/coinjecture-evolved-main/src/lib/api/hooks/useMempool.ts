import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { createSSEConnection } from '../sse';
import type { MempoolEvent } from '../types';

export function useMempool() {
  const queryClient = useQueryClient();

  const query = useQuery<MempoolEvent | null>({
    queryKey: ['mempool'],
    queryFn: () => Promise.resolve(null), // Initial data comes from SSE
    staleTime: Infinity,
  });

  useEffect(() => {
    const sse = createSSEConnection('/events/mempool', {
      onEvent: (event, data) => {
        if (event === 'mempool') {
          queryClient.setQueryData(['mempool'], data);
        }
      },
    });
    return () => sse.close();
  }, [queryClient]);

  return query;
}
