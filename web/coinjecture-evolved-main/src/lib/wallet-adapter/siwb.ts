// SIWB (Sign-In With BEANS) API client
// Talks to the COINjecture API server auth endpoints

import type { AuthMeResponse, AuthUser, SiwbChallenge, SiwbVerifyResponse } from './types';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3030';

/**
 * POST /auth/challenge — request a SIWB challenge message for the given wallet.
 */
export async function requestChallenge(walletAddress: string): Promise<SiwbChallenge> {
  const res = await fetch(`${API_BASE}/auth/challenge`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ wallet_address: walletAddress }),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body?.error?.message || `Challenge failed: ${res.status}`);
  }
  return res.json();
}

/**
 * POST /auth/verify — submit the signed challenge and receive a JWT.
 */
export async function verifySignature(
  walletAddress: string,
  signature: string, // hex
  message: string,
): Promise<SiwbVerifyResponse> {
  const res = await fetch(`${API_BASE}/auth/verify`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ wallet_address: walletAddress, signature, message }),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body?.error?.message || `Verify failed: ${res.status}`);
  }
  return res.json();
}

/**
 * GET /auth/me — validate the current JWT and return session info.
 */
export async function getMe(token: string): Promise<AuthMeResponse> {
  const res = await fetch(`${API_BASE}/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    throw new Error(`Auth check failed: ${res.status}`);
  }
  return res.json();
}

/**
 * Full SIWB flow:
 * 1. Request challenge
 * 2. Sign the challenge message
 * 3. Verify the signature and receive a JWT
 */
export async function performSiwbAuth(
  walletAddress: string,
  signFn: (message: Uint8Array) => Promise<string>, // returns hex signature
): Promise<SiwbVerifyResponse> {
  const challenge = await requestChallenge(walletAddress);
  const signature = await signFn(new TextEncoder().encode(challenge.message));
  return verifySignature(walletAddress, signature, challenge.message);
}
