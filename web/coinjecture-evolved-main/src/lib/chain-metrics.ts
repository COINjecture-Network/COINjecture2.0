/**
 * Display helpers aligned with consensus + tokenomics (Rust):
 * - Work score: consensus/src/work_score.rs — bits = log₂(solve/verify) × quality
 * - Block reward: tokenomics/src/rewards.rs — `⌊base_reward / W⌋` with `W` = parent-chain cumulative work
 */

/** Match `consensus/src/work_score.rs` — same floors as the f64 `calculate` path. */
const MIN_VERIFY_TIME_US = 1;
const MIN_ASYMMETRY_RATIO = 2;

/**
 * Bit-equivalent work score (header `work_score`), matching `WorkScoreCalculator::calculate`.
 */
export function workScoreBitsFromPouw(
  solveTimeUs: number,
  verifyTimeUs: number,
  quality: number
): number {
  if (!Number.isFinite(quality) || quality <= 0) return 0;
  const q = Math.min(1, Math.max(0, quality));
  const solveUs = Math.max(0, Math.floor(solveTimeUs));
  const verifyUs = Math.max(MIN_VERIFY_TIME_US, Math.floor(verifyTimeUs));
  if (solveUs < verifyUs * MIN_ASYMMETRY_RATIO) return 0;
  const ratio = solveUs / verifyUs;
  return Math.log2(ratio) * q;
}

/** `RewardCalculator::new().base_reward` in tokenomics/src/rewards.rs */
export const REWARD_BASE_REWARD = 10_000_000n;

/**
 * Same as Rust `RewardCalculator::calculate_block_reward`: `⌊base_reward / W⌋` for `W > 0`, else `0`.
 */
export function blockRewardFromParentCumulativeWork(parentCumulativeWork: bigint): bigint {
  if (parentCumulativeWork <= 0n) return 0n;
  return REWARD_BASE_REWARD / parentCumulativeWork;
}

export function parseBalance(raw: unknown): bigint | null {
  if (raw === null || raw === undefined) return null;
  if (typeof raw === "bigint") return raw;
  if (typeof raw === "number" && Number.isFinite(raw)) return BigInt(Math.trunc(raw));
  if (typeof raw === "string") {
    const s = raw.trim();
    if (/^\d+$/.test(s)) return BigInt(s);
  }
  return null;
}

/** Decimal string `u128` from RPC/API (digits only). */
export function parseU128DecimalString(raw: unknown): bigint | null {
  return parseBalance(raw);
}

export function formatBeans(n: bigint): string {
  return n.toLocaleString();
}

/** Bits from header — match display precision to typical chain values */
export function formatWorkScoreBits(bits: number): string {
  if (!Number.isFinite(bits)) return "—";
  return bits.toFixed(3);
}
