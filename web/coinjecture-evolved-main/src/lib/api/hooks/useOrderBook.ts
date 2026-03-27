import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '../client';

export function useOrderBook(pairId: string | null) {
  return useQuery({
    queryKey: ['orderBook', pairId],
    queryFn: () => apiFetch(`/marketplace/orders?pair_id=${pairId}&status=open`),
    enabled: !!pairId,
    staleTime: 5_000,
    refetchInterval: 10_000,
  });
}
