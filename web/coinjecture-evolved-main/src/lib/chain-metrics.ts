/**
 * Display helpers aligned with consensus + tokenomics (Rust):
 * - Work score: consensus/src/work_score.rs — bits = log₂(solve/verify) × quality
 * - Block reward: `tokenomics/src/rewards.rs` — minted **atoms** = `⌊w_trunc·S·K / W_parent⌋`
 *   with `S = REWARD_FIXED_POINT_SCALE`, `K = REWARD_EMISSION_MULTIPLIER`; one **display BEANS** = S atoms.
 */

/** Must match `coinject_tokenomics::REWARD_FIXED_POINT_SCALE` (10^12). */
export const REWARD_FIXED_POINT_SCALE = 1_000_000_000_000n;

/** Must match `coinject_tokenomics::REWARD_EMISSION_MULTIPLIER`. */
export const REWARD_EMISSION_MULTIPLIER = 50n;

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
 * Same as Rust `RewardCalculator::calculate_block_reward`:
 * `⌊w_trunc·S·K / W_parent⌋` minted **atoms**.
 */
export function blockRewardFromTruncWorkAndParentW(
  blockWorkTrunc: bigint,
  parentCumulativeWork: bigint
): bigint {
  if (parentCumulativeWork <= 0n) return 0n;
  return (blockWorkTrunc * REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER) / parentCumulativeWork;
}

/** Convert whole display BEANS to ledger atoms. */
export function displayBeansToAtoms(displayBeans: bigint): bigint {
  return displayBeans * REWARD_FIXED_POINT_SCALE;
}

/**
 * Work units summed into chain cumulative W (`node/src/chain.rs`):
 * `(header.work_score.max(0.0) as u64) as u128` — uses the **stored** header field only
 * (not the PoUW recompute used for display bits).
 */
export function truncatedHeaderWorkScoreU128(workScore: unknown): bigint {
  if (workScore == null) return 0n;
  const x =
    typeof workScore === "number"
      ? workScore
      : typeof workScore === "string" && workScore.trim() !== ''
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

/** Format ledger **atoms** as display BEANS (trim fractional zeros). */
export function formatBeans(atoms: bigint): string {
  const s = REWARD_FIXED_POINT_SCALE;
  if (atoms === 0n) return "0";
  const neg = atoms < 0n;
  const a = neg ? -atoms : atoms;
  const whole = a / s;
  const frac = a % s;
  if (frac === 0n) {
    return (neg ? "-" : "") + whole.toLocaleString(undefined, { maximumFractionDigits: 0 });
  }
  const fracStr = frac
    .toString()
    .padStart(12, "0")
    .replace(/0+$/, "");
  return (neg ? "-" : "") + whole.toLocaleString(undefined, { maximumFractionDigits: 0 }) + "." + fracStr;
}

/** Bits from header — match display precision to typical chain values */
export function formatWorkScoreBits(bits: number): string {
  if (!Number.isFinite(bits)) return "—";
  return bits.toFixed(3);
}
