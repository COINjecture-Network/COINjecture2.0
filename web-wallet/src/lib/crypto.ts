// Cryptographic utilities for COINjecture Network B
// Client-side key generation and transaction signing using Ed25519

import { ed25519 } from '@noble/curves/ed25519';
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, hexToBytes } from '@noble/hashes/utils';

export interface KeyPair {
  privateKey: string; // hex
  publicKey: string;  // hex
  address: string;    // hex
}

/**
 * Generate a new Ed25519 keypair
 * Matches the Rust implementation in validator keystore
 */
export function generateKeyPair(): KeyPair {
  // Generate random private key (32 bytes)
  const privateKeyBytes = ed25519.utils.randomPrivateKey();

  // Derive public key
  const publicKeyBytes = ed25519.getPublicKey(privateKeyBytes);

  // Address IS the public key (Rust: Address::from_pubkey just copies bytes)
  // No hashing needed!
  const addressBytes = publicKeyBytes;

  return {
    privateKey: bytesToHex(privateKeyBytes),
    publicKey: bytesToHex(publicKeyBytes),
    address: bytesToHex(addressBytes)
  };
}

/**
 * Import keypair from private key hex
 */
export function importKeyPair(privateKeyHex: string): KeyPair {
  const privateKeyBytes = hexToBytes(privateKeyHex);

  if (privateKeyBytes.length !== 32) {
    throw new Error('Private key must be 32 bytes');
  }

  const publicKeyBytes = ed25519.getPublicKey(privateKeyBytes);
  // Address IS the public key (no hashing)
  const addressBytes = publicKeyBytes;

  return {
    privateKey: privateKeyHex,
    publicKey: bytesToHex(publicKeyBytes),
    address: bytesToHex(addressBytes)
  };
}

/**
 * Sign a message with Ed25519
 */
export function signMessage(message: Uint8Array, privateKeyHex: string): string {
  const privateKeyBytes = hexToBytes(privateKeyHex);
  const signature = ed25519.sign(message, privateKeyBytes);
  return bytesToHex(signature);
}

/**
 * Verify an Ed25519 signature
 */
export function verifySignature(
  message: Uint8Array,
  signatureHex: string,
  publicKeyHex: string
): boolean {
  try {
    const signature = hexToBytes(signatureHex);
    const publicKey = hexToBytes(publicKeyHex);
    return ed25519.verify(signature, message, publicKey);
  } catch {
    return false;
  }
}

/**
 * Storage utilities for managing keys in browser
 */
export const KeyStore = {
  save(name: string, keyPair: KeyPair): void {
    const keys = this.list();
    keys[name] = {
      publicKey: keyPair.publicKey,
      address: keyPair.address,
      // WARNING: Storing private keys in localStorage is NOT secure for production
      // This is for testnet demonstration only
      privateKey: keyPair.privateKey
    };
    localStorage.setItem('coinject_keys', JSON.stringify(keys));
  },

  load(name: string): KeyPair | null {
    const keys = this.list();
    return keys[name] || null;
  },

  list(): Record<string, KeyPair> {
    const data = localStorage.getItem('coinject_keys');
    return data ? JSON.parse(data) : {};
  },

  delete(name: string): void {
    const keys = this.list();
    delete keys[name];
    localStorage.setItem('coinject_keys', JSON.stringify(keys));
  },

  clear(): void {
    localStorage.removeItem('coinject_keys');
  }
};

/**
 * Create a transaction payload for signing
 * Must match the Rust Transaction structure serialization
 */
export interface TransactionPayload {
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
}

/**
 * Serialize transaction for signing (simple JSON serialization)
 * In production, this should match the exact bincode serialization from Rust
 */
export function serializeTransaction(tx: TransactionPayload): Uint8Array {
  const json = JSON.stringify({
    from: tx.from,
    to: tx.to,
    amount: tx.amount.toString(),
    fee: tx.fee.toString(),
    nonce: tx.nonce
  });
  return new TextEncoder().encode(json);
}

/**
 * Create and sign a transfer transaction
 * Returns a JSON-serialized Transaction enum matching Rust format
 */
export function createSignedTransferTransaction(
  from: string,
  to: string,
  amount: number,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  // Convert hex strings to byte arrays
  const fromBytes = Array.from(hexToBytes(from));
  const toBytes = Array.from(hexToBytes(to));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Create signing message matching Rust's signing_message() format
  // Must match exactly: from + to + amount + fee + nonce + public_key (all as raw bytes)
  // Balance type in Rust is u128 (16 bytes), not u64!
  const signingMessage = new Uint8Array(32 + 32 + 16 + 16 + 8 + 32); // 136 bytes total
  let offset = 0;

  // from (32 bytes)
  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // to (32 bytes)
  signingMessage.set(hexToBytes(to), offset);
  offset += 32;

  // amount (16 bytes, little-endian u128)
  // Since amounts are small, store in lower 64 bits, upper 64 bits = 0
  const amountView = new DataView(new ArrayBuffer(16));
  amountView.setBigUint64(0, BigInt(amount), true); // lower 64 bits, little-endian
  amountView.setBigUint64(8, 0n, true); // upper 64 bits = 0
  signingMessage.set(new Uint8Array(amountView.buffer), offset);
  offset += 16;

  // fee (16 bytes, little-endian u128)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true); // lower 64 bits, little-endian
  feeView.setBigUint64(8, 0n, true); // upper 64 bits = 0
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (8 bytes, little-endian u64)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  // public_key (32 bytes)
  signingMessage.set(hexToBytes(publicKeyHex), offset);

  // Sign the raw bytes (matching Rust's signing_message)
  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  // Create final transaction in Rust enum format
  const transaction = {
    Transfer: {
      from: fromBytes,
      to: toBytes,
      amount,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign a timelock transaction
 */
export function createSignedTimeLockTransaction(
  from: string,
  recipient: string,
  amount: number,
  unlockTime: number, // Unix timestamp
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const fromBytes = Array.from(hexToBytes(from));
  const recipientBytes = Array.from(hexToBytes(recipient));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Signing message: from + recipient + amount + unlock_time + fee + nonce + public_key
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 16 + 8 + 32); // 144 bytes
  let offset = 0;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  signingMessage.set(hexToBytes(recipient), offset);
  offset += 32;

  // amount (u128, 16 bytes)
  const amountView = new DataView(new ArrayBuffer(16));
  amountView.setBigUint64(0, BigInt(amount), true);
  amountView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(amountView.buffer), offset);
  offset += 16;

  // unlock_time (i64, 8 bytes)
  const unlockTimeView = new DataView(new ArrayBuffer(8));
  unlockTimeView.setBigInt64(0, BigInt(unlockTime), true);
  signingMessage.set(new Uint8Array(unlockTimeView.buffer), offset);
  offset += 8;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    TimeLock: {
      from: fromBytes,
      recipient: recipientBytes,
      amount,
      unlock_time: unlockTime,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign an escrow creation transaction
 */
export function createSignedEscrowCreateTransaction(
  from: string,
  recipient: string,
  arbiter: string | null,
  amount: number,
  timeout: number, // Unix timestamp
  conditions: string,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const fromBytes = Array.from(hexToBytes(from));
  const recipientBytes = Array.from(hexToBytes(recipient));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Generate escrow ID
  const escrowIdData = `${from}-${recipient}-${amount}-${Date.now()}`;
  const escrowIdHash = blake3(new TextEncoder().encode(escrowIdData));
  const escrowIdBytes = Array.from(escrowIdHash);

  // Hash the conditions
  const conditionsHash = blake3(new TextEncoder().encode(conditions));
  const conditionsHashBytes = Array.from(conditionsHash);

  // Arbiter bytes (optional)
  const arbiterBytes = arbiter ? Array.from(hexToBytes(arbiter)) : null;

  // Signing message: escrow_id + from + fee + nonce + public_key + recipient + amount + timeout
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32 + 32 + 16 + 8); // 176 bytes
  let offset = 0;

  signingMessage.set(escrowIdHash, offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);
  offset += 32;

  signingMessage.set(hexToBytes(recipient), offset);
  offset += 32;

  // amount (u128, 16 bytes)
  const amountView = new DataView(new ArrayBuffer(16));
  amountView.setBigUint64(0, BigInt(amount), true);
  amountView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(amountView.buffer), offset);
  offset += 16;

  // timeout (i64, 8 bytes)
  const timeoutView = new DataView(new ArrayBuffer(8));
  timeoutView.setBigInt64(0, BigInt(timeout), true);
  signingMessage.set(new Uint8Array(timeoutView.buffer), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    Escrow: {
      escrow_type: {
        Create: {
          recipient: recipientBytes,
          arbiter: arbiterBytes,
          amount,
          timeout,
          conditions_hash: conditionsHashBytes
        }
      },
      escrow_id: escrowIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes,
      additional_signatures: []
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign an escrow release transaction
 */
export function createSignedEscrowReleaseTransaction(
  escrowId: string,
  from: string,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const escrowIdBytes = Array.from(hexToBytes(escrowId));
  const fromBytes = Array.from(hexToBytes(from));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Signing message: escrow_id + from + fee + nonce + public_key
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32); // 120 bytes
  let offset = 0;

  signingMessage.set(hexToBytes(escrowId), offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    Escrow: {
      escrow_type: "Release",
      escrow_id: escrowIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes,
      additional_signatures: []
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign an escrow refund transaction
 */
export function createSignedEscrowRefundTransaction(
  escrowId: string,
  from: string,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const escrowIdBytes = Array.from(hexToBytes(escrowId));
  const fromBytes = Array.from(hexToBytes(from));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Signing message: escrow_id + from + fee + nonce + public_key
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32); // 120 bytes
  let offset = 0;

  signingMessage.set(hexToBytes(escrowId), offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    Escrow: {
      escrow_type: "Refund",
      escrow_id: escrowIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes,
      additional_signatures: []
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign a channel open transaction
 */
export function createSignedChannelOpenTransaction(
  from: string,
  participantB: string,
  depositA: number,
  depositB: number,
  timeout: number, // seconds
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const fromBytes = Array.from(hexToBytes(from));
  const participantBBytes = Array.from(hexToBytes(participantB));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Generate channel ID
  const channelIdData = `${from}-${participantB}-${depositA + depositB}-${Date.now()}`;
  const channelIdHash = blake3(new TextEncoder().encode(channelIdData));
  const channelIdBytes = Array.from(channelIdHash);

  // Signing message: channel_id + from + fee + nonce + public_key + deposit_a + deposit_b
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32 + 16 + 16); // 152 bytes
  let offset = 0;

  signingMessage.set(channelIdHash, offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);
  offset += 32;

  // deposit_a (u128, 16 bytes)
  const depositAView = new DataView(new ArrayBuffer(16));
  depositAView.setBigUint64(0, BigInt(depositA), true);
  depositAView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(depositAView.buffer), offset);
  offset += 16;

  // deposit_b (u128, 16 bytes)
  const depositBView = new DataView(new ArrayBuffer(16));
  depositBView.setBigUint64(0, BigInt(depositB), true);
  depositBView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(depositBView.buffer), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    Channel: {
      channel_type: {
        Open: {
          participant_a: fromBytes,
          participant_b: participantBBytes,
          deposit_a: depositA,
          deposit_b: depositB,
          timeout
        }
      },
      channel_id: channelIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes,
      additional_signatures: []
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign a channel close transaction
 */
export function createSignedChannelCloseTransaction(
  channelId: string,
  from: string,
  finalBalanceA: number,
  finalBalanceB: number,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const channelIdBytes = Array.from(hexToBytes(channelId));
  const fromBytes = Array.from(hexToBytes(from));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Signing message: channel_id + from + fee + nonce + public_key + final_balance_a + final_balance_b
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32 + 16 + 16); // 152 bytes
  let offset = 0;

  signingMessage.set(hexToBytes(channelId), offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);
  offset += 32;

  // final_balance_a (u128, 16 bytes)
  const balanceAView = new DataView(new ArrayBuffer(16));
  balanceAView.setBigUint64(0, BigInt(finalBalanceA), true);
  balanceAView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(balanceAView.buffer), offset);
  offset += 16;

  // final_balance_b (u128, 16 bytes)
  const balanceBView = new DataView(new ArrayBuffer(16));
  balanceBView.setBigUint64(0, BigInt(finalBalanceB), true);
  balanceBView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(balanceBView.buffer), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    Channel: {
      channel_type: {
        CooperativeClose: {
          final_balance_a: finalBalanceA,
          final_balance_b: finalBalanceB
        }
      },
      channel_id: channelIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes,
      additional_signatures: []
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign a pool swap transaction
 */
export function createSignedPoolSwapTransaction(
  from: string,
  poolFrom: 'D1' | 'D2' | 'D3',
  poolTo: 'D1' | 'D2' | 'D3',
  amountIn: number,
  minAmountOut: number,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const fromBytes = Array.from(hexToBytes(from));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Signing message: pool_from + pool_to + from + amount_in + min_amount_out + fee + nonce + public_key
  const signingMessage = new Uint8Array(32 + 16 + 16 + 16 + 8 + 32); // 120 bytes (no pool enums in signing)
  let offset = 0;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // amount_in (u128, 16 bytes)
  const amountInView = new DataView(new ArrayBuffer(16));
  amountInView.setBigUint64(0, BigInt(amountIn), true);
  amountInView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(amountInView.buffer), offset);
  offset += 16;

  // min_amount_out (u128, 16 bytes)
  const minAmountOutView = new DataView(new ArrayBuffer(16));
  minAmountOutView.setBigUint64(0, BigInt(minAmountOut), true);
  minAmountOutView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(minAmountOutView.buffer), offset);
  offset += 16;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    DimensionalPoolSwap: {
      pool_from: poolFrom,
      pool_to: poolTo,
      from: fromBytes,
      amount_in: amountIn,
      min_amount_out: minAmountOut,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes
    }
  };

  return JSON.stringify(transaction);
}

/**
 * Create and sign a TrustLine creation transaction
 */
export function createSignedTrustLineCreateTransaction(
  from: string,
  accountB: string,
  limitAtoB: number,
  limitBtoA: number,
  dimensionalScale: number,
  fee: number,
  nonce: number,
  privateKeyHex: string,
  publicKeyHex: string
): string {
  const fromBytes = Array.from(hexToBytes(from));
  const accountBBytes = Array.from(hexToBytes(accountB));
  const publicKeyBytes = Array.from(hexToBytes(publicKeyHex));

  // Generate trustline ID
  const trustlineIdData = `${from}-${accountB}-${Date.now()}`;
  const trustlineIdHash = blake3(new TextEncoder().encode(trustlineIdData));
  const trustlineIdBytes = Array.from(trustlineIdHash);

  // Serialize trustline_type (bincode format)
  // TrustLineType::Create variant (index 0) + Create struct fields
  const trustlineTypeBytes = new Uint8Array(4 + 32 + 16 + 16 + 2 + 2 + 1 + 1); // 74 bytes
  let typeOffset = 0;

  // Variant index for Create = 0 (u32, little-endian)
  const variantView = new DataView(trustlineTypeBytes.buffer);
  variantView.setUint32(typeOffset, 0, true);
  typeOffset += 4;

  // account_b (32 bytes)
  trustlineTypeBytes.set(hexToBytes(accountB), typeOffset);
  typeOffset += 32;

  // limit_a_to_b (u128, 16 bytes)
  const limitAToBView = new DataView(new ArrayBuffer(16));
  limitAToBView.setBigUint64(0, BigInt(limitAtoB), true);
  limitAToBView.setBigUint64(8, 0n, true);
  trustlineTypeBytes.set(new Uint8Array(limitAToBView.buffer), typeOffset);
  typeOffset += 16;

  // limit_b_to_a (u128, 16 bytes)
  const limitBToAView = new DataView(new ArrayBuffer(16));
  limitBToAView.setBigUint64(0, BigInt(limitBtoA), true);
  limitBToAView.setBigUint64(8, 0n, true);
  trustlineTypeBytes.set(new Uint8Array(limitBToAView.buffer), typeOffset);
  typeOffset += 16;

  // quality_in (u16, 2 bytes)
  variantView.setUint16(typeOffset, 10000, true); // 100% in basis points
  typeOffset += 2;

  // quality_out (u16, 2 bytes)
  variantView.setUint16(typeOffset, 10000, true); // 100% in basis points
  typeOffset += 2;

  // ripple_enabled (bool, 1 byte)
  trustlineTypeBytes[typeOffset] = 1; // true
  typeOffset += 1;

  // dimensional_scale (u8, 1 byte)
  trustlineTypeBytes[typeOffset] = dimensionalScale;

  // Signing message: trustline_id + from + fee + nonce + public_key + trustline_type
  const signingMessage = new Uint8Array(32 + 32 + 16 + 8 + 32 + 74); // 194 bytes
  let offset = 0;

  signingMessage.set(trustlineIdHash, offset);
  offset += 32;

  signingMessage.set(hexToBytes(from), offset);
  offset += 32;

  // fee (u128, 16 bytes)
  const feeView = new DataView(new ArrayBuffer(16));
  feeView.setBigUint64(0, BigInt(fee), true);
  feeView.setBigUint64(8, 0n, true);
  signingMessage.set(new Uint8Array(feeView.buffer), offset);
  offset += 16;

  // nonce (u64, 8 bytes)
  const nonceView = new DataView(new ArrayBuffer(8));
  nonceView.setBigUint64(0, BigInt(nonce), true);
  signingMessage.set(new Uint8Array(nonceView.buffer), offset);
  offset += 8;

  signingMessage.set(hexToBytes(publicKeyHex), offset);
  offset += 32;

  // Add serialized trustline_type
  signingMessage.set(trustlineTypeBytes, offset);

  const signatureHex = signMessage(signingMessage, privateKeyHex);
  const signatureBytes = Array.from(hexToBytes(signatureHex));

  const transaction = {
    TrustLine: {
      trustline_type: {
        Create: {
          account_b: accountBBytes,
          limit_a_to_b: limitAtoB,
          limit_b_to_a: limitBtoA,
          quality_in: 10000, // 100% (basis points)
          quality_out: 10000, // 100% (basis points)
          ripple_enabled: true,
          dimensional_scale: dimensionalScale
        }
      },
      trustline_id: trustlineIdBytes,
      from: fromBytes,
      fee,
      nonce,
      public_key: publicKeyBytes,
      signature: signatureBytes
    }
  };

  return JSON.stringify(transaction);
}

