/**
 * Client-Side Mining for COINjecture Network B
 * Implements deterministic problem generation, solving, commitment creation, and block mining
 */

import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, hexToBytes } from '@noble/hashes/utils';
import {
  blockRewardFromTruncWorkAndParentW,
  truncatedHeaderWorkScoreU128,
  workScoreBitsFromPouw,
} from './chain-metrics';
import { ProblemType, SolutionType, Block, BlockHeader } from './rpc-client';
import MiningPowWorker from './mining-pow.worker?worker';
import {
  buildHeaderHashDebug,
  calculateHeaderHash,
  maxNonceForDifficulty,
  MAX_LEADING_HEX_ZEROS,
} from './mining-pow-core';

// Types matching Rust implementation
export interface Commitment {
  hash: string;
  problem_hash: string;
}

export interface SolutionReveal {
  problem: ProblemType;
  solution: SolutionType;
  commitment: Commitment;
}

export interface Solution {
  SubsetSum?: number[];
  SAT?: boolean[];
  TSP?: number[];
  Custom?: string;
}

// Constants from Rust implementation
const TAU_C = Math.SQRT2; // √2 ≈ 1.414
const MIN_PROBLEM_SIZE = 5;
const MAX_SUBSET_SUM_SIZE = 50;
const MAX_SAT_VARIABLES = 30;
const MAX_TSP_CITIES = 10;

/** Leading **hex** zeroes required on `hex(headerHash)` — not Bitcoin difficulty bits / nBits. */
const DEFAULT_DIFFICULTY = 2;

/** Verify passes averaged for header `verify_time_us` — reduces 1 ms timer quantisation in Workers. */
const N_VERIFY_PASSES = 100;

/** Wall-clock cap for in-browser PoW (worker); after this we give up so the UI does not hang forever. */
const MAX_POW_WORKER_MS = 25 * 60 * 1000;
/** Main-thread fallback: max ms per tight slice before yielding (no Worker). */
const MAX_POW_SLICE_MS = 12;

function normalizeHeaderFloat(value: number, decimals: number = 12): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Number(value.toFixed(decimals));
}

/** Coinbase `reward` as JSON — decimal string for full `u128` atom amounts. */
function rewardAsJson(reward: bigint): string {
  if (reward < 0n) return "0";
  return reward.toString();
}

/**
 * Seeded RNG for deterministic problem generation
 * Simple LCG (Linear Congruential Generator) for compatibility
 */
class SeededRNG {
  private seed: number;

  constructor(seed: number) {
    this.seed = seed;
  }

  // Generate next random number (0 to 1)
  next(): number {
    // LCG: (a * seed + c) mod m
    // Using constants from Numerical Recipes
    this.seed = (this.seed * 1664525 + 1013904223) >>> 0;
    return (this.seed >>> 0) / 0xFFFFFFFF;
  }

  // Generate random integer in range [min, max)
  nextInt(min: number, max: number): number {
    return Math.floor(this.next() * (max - min)) + min;
  }

  // Generate random boolean
  nextBool(probability: number = 0.5): boolean {
    return this.next() < probability;
  }

  // Shuffle array in place
  shuffle<T>(array: T[]): void {
    for (let i = array.length - 1; i > 0; i--) {
      const j = this.nextInt(0, i + 1);
      [array[i], array[j]] = [array[j], array[i]];
    }
  }
}

/**
 * Extract hex string from hash (handles string, object, and byte array formats)
 */
export function extractHashHex(hash: string | { hash?: string; bytes?: string } | number[] | Uint8Array | any): string {
  // If it's already a string, return it
  if (typeof hash === 'string') {
    return hash;
  }
  
  // If it's a byte array (number[] or Uint8Array), convert to hex
  if (Array.isArray(hash)) {
    return bytesToHex(new Uint8Array(hash));
  }
  if (hash instanceof Uint8Array) {
    return bytesToHex(hash);
  }
  
  // If it's an object, try to extract the hash
  if (hash && typeof hash === 'object') {
    // Check for common hash properties
    if (hash.hash && typeof hash.hash === 'string') {
      return hash.hash;
    }
    if (hash.bytes && typeof hash.bytes === 'string') {
      return hash.bytes;
    }
    // Check if it has a bytes array property
    if (Array.isArray(hash.bytes)) {
      return bytesToHex(new Uint8Array(hash.bytes));
    }
    // Try to find any string property that looks like a hex hash
    for (const key in hash) {
      if (typeof hash[key] === 'string' && hash[key].length === 64) {
        return hash[key];
      }
      // Check for byte arrays in object properties
      if (Array.isArray(hash[key]) && hash[key].length === 32) {
        return bytesToHex(new Uint8Array(hash[key]));
      }
    }
  }
  
  throw new Error(`Invalid hash format: ${JSON.stringify(hash)}`);
}

/**
 * Create deterministic seed from prev_hash and height
 */
function createSeed(prevHash: string | { hash?: string; bytes?: string } | any, height: number): number {
  const hashHex = extractHashHex(prevHash);
  const hashBytes = hexToBytes(hashHex);
  let seed = 0;
  
  // XOR height into first 8 bytes
  for (let i = 0; i < 8; i++) {
    const byte = hashBytes[i] || 0;
    const heightByte = (height >> (i * 8)) & 0xFF;
    seed ^= (byte ^ heightByte) << (i * 8);
  }
  
  return seed >>> 0; // Ensure unsigned 32-bit
}

/**
 * Generate problem deterministically from prev_hash and height
 */
export function generateProblem(
  prevHash: string,
  height: number,
  problemSize: number = 10
): ProblemType {
  const seed = createSeed(prevHash, height);
  const rng = new SeededRNG(seed);
  
  // Calculate tau for consensus state (not used in generation but kept for compatibility)
  const tau = height / TAU_C;
  
  // Choose problem type (0=SubsetSum, 1=SAT, 2=TSP)
  const problemType = rng.nextInt(0, 3);
  
  switch (problemType) {
    case 0: {
      // Subset Sum - Generate solvable problem
      const size = Math.min(problemSize, MAX_SUBSET_SUM_SIZE);
      const numbers: number[] = [];
      for (let i = 0; i < size; i++) {
        numbers.push(rng.nextInt(1, 1000));
      }
      
      // Select random subset for solution
      const subsetSize = rng.nextInt(1, size);
      const indices = Array.from({ length: size }, (_, i) => i);
      rng.shuffle(indices);
      const selectedIndices = indices.slice(0, subsetSize);
      
      // Calculate target as sum of selected numbers
      const target = selectedIndices.reduce((sum, idx) => sum + numbers[idx], 0);
      
      return {
        SubsetSum: { numbers, target }
      };
    }
    
    case 1: {
      // SAT - Generate satisfiable problem
      const variables = Math.min(Math.max(problemSize, MIN_PROBLEM_SIZE), MAX_SAT_VARIABLES);
      const numClauses = variables * 3;
      
      // Generate random satisfying assignment
      const satisfyingAssignment: boolean[] = [];
      for (let i = 0; i < variables; i++) {
        satisfyingAssignment.push(rng.nextBool(0.5));
      }
      
      const clauses: Array<{ literals: number[] }> = [];
      for (let i = 0; i < numClauses; i++) {
        // Select 3 distinct variables
        const varIndices = Array.from({ length: variables }, (_, i) => i);
        rng.shuffle(varIndices);
        const selectedVars = varIndices.slice(0, 3);
        
        const literals: number[] = selectedVars.map(varIdx => {
          const varNum = varIdx + 1; // Variables are 1-indexed
          const assignmentValue = satisfyingAssignment[varIdx];
          
          // 70% chance: literal matches assignment (satisfied)
          if (rng.nextBool(0.7)) {
            return assignmentValue ? varNum : -varNum;
          } else {
            return assignmentValue ? -varNum : varNum;
          }
        });
        
        clauses.push({ literals });
      }
      
      return {
        SAT: { variables, clauses }
      };
    }
    
    default: {
      // TSP
      const cities = Math.min(Math.max(problemSize, MIN_PROBLEM_SIZE), MAX_TSP_CITIES);
      const distances: number[][] = [];
      
      for (let i = 0; i < cities; i++) {
        distances[i] = [];
        for (let j = 0; j < cities; j++) {
          if (i === j) {
            distances[i][j] = 0;
          } else if (j < i) {
            distances[i][j] = distances[j][i]; // Symmetric
          } else {
            distances[i][j] = rng.nextInt(1, 100);
          }
        }
      }
      
      return {
        TSP: { cities, distances }
      };
    }
  }
}

/**
 * Solve Subset Sum using dynamic programming (nonnegative integers).
 * Matches on-chain / RPC verification: subset indices must sum exactly to `target`.
 */
export function solveSubsetSum(numbers: number[], target: number): number[] | null {
  const n = numbers.length;
  if (n === 0) {
    return target === 0 ? [] : null;
  }
  if (!numbers.every((x) => Number.isInteger(x) && x >= 0)) {
    return null;
  }
  const sum = numbers.reduce((a, b) => a + b, 0);
  if (target < 0 || target > sum || !Number.isInteger(target)) {
    return null;
  }

  const dp: boolean[][] = Array.from({ length: n + 1 }, () => new Array(sum + 1).fill(false));
  dp[0][0] = true;
  for (let i = 1; i <= n; i++) {
    const w = numbers[i - 1];
    for (let s = 0; s <= sum; s++) {
      dp[i][s] = dp[i - 1][s];
      if (s >= w) {
        dp[i][s] = dp[i][s] || dp[i - 1][s - w];
      }
    }
  }

  if (!dp[n][target]) {
    return null;
  }

  const indices: number[] = [];
  let s = target;
  for (let i = n; i >= 1; i--) {
    const w = numbers[i - 1];
    if (dp[i - 1][s]) {
      continue;
    }
    if (s >= w && dp[i - 1][s - w]) {
      indices.push(i - 1);
      s -= w;
    }
  }
  indices.reverse();
  return indices;
}

/**
 * Solve SAT using brute force (for small problems). Literals are 1-indexed DIMACS-style (see `core::problem::Clause`).
 */
export function solveSATBruteForce(
  variables: number,
  clauses: Array<{ literals: number[] }>
): boolean[] | null {
  const maxAttempts = Math.min(1 << Math.min(variables, 20), 1000000); // Limit to 1M attempts
  
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const assignment: boolean[] = [];
    for (let i = 0; i < variables; i++) {
      assignment.push(((attempt >> i) & 1) === 1);
    }
    
    // Check if all clauses are satisfied
    const allSatisfied = clauses.every(clause => {
      return clause.literals.some(literal => {
        const varIdx = Math.abs(literal) - 1;
        if (varIdx >= assignment.length) return false;
        const value = assignment[varIdx];
        return literal > 0 ? value : !value;
      });
    });
    
    if (allSatisfied) {
      return assignment;
    }
  }
  
  return null;
}

/**
 * Solve TSP using nearest-neighbor heuristic on a distance matrix (same as client mining).
 */
export function solveTSP(cities: number, distances: number[][]): number[] | null {
  if (cities === 0) return null;
  
  const tour: number[] = [];
  const visited = new Array(cities).fill(false);
  let current = 0;
  
  tour.push(current);
  visited[current] = true;
  
  for (let i = 1; i < cities; i++) {
    let nearest: number | null = null;
    let minDist = Infinity;
    
    for (let next = 0; next < cities; next++) {
      if (!visited[next]) {
        const dist = distances[current][next];
        if (dist < minDist) {
          minDist = dist;
          nearest = next;
        }
      }
    }
    
    if (nearest === null) break;
    current = nearest;
    tour.push(current);
    visited[current] = true;
  }
  
  return tour.length === cities ? tour : null;
}

/**
 * Solve a problem and return solution
 */
export function solveProblem(problem: ProblemType): { solution: Solution; solveTimeMs: number } | null {
  const startTime = performance.now();
  
  let solution: Solution | null = null;
  
  if (problem.SubsetSum) {
    const { numbers, target } = problem.SubsetSum;
    const indices = solveSubsetSum(numbers, target);
    if (indices) {
      solution = { SubsetSum: indices };
    }
  } else if (problem.SAT) {
    const { variables, clauses } = problem.SAT;
    const assignment = solveSATBruteForce(variables, clauses);
    if (assignment) {
      solution = { SAT: assignment };
    }
  } else if (problem.TSP) {
    const { cities, distances } = problem.TSP;
    const tour = solveTSP(cities, distances);
    if (tour) {
      solution = { TSP: tour };
    }
  }
  
  const solveTimeMs = performance.now() - startTime;
  
  if (!solution) {
    console.error('Failed to solve problem:', problem);
    return null;
  }
  
  return { solution, solveTimeMs };
}

/**
 * Hash data using blake3
 */
function hash(data: Uint8Array | string): string {
  const bytes = typeof data === 'string' ? hexToBytes(data) : data;
  return bytesToHex(blake3(bytes));
}

/**
 * Serialize problem to bytes using JSON (matches server-side serde_json::to_vec)
 */
function serializeProblem(problem: ProblemType): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(problem));
}

/**
 * Serialize solution to bytes using JSON (matches server-side serde_json::to_vec)
 */
function serializeSolution(solution: Solution): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(solution));
}

/**
 * Create commitment from problem, solution, and epoch salt
 * Uses JSON serialization to match server-side create_from_json() method
 * This enables client-side mining from web browsers
 * 
 * Matches Rust core/src/commitment.rs::create_from_json():
 * - problem_hash = Hash::new(serde_json::to_vec(problem))
 * - solution_hash = Hash::new(serde_json::to_vec(solution))
 * - commitment = H(problem_hash_bytes || epoch_salt_bytes || solution_hash_bytes)
 */
export function createCommitment(
  problem: ProblemType,
  solution: Solution,
  epochSalt: string // prev_hash (hex string)
): Commitment {
  // 1. Calculate problem_hash using JSON serialization (matches server-side create_from_json)
  const problemJson = JSON.stringify(problem);
  const problemBytes = new TextEncoder().encode(problemJson);
  const problemHashBytes = blake3(problemBytes);
  const problemHash = bytesToHex(problemHashBytes);
  
  // 2. Calculate solution_hash using JSON serialization
  const solutionJson = JSON.stringify(solution);
  const solutionBytes = new TextEncoder().encode(solutionJson);
  const solutionHashBytes = blake3(solutionBytes);
  const solutionHash = bytesToHex(solutionHashBytes);
  
  // 3. Concatenate: problem_hash_bytes || epoch_salt_bytes || solution_hash_bytes
  // epoch_salt is the prev_hash (32 bytes)
  const epochSaltBytes = hexToBytes(epochSalt);
  const commitmentData = new Uint8Array(32 + 32 + 32); // problem_hash (32) + epoch_salt (32) + solution_hash (32)
  commitmentData.set(problemHashBytes, 0);
  commitmentData.set(epochSaltBytes, 32);
  commitmentData.set(solutionHashBytes, 64);
  
  // 4. Hash the concatenated bytes
  const commitmentHashBytes = blake3(commitmentData);
  const commitmentHash = bytesToHex(commitmentHashBytes);
  
  return {
    hash: commitmentHash,
    problem_hash: problemHash
  };
}

/**
 * Main-thread PoW fallback (slow at high difficulty). Used when `Worker` is unavailable or fails.
 */
async function mineHeaderMainThreadCooperative(
  header: Block['header'],
  difficulty: number,
  onProgress?: (nonce: number, hash: string) => void
): Promise<{ nonce: number; hash: string } | null> {
  const n = Math.min(Math.max(0, difficulty), MAX_LEADING_HEX_ZEROS);
  const targetPrefix = '0'.repeat(n);
  const maxNonce = maxNonceForDifficulty(n);
  const yieldToBrowser = () => new Promise<void>((r) => setTimeout(r, 0));
  let lastProgressWall = 0;
  const wallStart = performance.now();
  for (let nonce = 0; nonce < maxNonce; ) {
    if (performance.now() - wallStart > MAX_POW_WORKER_MS) {
      throw new Error(
        'Header PoW exceeded the 25-minute browser limit. For very high difficulty, use a native miner or CLI.',
      );
    }
    const sliceStart = performance.now();
    while (nonce < maxNonce && performance.now() - sliceStart < MAX_POW_SLICE_MS) {
      header.nonce = nonce;
      const h = calculateHeaderHash(header);
      if (h.startsWith(targetPrefix)) {
        return { nonce, hash: h };
      }
      if (nonce > 0 && onProgress) {
        const now = performance.now();
        if (nonce % 200_000 === 0 || now - lastProgressWall >= 400) {
          onProgress(nonce, h);
          lastProgressWall = now;
        }
      }
      nonce++;
    }
    await yieldToBrowser();
  }
  return null;
}

/**
 * Header PoW: prefers a **dedicated worker** (full CPU, UI stays responsive). Falls back to
 * time-sliced main-thread search if workers fail.
 */
export async function mineHeader(
  header: Block['header'],
  difficulty: number = DEFAULT_DIFFICULTY,
  onProgress?: (nonce: number, hash: string) => void
): Promise<{ nonce: number; hash: string } | null> {
  if (typeof Worker !== 'undefined') {
    try {
      const worker = new MiningPowWorker();
      const headerClone = JSON.parse(JSON.stringify(header)) as Block['header'];
      const result = await new Promise<{ nonce: number; hash: string } | null>((resolve, reject) => {
        const timer = window.setTimeout(() => {
          worker.terminate();
          reject(
            new Error(
              'Header PoW exceeded the 25-minute browser limit. For very high difficulty, use a native miner or CLI.',
            ),
          );
        }, MAX_POW_WORKER_MS);
        worker.onmessage = (
          ev: MessageEvent<{
            type: string;
            message?: string;
            nonce?: number;
            hash?: string;
            result?: { nonce: number; hash: string } | null;
          }>
        ) => {
          const d = ev.data;
          if (d.type === 'progress' && onProgress && d.nonce != null && d.hash) {
            onProgress(d.nonce, d.hash);
          }
          if (d.type === 'error') {
            clearTimeout(timer);
            worker.terminate();
            reject(new Error(d.message || 'Header PoW worker failed'));
            return;
          }
          if (d.type === 'done') {
            clearTimeout(timer);
            worker.terminate();
            resolve(d.result ?? null);
          }
        };
        worker.onerror = (err) => {
          clearTimeout(timer);
          worker.terminate();
          reject(err);
        };
        worker.postMessage({ header: headerClone, difficulty });
      });
      if (result) {
        header.nonce = result.nonce;
      }
      return result;
    } catch (e) {
      console.warn('[mining] PoW worker failed; using main-thread fallback', e);
    }
  }

  const out = await mineHeaderMainThreadCooperative(header, difficulty, onProgress);
  if (out) {
    header.nonce = out.nonce;
  }
  return out;
}

export function getClientHeaderHashDebug(header: Block['header']): { hash: string; json: string } {
  return buildHeaderHashDebug(header);
}

/**
 * Calculate problem difficulty weight (matches Rust core/src/problem.rs)
 */
function calculateProblemDifficultyWeight(problem: ProblemType): number {
  if (problem.SubsetSum) {
    // Weight based on number count and magnitude
    return Math.log2(problem.SubsetSum.numbers.length);
  } else if (problem.SAT) {
    // Weight based on variables and clauses
    return problem.SAT.variables * Math.log2(problem.SAT.clauses.length);
  } else if (problem.TSP) {
    // Weight based on city count (factorial complexity)
    return Math.pow(problem.TSP.cities, 2);
  }
  return 1.0; // Custom problems
}

/**
 * Verify solution matches problem (matches Rust core/src/problem.rs)
 */
function verifySolution(solution: Solution, problem: ProblemType): boolean {
  if (solution.SubsetSum && problem.SubsetSum) {
    const sum = solution.SubsetSum.reduce((acc, idx) => acc + (problem.SubsetSum!.numbers[idx] || 0), 0);
    return sum === problem.SubsetSum.target;
  } else if (solution.SAT && problem.SAT) {
    if (solution.SAT.length !== problem.SAT.variables) {
      return false;
    }
    return problem.SAT.clauses.every(clause => 
      clause.literals.some(literal => {
        const varIdx = Math.abs(literal) - 1;
        if (varIdx >= solution.SAT!.length) return false;
        const value = solution.SAT![varIdx];
        return (literal > 0) === value;
      })
    );
  } else if (solution.TSP && problem.TSP) {
    // Verify tour visits all cities exactly once
    if (solution.TSP.length !== problem.TSP.cities) {
      return false;
    }
    const visited = new Array(problem.TSP.cities).fill(false);
    for (const city of solution.TSP) {
      if (city >= problem.TSP.cities || visited[city]) {
        return false;
      }
      visited[city] = true;
    }
    return true;
  }
  return false;
}

/**
 * Calculate solution quality (0.0 to 1.0) - matches Rust core/src/problem.rs
 */
function calculateSolutionQuality(solution: Solution, problem: ProblemType): number {
  if (solution.SubsetSum && problem.SubsetSum) {
    // Exact solution gets 1.0
    if (!verifySolution(solution, problem)) {
      return 0.0;
    }
    return 1.0;
  } else if (solution.SAT && problem.SAT) {
    // Exact solution gets 1.0
    if (!verifySolution(solution, problem)) {
      return 0.0;
    }
    return 1.0;
  } else if (solution.TSP && problem.TSP) {
    // Calculate tour length for quality
    if (!verifySolution(solution, problem)) {
      return 0.0;
    }
    let length = 0;
    for (let i = 0; i < problem.TSP.cities; i++) {
      const from = solution.TSP[i];
      const to = solution.TSP[(i + 1) % problem.TSP.cities];
      length += problem.TSP.distances[from][to];
    }
    // Lower length = higher quality (inverse ratio)
    return 1.0 / (length + 1.0);
  }
  return 0.0;
}

/**
 * Create complete block from problem, solution, and chain state
 */
export async function createBlock(
  prevHash: string | { hash?: string; bytes?: string } | any,
  height: number,
  minerAddress: string,
  transactions: any[] = [],
  problemSize: number = 10,
  difficulty: number = DEFAULT_DIFFICULTY,
  parentCumulativeWork: bigint = 0n,
  onMiningProgress?: (nonce: number, hash: string) => void
): Promise<Block | null> {
  console.log(`⛏️  Mining block #${height}...`);
  
  // Extract hash hex string
  const prevHashHex = extractHashHex(prevHash);
  
  // 1. Generate problem deterministically
  const problem = generateProblem(prevHashHex, height, problemSize);
  console.log(`📋 Generated problem:`, problem);
  
  // 2. Solve problem
  const solveResult = solveProblem(problem);
  if (!solveResult) {
    console.error('❌ Failed to solve problem');
    return null;
  }
  
  const { solution, solveTimeMs } = solveResult;
  console.log(`✅ Solved in ${solveTimeMs.toFixed(2)}ms`);
  
  // 3. Create commitment (epoch salt = prev_hash)
  const commitment = createCommitment(problem, solution, prevHashHex);
  console.log(`🔒 Commitment created: ${commitment.hash.slice(0, 16)}...`);
  
  // 4. Measured verify + PoUW metrics (consensus/src/work_score.rs: log₂(solve/verify)×quality)
  const solveTimeUs = Math.max(1, Math.round(solveTimeMs * 1000));
  if (!verifySolution(solution, problem)) {
    console.error('❌ Internal verify failed after solve');
    return null;
  }
  const verifyStart = performance.now();
  for (let i = 0; i < N_VERIFY_PASSES; i++) {
    verifySolution(solution, problem);
  }
  const verifyTimeUs = Math.max(
    1,
    Math.round(((performance.now() - verifyStart) * 1000) / N_VERIFY_PASSES)
  );
  const timeAsymmetryRatio = normalizeHeaderFloat(solveTimeUs / verifyTimeUs);
  
  const complexityWeight = normalizeHeaderFloat(calculateProblemDifficultyWeight(problem));
  const solutionQuality = normalizeHeaderFloat(calculateSolutionQuality(solution, problem));
  const workScore = normalizeHeaderFloat(
    workScoreBitsFromPouw(solveTimeUs, verifyTimeUs, solutionQuality)
  );
  
  const energyEstimateJoules = normalizeHeaderFloat(100.0 * (solveTimeMs / 1000));
  
  // 5. Create block header
  const timestamp = Math.floor(Date.now() / 1000);
  const transactionsRoot = hash(new TextEncoder().encode(JSON.stringify(transactions)));
  const solutionsRoot = hash(serializeSolution(solution));
  
  // Convert hashes and addresses to byte arrays for JSON serialization
  // Rust Hash and Address types serialize as [u8; 32] byte arrays
  const prevHashBytes = hexToBytes(prevHashHex);
  const transactionsRootBytes = hexToBytes(transactionsRoot);
  const solutionsRootBytes = hexToBytes(solutionsRoot);
  const commitmentHashBytes = hexToBytes(commitment.hash);
  const commitmentProblemHashBytes = hexToBytes(commitment.problem_hash);
  const minerAddressBytes = hexToBytes(minerAddress);
  
  let header: Block['header'] = {
    // Production is on the golden-enhanced chain, so submitted blocks must use v2 headers.
    version: 2,
    height,
    prev_hash: bytesToHex(prevHashBytes), // Hash as hex string (will be converted to byte array by serializeBlockForRpc)
    timestamp,
    transactions_root: bytesToHex(transactionsRootBytes), // Hash as hex string
    solutions_root: bytesToHex(solutionsRootBytes), // Hash as hex string
    commitment: {
      hash: bytesToHex(commitmentHashBytes), // Hash as hex string
      problem_hash: bytesToHex(commitmentProblemHashBytes) // Hash as hex string
    },
    work_score: workScore,
    miner: bytesToHex(minerAddressBytes), // Address as hex string (will be converted to byte array by serializeBlockForRpc)
    nonce: 0,
    solve_time_us: solveTimeUs,
    verify_time_us: verifyTimeUs,
    time_asymmetry_ratio: timeAsymmetryRatio,
    solution_quality: solutionQuality,
    complexity_weight: complexityWeight,
    energy_estimate_joules: energyEstimateJoules
  };
  
  // 6. Mine header (find nonce)
  console.log(`🎯 Mining header (difficulty: ${difficulty})...`);
  const miningResult = await mineHeader(header, difficulty, onMiningProgress);
  if (!miningResult) {
    console.error('❌ Failed to mine header');
    return null;
  }
  
  header.nonce = miningResult.nonce;
  console.log(`✅ Header mined! Nonce: ${miningResult.nonce}, Hash: ${miningResult.hash.slice(0, 16)}...`);
  
  // 7. Create solution reveal (convert Solution to SolutionType)
  const solutionType: SolutionType = {
    SubsetSum: solution.SubsetSum,
    SAT: solution.SAT,
    TSP: solution.TSP,
    Custom: solution.Custom
  };
  
  // 7. Create solution reveal (convert commitment hashes to hex strings)
  const solutionReveal: SolutionReveal = {
    problem,
    solution: solutionType,
    commitment: {
      hash: bytesToHex(commitmentHashBytes), // Hash as hex string (will be converted to byte array by serializeBlockForRpc)
      problem_hash: bytesToHex(commitmentProblemHashBytes) // Hash as hex string
    }
  };
  
  // 8. Create coinbase transaction (must match Rust CoinbaseTransaction: { to: Address, reward: Balance, height: u64 })
  // Note: Address and Hash serialize as byte arrays [u8; 32] in JSON
  const rewardBn = blockRewardFromTruncWorkAndParentW(
    truncatedHeaderWorkScoreU128(header.work_score),
    parentCumulativeWork
  );
  const coinbase = {
    to: Array.from(minerAddressBytes), // Address as byte array (reuse from above)
    reward: rewardAsJson(rewardBn),
    height
  };
  
  // 9. Construct complete block
  const block: Block = {
    header,
    coinbase,
    transactions,
    solution_reveal: solutionReveal
  };
  
  console.log(`🎉 Block #${height} created successfully!`);
  return block;
}

/**
 * Build a block from a caller-provided problem and solution, using `prevHash` as the epoch salt
 * (same as `createCommitment` / mining: H(problem_hash || epoch_salt || solution_hash)).
 * Use the current chain tip hash and next height from RPC before submitting.
 */
export async function createBlockFromSolvedProblem(
  prevHash: string | { hash?: string; bytes?: string } | any,
  height: number,
  minerAddress: string,
  problem: ProblemType,
  solution: Solution,
  solveTimeMs: number,
  parentCumulativeWork: bigint,
  transactions: any[] = [],
  difficulty: number = DEFAULT_DIFFICULTY,
  onMiningProgress?: (nonce: number, hash: string) => void
): Promise<Block | null> {
  const prevHashHex = extractHashHex(prevHash);
  if (!verifySolution(solution, problem)) {
    console.error("❌ Solution does not verify against problem");
    return null;
  }

  const commitment = createCommitment(problem, solution, prevHashHex);

  const solveTimeUs = Math.max(1, Math.round(solveTimeMs * 1000));
  const verifyStart = performance.now();
  for (let i = 0; i < N_VERIFY_PASSES; i++) {
    verifySolution(solution, problem);
  }
  const verifyTimeUs = Math.max(
    1,
    Math.round(((performance.now() - verifyStart) * 1000) / N_VERIFY_PASSES)
  );
  const timeAsymmetryRatio = normalizeHeaderFloat(solveTimeUs / verifyTimeUs);

  const complexityWeight = normalizeHeaderFloat(calculateProblemDifficultyWeight(problem));
  const solutionQuality = normalizeHeaderFloat(calculateSolutionQuality(solution, problem));
  const workScore = normalizeHeaderFloat(
    workScoreBitsFromPouw(solveTimeUs, verifyTimeUs, solutionQuality)
  );
  const energyEstimateJoules = normalizeHeaderFloat(100.0 * (solveTimeMs / 1000));

  const timestamp = Math.floor(Date.now() / 1000);
  const transactionsRoot = hash(new TextEncoder().encode(JSON.stringify(transactions)));
  const solutionsRoot = hash(serializeSolution(solution));

  const prevHashBytes = hexToBytes(prevHashHex);
  const transactionsRootBytes = hexToBytes(transactionsRoot);
  const solutionsRootBytes = hexToBytes(solutionsRoot);
  const commitmentHashBytes = hexToBytes(commitment.hash);
  const commitmentProblemHashBytes = hexToBytes(commitment.problem_hash);
  const minerAddressBytes = hexToBytes(minerAddress);

  let header: Block["header"] = {
    // Production is on the golden-enhanced chain, so submitted blocks must use v2 headers.
    version: 2,
    height,
    prev_hash: bytesToHex(prevHashBytes),
    timestamp,
    transactions_root: bytesToHex(transactionsRootBytes),
    solutions_root: bytesToHex(solutionsRootBytes),
    commitment: {
      hash: bytesToHex(commitmentHashBytes),
      problem_hash: bytesToHex(commitmentProblemHashBytes),
    },
    work_score: workScore,
    miner: bytesToHex(minerAddressBytes),
    nonce: 0,
    solve_time_us: solveTimeUs,
    verify_time_us: verifyTimeUs,
    time_asymmetry_ratio: timeAsymmetryRatio,
    solution_quality: solutionQuality,
    complexity_weight: complexityWeight,
    energy_estimate_joules: energyEstimateJoules,
  };

  const miningResult = await mineHeader(header, difficulty, onMiningProgress);
  if (!miningResult) {
    console.error("❌ Failed to mine header");
    return null;
  }

  header.nonce = miningResult.nonce;

  const solutionType: SolutionType = {
    SubsetSum: solution.SubsetSum,
    SAT: solution.SAT,
    TSP: solution.TSP,
    Custom: solution.Custom,
  };

  const solutionReveal: SolutionReveal = {
    problem,
    solution: solutionType,
    commitment: {
      hash: bytesToHex(commitmentHashBytes),
      problem_hash: bytesToHex(commitmentProblemHashBytes),
    },
  };

  const rewardBn = blockRewardFromTruncWorkAndParentW(
    truncatedHeaderWorkScoreU128(header.work_score),
    parentCumulativeWork
  );
  const coinbase = {
    to: Array.from(minerAddressBytes),
    reward: rewardAsJson(rewardBn),
    height,
  };

  return {
    header,
    coinbase,
    transactions,
    solution_reveal: solutionReveal,
  };
}