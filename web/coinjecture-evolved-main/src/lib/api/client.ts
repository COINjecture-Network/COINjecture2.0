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

/** Unified activity row from `GET /wallet/transactions` (signed txs, receives, mining, marketplace). */
/** Older APIs return flat `block_transactions` rows without `kind` / `label` — fields optional where needed. */
export interface WalletActivityItem {
  id?: string;
  kind?: string;
  label?: string;
  block_height?: number;
  block_timestamp?: string | null;
  tx_hash?: string | null;
  tx_index?: number | null;
  amount?: string | null;
  fee?: string | null;
  /** Truncated hex address or display string */
  counterparty?: string | null | unknown;
  tx_type?: string;
  event_type?: string | null;
  problem_id?: string | null;
  detail?: unknown;
}

export interface WalletTransactionsOptions {
  limit?: number;
  offset?: number;
}

export async function getWalletTransactions(
  address: string,
  options: WalletTransactionsOptions = {},
): Promise<WalletActivityItem[]> {
  const limit = options.limit ?? 100;
  const offset = options.offset ?? 0;
  const q = new URLSearchParams({
    address,
    limit: String(limit),
    offset: String(offset),
  });
  const res = await fetch(`${API_BASE}/wallet/transactions?${q.toString()}`, {
    headers: { Accept: 'application/json' },
  });
  // Older API builds or edge configs without `GET /wallet/transactions` return 404 — treat as empty history.
  if (res.status === 404) {
    return [];
  }
  if (!res.ok) {
    const error = await res
      .json()
      .catch(() => ({ error: { message: res.statusText } }));
    const msg: string = error.error?.message || `API error: ${res.status}`;
    if (
      res.status === 503 &&
      /supabase|indexer|db error|db_size_quota|exceed_db/i.test(msg)
    ) {
      throw new Error(
        'Indexed history is temporarily unavailable (Supabase storage limit or indexer). Recent on-chain activity may still load after the API is updated.',
      );
    }
    throw new Error(msg);
  }
  const raw: unknown = await res.json();
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw as WalletActivityItem[];
}

export { API_BASE };
