// Base API client for the COINjecture API server.
// All REST calls go through this to get auth headers and error handling.

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3030';

export async function apiFetch<T = any>(
  path: string,
  options?: RequestInit & { token?: string },
): Promise<T> {
  const { token, ...fetchOptions } = options || {};
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...(fetchOptions.headers || {}),
  };

  const res = await fetch(`${API_BASE}${path}`, {
    ...fetchOptions,
    headers,
  });

  if (!res.ok) {
    const error = await res
      .json()
      .catch(() => ({ error: { message: res.statusText } }));
    throw new Error(error.error?.message || `API error: ${res.status}`);
  }

  return res.json();
}

export interface WalletTransaction {
  tx_hash: string;
  tx_type: string;
  block_height: number;
  signer: string | null;
  payload: unknown;
}

export async function getWalletTransactions(
  address: string,
  limit = 20,
): Promise<WalletTransaction[]> {
  const q = new URLSearchParams({ address, limit: String(limit) });
  const raw = await apiFetch<unknown>(`/wallet/transactions?${q.toString()}`);
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw as WalletTransaction[];
}

export { API_BASE };
