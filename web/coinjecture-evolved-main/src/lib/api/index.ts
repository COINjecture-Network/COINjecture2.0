// API client — re-exports for convenient imports
//
//   import { useLatestBlock, useChainInfo, apiFetch } from '@/lib/api';

export { apiFetch, API_BASE } from './client';
export { createSSEConnection } from './sse';
export { useLatestBlock } from './hooks/useBlocks';
export { useMempool } from './hooks/useMempool';
export { useOrderBook } from './hooks/useOrderBook';
export { useRecentTrades } from './hooks/useTrades';
export { useOpenTasks } from './hooks/useTasks';
export { useChainInfo } from './hooks/useChainInfo';
export type { BlockEvent, MempoolEvent, ChainInfo, TradingPair, Trade, PouwTask } from './types';
