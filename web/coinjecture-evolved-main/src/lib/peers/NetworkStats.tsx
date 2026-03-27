// Network health summary bar

interface NetworkStatsProps {
  peerCount: number;
  chainId?: string;
  bestHeight?: number;
  isSyncing?: boolean;
  listenAddresses?: string[];
}

export function NetworkStats({
  peerCount,
  chainId,
  bestHeight,
  isSyncing,
  listenAddresses,
}: NetworkStatsProps) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
      <StatCard label="Peers" value={peerCount} />
      <StatCard label="Block Height" value={bestHeight ?? '—'} />
      <StatCard
        label="Sync Status"
        value={isSyncing ? 'Syncing...' : 'Synced'}
        color={isSyncing ? 'text-amber-400' : 'text-emerald-400'}
      />
      <StatCard label="Chain" value={chainId ?? '—'} small />
      {listenAddresses && listenAddresses.length > 0 && (
        <div className="col-span-full text-xs text-zinc-500 font-mono truncate">
          Listen: {listenAddresses.join(', ')}
        </div>
      )}
    </div>
  );
}

function StatCard({
  label,
  value,
  color,
  small,
}: {
  label: string;
  value: string | number;
  color?: string;
  small?: boolean;
}) {
  return (
    <div className="rounded-lg bg-zinc-800/50 border border-zinc-700/50 p-3">
      <div className="text-xs text-zinc-500 uppercase tracking-wider">{label}</div>
      <div
        className={`font-mono font-medium mt-1 ${
          color || 'text-zinc-200'
        } ${small ? 'text-xs' : 'text-lg'}`}
      >
        {value}
      </div>
    </div>
  );
}
