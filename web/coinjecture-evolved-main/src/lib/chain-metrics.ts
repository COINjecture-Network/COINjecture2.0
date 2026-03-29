/**
 * Display helpers aligned with consensus + tokenomics (Rust):
 * - Work score: consensus/src/work_score.rs — bits = log₂(solve/verify) × quality
 * - Block reward: tokenomics/src/rewards.rs — reward = base_constant × (work_score / epoch_average_work)
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

/** `RewardCalculator::new()` in tokenomics/src/rewards.rs */
export const REWARD_BASE_CONSTANT = 10_000_000;

/** Default epoch average when not tuned */
export const DEFAULT_EPOCH_AVG_WORK = 1.0;

/** Same formula as `RewardCalculator::calculate_reward` (truncates to integer Balance). */
export function blockRewardFromWorkScore(workScore: number): bigint {
  if (!Number.isFinite(workScore) || workScore <= 0) return 0n;
  const reward = REWARD_BASE_CONSTANT * (workScore / DEFAULT_EPOCH_AVG_WORK);
  return BigInt(Math.floor(reward));
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

export function formatBeans(n: bigint): string {
  return n.toLocaleString();
}

/** Bits from header — match display precision to typical chain values */
export function formatWorkScoreBits(bits: number): string {
  if (!Number.isFinite(bits)) return "—";
  return bits.toFixed(3);
}
