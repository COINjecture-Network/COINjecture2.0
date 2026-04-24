/**
 * Header JSON hash + PoW search — shared by main thread (`mining.ts`) and `mining-pow.worker.ts`.
 * Kept separate so the worker can run a tight loop without blocking the UI.
 */

import { blake3 } from "@noble/hashes/blake3";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import type { Block } from "./rpc-client";

/** Leading hex zeroes on `hex(headerHash)` (node rule). */
export const DEFAULT_POW_DIFFICULTY = 2;
export const MAX_LEADING_HEX_ZEROS = 64;

const MINING_DEBUG_FLAG_KEY = "coinjecture:mining-debug";

function shouldLogMiningDebug(): boolean {
  try {
    if (import.meta.env.DEV) return true;
  } catch {
    /* non-Vite */
  }
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage?.getItem(MINING_DEBUG_FLAG_KEY) === "true";
  } catch {
    return false;
  }
}

function blake3ToHex(data: Uint8Array | string): string {
  const bytes = typeof data === "string" ? hexToBytes(data) : data;
  return bytesToHex(blake3(bytes));
}

/** Expected ~16^n trials for n leading hex zeroes; cap to avoid unbounded search. */
export function maxNonceForDifficulty(difficulty: number): number {
  const n = Math.min(Math.max(0, difficulty), MAX_LEADING_HEX_ZEROS);
  if (n === 0) return 1;
  const expected = Math.pow(16, Math.min(n, 10));
  return Math.min(400_000_000, Math.max(12_000_000, Math.floor(expected * 80)));
}

/**
 * Same as `mining.ts` / Rust `hash_from_json` — field order and float encoding must match consensus.
 */
export function buildHeaderHashDebug(header: Block["header"]): { hash: string; json: string } {
  const headerForHash: Record<string, unknown> = {
    version: header.version,
    height: header.height,
    prev_hash:
      typeof header.prev_hash === "string" ? Array.from(hexToBytes(header.prev_hash)) : header.prev_hash,
    timestamp: header.timestamp,
    transactions_root:
      typeof header.transactions_root === "string"
        ? Array.from(hexToBytes(header.transactions_root))
        : header.transactions_root,
    solutions_root:
      typeof header.solutions_root === "string"
        ? Array.from(hexToBytes(header.solutions_root))
        : header.solutions_root,
    commitment: {
      hash:
        typeof header.commitment.hash === "string"
          ? Array.from(hexToBytes(header.commitment.hash))
          : header.commitment.hash,
      problem_hash:
        typeof header.commitment.problem_hash === "string"
          ? Array.from(hexToBytes(header.commitment.problem_hash))
          : header.commitment.problem_hash,
    },
    work_score: header.work_score,
    miner: typeof header.miner === "string" ? Array.from(hexToBytes(header.miner)) : header.miner,
    nonce: header.nonce,
    solve_time_us: header.solve_time_us,
    verify_time_us: header.verify_time_us,
    time_asymmetry_ratio: header.time_asymmetry_ratio,
    solution_quality: header.solution_quality,
    complexity_weight: header.complexity_weight,
    energy_estimate_joules: header.energy_estimate_joules,
  };

  const floatFieldNames = new Set([
    "work_score",
    "time_asymmetry_ratio",
    "solution_quality",
    "complexity_weight",
    "energy_estimate_joules",
  ]);

  const serializeRustLikeJson = (value: unknown, parentKey?: string): string => {
    if (value === null) return "null";
    if (typeof value === "number") {
      if (!Number.isFinite(value)) {
        throw new Error(`Cannot serialize non-finite number for ${parentKey ?? "value"}`);
      }
      if (parentKey && floatFieldNames.has(parentKey) && Number.isInteger(value)) {
        return `${value.toFixed(1)}`;
      }
      return JSON.stringify(value);
    }
    if (typeof value === "boolean") return value ? "true" : "false";
    if (typeof value === "string") return JSON.stringify(value);
    if (Array.isArray(value)) {
      return `[${value.map((item) => serializeRustLikeJson(item)).join(",")}]`;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value as Record<string, unknown>);
      return `{${entries
        .map(([key, item]) => `${JSON.stringify(key)}:${serializeRustLikeJson(item, key)}`)
        .join(",")}}`;
    }
    throw new Error(`Unsupported value in header serialization: ${String(value)}`);
  };

  const headerJson = serializeRustLikeJson(headerForHash);
  const headerBytes = new TextEncoder().encode(headerJson);
  const calculatedHash = blake3ToHex(headerBytes);

  if (shouldLogMiningDebug()) {
    console.log("Client header JSON (hashed payload):", headerJson.slice(0, 400));
    console.log("Client header hash:", calculatedHash);
  }

  return { hash: calculatedHash, json: headerJson };
}

export function calculateHeaderHash(header: Block["header"]): string {
  return buildHeaderHashDebug(header).hash;
}

/**
 * Synchronous PoW loop — run inside a Web Worker, not on the main thread.
 */
export function mineHeaderPowSync(
  header: Block["header"],
  difficulty: number = DEFAULT_POW_DIFFICULTY,
  onProgress?: (nonce: number, hash: string) => void
): { nonce: number; hash: string } | null {
  const n = Math.min(Math.max(0, difficulty), MAX_LEADING_HEX_ZEROS);
  const targetPrefix = "0".repeat(n);
  const maxNonce = maxNonceForDifficulty(n);
  let lastProgressWall = 0;

  for (let nonce = 0; nonce < maxNonce; nonce++) {
    header.nonce = nonce;
    const hash = calculateHeaderHash(header);
    if (hash.startsWith(targetPrefix)) {
      return { nonce, hash };
    }
    if (onProgress && nonce > 0) {
      const now = performance.now();
      if (nonce % 200_000 === 0 || now - lastProgressWall >= 400) {
        onProgress(nonce, hash);
        lastProgressWall = now;
      }
    }
  }
  return null;
}
