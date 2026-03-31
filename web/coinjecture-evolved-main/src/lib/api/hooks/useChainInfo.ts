import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '../client';
import type { ChainInfo } from '../types';

export function useChainInfo() {
  return useQuery<ChainInfo>({
    queryKey: ['chainInfo'],
    queryFn: () => apiFetch('/chain/info'),
    staleTime: 30_000,
    refetchInterval: 30_000,
  });
}
