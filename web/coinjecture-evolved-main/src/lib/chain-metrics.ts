/**
 * Display helpers aligned with consensus + tokenomics (Rust):
 * - Work score: consensus/src/work_score.rs — bits = log₂(solve/verify) × quality
 * - Block reward: tokenomics/src/rewards.rs — `⌊w_trunc / W_parent⌋` (same trunc as chain cumulative W).
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

/**
 * Same as Rust `RewardCalculator::calculate_block_reward(work_score, W)`:
 * `⌊w_trunc / W_parent⌋` for `W_parent > 0`, else `0`.
 */
export function blockRewardFromTruncWorkAndParentW(
  blockWorkTrunc: bigint,
  parentCumulativeWork: bigint
): bigint {
  if (parentCumulativeWork <= 0n) return 0n;
  return blockWorkTrunc / parentCumulativeWork;
}

/**
 * Work units summed into chain cumulative W (`node/src/chain.rs`):
 * `(header.work_score.max(0.0) as u64) as u128` — uses the **stored** header field only
 * (not the PoUW recompute used for display bits).
 */
export function truncatedHeaderWorkScoreU128(workScore: unknown): bigint {
  if (workScore == null) return 0n;
  const x =
    typeof workScore === 'number'
      ? workScore
      : typeof workScore === 'string' && workScore.trim() !== ''
        ? Number(workScore)
        : NaN;
  if (!Number.isFinite(x) || x <= 0) return 0n;
  const t = Math.trunc(x);
  if (t <= 0) return 0n;
  if (t > Number.MAX_SAFE_INTEGER) {
    try {
      return BigInt(Math.floor(x));
    } catch {
      return 0n;
    }
  }
  return BigInt(t);
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
