// Privacy Cryptography Utilities
// Client-side commitment generation and placeholder ZK proofs for testnet

import type { ProblemType } from './blockchain-rpc-client';

export interface PrivacyCredentials {
  commitment: string; // Hex-encoded hash
  salt: string; // Hex-encoded 32 bytes
  proof: {
    proof_bytes: string; // Hex-encoded placeholder proof
    vk_hash: string; // Hex-encoded verification key hash
    public_inputs: string[]; // Hex-encoded public inputs
  };
}

/**
 * Generate a cryptographically secure random salt (32 bytes)
 */
export function generateSalt(): Uint8Array {
  const salt = new Uint8Array(32);
  crypto.getRandomValues(salt);
  return salt;
}

/**
 * Convert Uint8Array to hex string with 0x prefix
 */
export function toHex(bytes: Uint8Array): string {
  return '0x' + Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Convert hex string (with or without 0x prefix) to Uint8Array
 */
export function fromHex(hex: string): Uint8Array {
  const cleaned = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleaned.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/**
 * Serialize a problem to bytes for hashing
 * Matches the serialization logic in core/src/privacy.rs
 */
function serializeProblem(problem: ProblemType): Uint8Array {
  // For testnet, use simple JSON serialization
  // In production, this should match bincode format exactly
  const json = JSON.stringify(problem);
  const encoder = new TextEncoder();
  return encoder.encode(json);
}

/**
 * Compute SHA-256 hash of data
 */
async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  return new Uint8Array(hashBuffer);
}

/**
 * Compute commitment: H(problem || salt)
 * Matches the commitment scheme in core/src/privacy.rs
 */
export async function computeCommitment(
  problem: ProblemType,
  salt: Uint8Array
): Promise<string> {
  const problemBytes = serializeProblem(problem);

  // Concatenate problem bytes + salt
  const combined = new Uint8Array(problemBytes.length + salt.length);
  combined.set(problemBytes, 0);
  combined.set(salt, problemBytes.length);

  // Hash the concatenation
  const hash = await sha256(combined);

  return toHex(hash);
}

/**
 * Estimate problem complexity based on type and size
 */
function estimateComplexity(problem: ProblemType): number {
  if ('SubsetSum' in problem) {
    return problem.SubsetSum.numbers.length * 1.5;
  }
  if ('SAT' in problem) {
    return problem.SAT.variables * problem.SAT.clauses.length * 0.5;
  }
  if ('TSP' in problem) {
    return problem.TSP.cities ** 2;
  }
  return 1.0;
}

/**
 * Get problem type string and size
 */
function getProblemMetadata(problem: ProblemType): { type: string; size: number } {
  if ('SubsetSum' in problem) {
    return { type: 'SubsetSum', size: problem.SubsetSum.numbers.length };
  }
  if ('SAT' in problem) {
    return { type: 'SAT', size: problem.SAT.variables };
  }
  if ('TSP' in problem) {
    return { type: 'TSP', size: problem.TSP.cities };
  }
  return { type: 'Custom', size: 0 };
}

/**
 * Create a placeholder ZK proof for testnet
 *
 * IMPORTANT: This is a PLACEHOLDER for testnet only!
 * In production, this must be replaced with real ZK proof generation using:
 * - ark-groth16 (compiled to WASM)
 * - bellman (via WASM)
 * - or SnarkJS in the browser
 *
 * The placeholder proof contains:
 * - Marker bytes indicating this is a placeholder
 * - Problem hash for verification
 * - Public parameters
 */
async function createPlaceholderProof(
  problem: ProblemType,
  salt: Uint8Array,
  commitment: string
): Promise<{
  proof_bytes: string;
  vk_hash: string;
  public_inputs: string[];
}> {
  const { type, size } = getProblemMetadata(problem);
  const complexity = estimateComplexity(problem);

  // Create placeholder proof structure
  // Format: [PLACEHOLDER_MARKER (8 bytes)] [commitment (32 bytes)] [metadata]
  const marker = new TextEncoder().encode('TESTPROF'); // 8 bytes
  const commitmentBytes = fromHex(commitment);
  const metadata = new TextEncoder().encode(JSON.stringify({ type, size, complexity }));

  const proofBytes = new Uint8Array(marker.length + commitmentBytes.length + metadata.length);
  proofBytes.set(marker, 0);
  proofBytes.set(commitmentBytes, marker.length);
  proofBytes.set(metadata, marker.length + commitmentBytes.length);

  // Create placeholder verification key hash
  // In production, this would be the actual Groth16 verification key hash
  const vkHash = await sha256(new TextEncoder().encode('TESTNET_VK_V1'));

  // Public inputs for ZK circuit verification
  // In production: [commitment_field_element, complexity_field_element]
  const publicParams = new TextEncoder().encode(JSON.stringify({
    problem_type: type,
    size,
    complexity_estimate: complexity,
  }));
  const publicParamsHash = await sha256(publicParams);

  return {
    proof_bytes: toHex(proofBytes),
    vk_hash: toHex(vkHash),
    public_inputs: [
      commitment, // Commitment as hex
      toHex(publicParamsHash), // Public parameters hash
    ],
  };
}

/**
 * Generate complete privacy credentials for a private bounty submission
 *
 * Returns:
 * - commitment: Hash binding to the problem instance
 * - salt: Random salt used in commitment (MUST BE STORED SECURELY!)
 * - proof: Placeholder ZK proof demonstrating problem well-formedness
 *
 * SECURITY NOTE: The salt MUST be stored securely by the submitter.
 * Without the salt, the problem cannot be revealed later!
 */
export async function generatePrivacyCredentials(
  problem: ProblemType
): Promise<PrivacyCredentials> {
  // Generate random salt
  const salt = generateSalt();

  // Compute commitment
  const commitment = await computeCommitment(problem, salt);

  // Create placeholder proof
  const proof = await createPlaceholderProof(problem, salt, commitment);

  return {
    commitment,
    salt: toHex(salt),
    proof,
  };
}

/**
 * Verify that a reveal matches a commitment
 * Used client-side to check reveal before submission
 */
export async function verifyReveal(
  problem: ProblemType,
  salt: string,
  expectedCommitment: string
): Promise<boolean> {
  const saltBytes = fromHex(salt);
  const actualCommitment = await computeCommitment(problem, saltBytes);
  return actualCommitment.toLowerCase() === expectedCommitment.toLowerCase();
}

/**
 * Get problem metadata for RPC submission
 */
export function getProblemParamsForRPC(problem: ProblemType) {
  const { type, size } = getProblemMetadata(problem);
  const complexity = estimateComplexity(problem);

  return {
    problem_type: type,
    size,
    complexity_estimate: complexity,
  };
}
