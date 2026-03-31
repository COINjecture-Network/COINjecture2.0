// Cryptographic utilities for COINjecture Network B
// Client-side key generation and transaction signing using Ed25519
// Adapted from web-wallet for integration into main web app

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
 * WARNING: Storing private keys in localStorage is NOT secure for production
 * This is for testnet demonstration only
 */
export const KeyStore = {
  save(name: string, keyPair: KeyPair): void {
    const keys = this.list();
    keys[name] = {
      publicKey: keyPair.publicKey,
      address: keyPair.address,
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
 */
export interface TransactionPayload {
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
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

