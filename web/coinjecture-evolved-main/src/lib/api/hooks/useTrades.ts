import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '../client';
import type { Trade } from '../types';

export function useRecentTrades(pairId: string | null, limit = 50) {
  return useQuery<Trade[]>({
    queryKey: ['trades', pairId, limit],
    queryFn: () => apiFetch(`/marketplace/trades?pair_id=${pairId}&limit=${limit}`),
    enabled: !!pairId,
    staleTime: 5_000,
  });
}
