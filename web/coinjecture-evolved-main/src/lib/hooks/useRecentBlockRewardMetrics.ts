import { useQuery } from "@tanstack/react-query";
import {
  averageAtoms,
  bountyPresetsFromRecentAvg,
  buildEmissionHintCopy,
  coinbaseRewardFromBlock,
  parseBalance,
} from "@/lib/chain-metrics";
import { rpcClient } from "@/lib/rpc-client";

const RECENT_BLOCK_WINDOW = 20;
const REFETCH_MS = 30_000;

export type RecentBlockRewardMetrics = {
  height: number | null;
  windowSize: number;
  sampleCount: number;
  recentAvgAtoms: bigint | null;
  latestRewardAtoms: bigint | null;
  lifetimeAvgAtoms: bigint | null;
  bountyPresets: string[];
  hintPrimary: string | null;
  hintSecondary: string | null;
};

async function fetchRecentBlockRewardMetrics(): Promise<RecentBlockRewardMetrics> {
  const chainInfo = await rpcClient.getChainInfo();
  const height = chainInfo.best_height ?? 0;
  const totalMinted = parseBalance(chainInfo.total_minted_rewards);
  const lifetimeAvgAtoms =
    totalMinted != null && height > 0 ? totalMinted / BigInt(height) : null;

  if (height <= 0) {
    const hints = buildEmissionHintCopy({
      recentAvgAtoms: null,
      latestRewardAtoms: null,
      lifetimeAvgAtoms,
      sampleCount: 0,
      height,
    });
    return {
      height,
      windowSize: RECENT_BLOCK_WINDOW,
      sampleCount: 0,
      recentAvgAtoms: null,
      latestRewardAtoms: null,
      lifetimeAvgAtoms,
      bountyPresets: bountyPresetsFromRecentAvg(null),
      hintPrimary: hints.primary,
      hintSecondary: hints.secondary,
    };
  }

  const start = Math.max(1, height - RECENT_BLOCK_WINDOW + 1);
  const heights: number[] = [];
  for (let h = start; h <= height; h++) heights.push(h);

  const blocks = await Promise.all(heights.map((h) => rpcClient.getBlock(h)));
  const rewards = blocks
    .map((b) => (b ? coinbaseRewardFromBlock(b) : null))
    .filter((r): r is bigint => r != null);

  const recentAvgAtoms = averageAtoms(rewards);
  const latestBlock = blocks[blocks.length - 1];
  const latestRewardAtoms = latestBlock ? coinbaseRewardFromBlock(latestBlock) : null;

  const hints = buildEmissionHintCopy({
    recentAvgAtoms,
    latestRewardAtoms,
    lifetimeAvgAtoms,
    sampleCount: rewards.length,
    height,
  });

  return {
    height,
    windowSize: RECENT_BLOCK_WINDOW,
    sampleCount: rewards.length,
    recentAvgAtoms,
    latestRewardAtoms,
    lifetimeAvgAtoms,
    bountyPresets: bountyPresetsFromRecentAvg(recentAvgAtoms),
    hintPrimary: hints.primary,
    hintSecondary: hints.secondary,
  };
}

export function useRecentBlockRewardMetrics() {
  return useQuery({
    queryKey: ["recent-block-reward-metrics", RECENT_BLOCK_WINDOW],
    queryFn: fetchRecentBlockRewardMetrics,
    staleTime: REFETCH_MS,
    refetchInterval: REFETCH_MS,
  });
}
