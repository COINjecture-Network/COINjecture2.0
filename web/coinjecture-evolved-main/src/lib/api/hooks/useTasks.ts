import { useQuery } from '@tanstack/react-query';
import { apiFetch } from '../client';
import type { PouwTask } from '../types';

export function useOpenTasks(classFilter?: string) {
  const params = classFilter ? `?class=${classFilter}` : '';
  return useQuery<PouwTask[]>({
    queryKey: ['tasks', classFilter],
    queryFn: () => apiFetch(`/marketplace/tasks${params}`),
    staleTime: 30_000,
  });
}
