// Admin peer dashboard — shows connected peers and network health.
// Auto-refreshes every 10 seconds.

import { useState } from 'react';
import { usePeers } from './usePeers';
import { NetworkStats } from './NetworkStats';

export function PeerDashboard() {
  const { network, chain, error, isLoading, addPeer } = usePeers();
  const [addAddr, setAddAddr] = useState('');
  const [addError, setAddError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const handleAdd = async () => {
    if (!addAddr.trim()) return;
    setAdding(true);
    setAddError(null);
    try {
      await addPeer(addAddr.trim());
      setAddAddr('');
    } catch (err: any) {
      setAddError(err.message);
    } finally {
      setAdding(false);
    }
  };

  if (isLoading) {
    return (
      <div className="text-zinc-500 text-sm p-6">Loading network status...</div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-800/50 bg-red-900/20 p-4 text-sm">
        <p className="text-red-400 font-medium">Network Unavailable</p>
        <p className="text-zinc-400 text-xs mt-1">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-zinc-100">Network Peers</h2>

      <NetworkStats
        peerCount={network?.peer_count ?? 0}
        chainId={chain?.chain_id}
        bestHeight={chain?.best_height}
        isSyncing={chain?.is_syncing}
        listenAddresses={network?.listen_addresses}
      />

      {/* Add Peer */}
      <div className="flex gap-2">
        <input
          type="text"
          placeholder="host:port (e.g., 1.2.3.4:707)"
          value={addAddr}
          onChange={(e) => setAddAddr(e.target.value)}
          className="flex-1 rounded-lg bg-zinc-900 border border-zinc-700 px-3 py-2 text-sm
                     text-zinc-200 placeholder-zinc-500 focus:border-emerald-500 focus:outline-none
                     font-mono"
          onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
        />
        <button
          onClick={handleAdd}
          disabled={adding || !addAddr.trim()}
          className="rounded-lg bg-emerald-600 hover:bg-emerald-500 px-4 py-2 text-sm
                     font-medium text-white transition-colors disabled:opacity-50"
        >
          {adding ? 'Adding...' : 'Add Peer'}
        </button>
      </div>
      {addError && <p className="text-red-400 text-xs">{addError}</p>}

      <p className="text-xs text-zinc-500">
        Detailed peer list with latency, reputation, and per-peer actions coming in Phase 3.
      </p>
    </div>
  );
}
