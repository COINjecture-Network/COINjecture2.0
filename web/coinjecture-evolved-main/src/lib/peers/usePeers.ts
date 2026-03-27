// Hook for fetching peer data from the admin API.
// Auto-refreshes every 10 seconds.

import { useState, useEffect, useCallback } from 'react';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3030';

export interface PeerData {
  network: {
    peer_id?: string;
    peer_count?: number;
    listen_addresses?: string[];
  } | null;
  chain: {
    chain_id?: string;
    best_height?: number;
    best_hash?: string;
    is_syncing?: boolean;
  } | null;
  error: string | null;
  isLoading: boolean;
}

export function usePeers(refreshInterval = 10_000) {
  const [data, setData] = useState<PeerData>({
    network: null,
    chain: null,
    error: null,
    isLoading: true,
  });

  const fetchPeers = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/admin/peers`);
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || `HTTP ${res.status}`);
      }
      const json = await res.json();
      setData({
        network: json.network || null,
        chain: json.chain || null,
        error: null,
        isLoading: false,
      });
    } catch (err: any) {
      setData((prev) => ({
        ...prev,
        error: err.message || 'Failed to fetch peers',
        isLoading: false,
      }));
    }
  }, []);

  useEffect(() => {
    fetchPeers();
    const id = setInterval(fetchPeers, refreshInterval);
    return () => clearInterval(id);
  }, [fetchPeers, refreshInterval]);

  const addPeer = useCallback(async (address: string) => {
    const res = await fetch(`${API_BASE}/admin/peers/add`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ address, connect_now: true }),
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body?.error?.message || 'Failed to add peer');
    }
    await fetchPeers();
    return res.json();
  }, [fetchPeers]);

  const removePeer = useCallback(async (address: string, banHours = 0) => {
    const res = await fetch(`${API_BASE}/admin/peers/remove`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ address, ban_hours: banHours }),
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body?.error?.message || 'Failed to remove peer');
    }
    await fetchPeers();
    return res.json();
  }, [fetchPeers]);

  return { ...data, addPeer, removePeer, refresh: fetchPeers };
}
