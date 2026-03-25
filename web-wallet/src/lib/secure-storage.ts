/**
 * Secure key storage using Web Crypto API (AES-GCM + PBKDF2).
 *
 * Keys are encrypted with a password-derived key so that raw private keys
 * are never stored in plaintext.  The salt is stored separately in
 * localStorage; the ciphertext is stored under a versioned key so it does
 * not collide with the legacy unencrypted store used by KeyStore.
 *
 * Usage:
 *   await SecureKeyStore.save(name, keyPair, password)
 *   const keys = await SecureKeyStore.list(password)   // throws if wrong password
 *   await SecureKeyStore.delete(name, password)
 *   SecureKeyStore.isSupported()                       // false in non-HTTPS contexts
 */

import { KeyPair } from './crypto'

const ENC_KEY = 'coinject_keys_enc_v1'
const SALT_KEY = 'coinject_enc_salt_v1'

// ── Helpers ──────────────────────────────────────────────────────────────────

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}

function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) throw new Error('Invalid hex string')
  const out = new Uint8Array(hex.length / 2)
  for (let i = 0; i < hex.length; i += 2) {
    out[i / 2] = parseInt(hex.slice(i, i + 2), 16)
  }
  return out
}

/** Retrieve or generate a 16-byte PBKDF2 salt tied to this browser. */
function getOrCreateSalt(): Uint8Array<ArrayBuffer> {
  const stored = localStorage.getItem(SALT_KEY)
  if (stored) {
    const b = hexToBytes(stored)
    // Ensure ArrayBuffer (not SharedArrayBuffer) for SubtleCrypto compatibility
    const out = new Uint8Array(b.byteLength)
    out.set(b)
    return out
  }
  const salt = new Uint8Array(16)
  crypto.getRandomValues(salt)
  localStorage.setItem(SALT_KEY, bytesToHex(salt))
  return salt
}

/** Derive a non-extractable AES-256-GCM key from password + salt. */
async function deriveKey(password: string, salt: Uint8Array<ArrayBuffer>): Promise<CryptoKey> {
  const raw = new TextEncoder().encode(password)
  const baseKey = await crypto.subtle.importKey('raw', raw, 'PBKDF2', false, ['deriveKey'])
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 100_000, hash: 'SHA-256' },
    baseKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  )
}

/** Encrypt plaintext → hex-encoded (IV ‖ ciphertext). */
async function encryptData(plaintext: string, key: CryptoKey): Promise<string> {
  const iv = new Uint8Array(12)
  crypto.getRandomValues(iv)
  const encoded = new TextEncoder().encode(plaintext)
  const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, encoded)
  const combined = new Uint8Array(12 + ciphertext.byteLength)
  combined.set(iv, 0)
  combined.set(new Uint8Array(ciphertext), 12)
  return bytesToHex(combined)
}

/** Decrypt hex-encoded (IV ‖ ciphertext) → plaintext. */
async function decryptData(encryptedHex: string, key: CryptoKey): Promise<string> {
  const combined = hexToBytes(encryptedHex)
  const iv = combined.slice(0, 12)
  const ciphertext = combined.slice(12)
  const decrypted = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext)
  return new TextDecoder().decode(decrypted)
}

// ── Public API ────────────────────────────────────────────────────────────────

export const SecureKeyStore = {
  /** True when the browser supports SubtleCrypto (requires HTTPS or localhost). */
  isSupported(): boolean {
    return (
      typeof window !== 'undefined' &&
      typeof window.crypto !== 'undefined' &&
      typeof window.crypto.subtle !== 'undefined'
    )
  },

  /** Encrypt and store a key pair under `name`. */
  async save(name: string, keyPair: KeyPair, password: string): Promise<void> {
    const salt = getOrCreateSalt()
    const cryptoKey = await deriveKey(password, salt)
    // Read existing, decrypt, merge, re-encrypt atomically
    let existing: Record<string, KeyPair> = {}
    const stored = localStorage.getItem(ENC_KEY)
    if (stored) {
      try {
        const plain = await decryptData(stored, cryptoKey)
        existing = JSON.parse(plain)
      } catch {
        // Wrong password or corrupt — start fresh (caller should handle)
        throw new Error('Wrong password — cannot unlock existing keystore')
      }
    }
    existing[name] = keyPair
    const encrypted = await encryptData(JSON.stringify(existing), cryptoKey)
    localStorage.setItem(ENC_KEY, encrypted)
  },

  /** Decrypt and list all stored key pairs. Throws on wrong password. */
  async list(password: string): Promise<Record<string, KeyPair>> {
    const stored = localStorage.getItem(ENC_KEY)
    if (!stored) return {}
    const salt = getOrCreateSalt()
    const cryptoKey = await deriveKey(password, salt)
    try {
      const plain = await decryptData(stored, cryptoKey)
      return JSON.parse(plain) as Record<string, KeyPair>
    } catch {
      throw new Error('Wrong password or corrupt keystore')
    }
  },

  /** Remove one key pair from the encrypted store. */
  async delete(name: string, password: string): Promise<void> {
    const keys = await this.list(password)
    delete keys[name]
    const salt = getOrCreateSalt()
    const cryptoKey = await deriveKey(password, salt)
    const encrypted = await encryptData(JSON.stringify(keys), cryptoKey)
    localStorage.setItem(ENC_KEY, encrypted)
  },

  /** True when there is already an encrypted keystore blob. */
  hasEncryptedStore(): boolean {
    return localStorage.getItem(ENC_KEY) !== null
  },
}
