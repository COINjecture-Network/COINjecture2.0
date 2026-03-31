import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { apiFetch } from '../client';
import { createSSEConnection } from '../sse';
import type { BlockEvent } from '../types';

export function useLatestBlock() {
  const queryClient = useQueryClient();

  const query = useQuery<BlockEvent | null>({
    queryKey: ['latestBlock'],
    queryFn: () => apiFetch<BlockEvent | null>('/chain/latest-block').catch(() => null),
    staleTime: 10_000,
  });

  // SSE subscription — push updates into the query cache
  useEffect(() => {
    const sse = createSSEConnection('/events/blocks', {
      onEvent: (event, data) => {
        if (event === 'block') {
          queryClient.setQueryData(['latestBlock'], data);
          queryClient.invalidateQueries({ queryKey: ['trades'] });
          queryClient.invalidateQueries({ queryKey: ['orderBook'] });
        }
      },
    });
    return () => sse.close();
  }, [queryClient]);

  return query;
}
