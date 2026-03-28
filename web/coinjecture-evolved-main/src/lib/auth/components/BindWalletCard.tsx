import { useState } from 'react';
import { useAuth } from '../useAuth';
import { useWallet } from '@/contexts/WalletContext';

const DISMISS_KEY = 'coinjecture:bind_wallet_dismissed';

export function BindWalletCard() {
  const { isAuthenticated, user, bindWallet } = useAuth();
  const { selectedAccount } = useWallet();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [dismissed, setDismissed] = useState(
    () => localStorage.getItem(DISMISS_KEY) === 'true',
  );

  // Only show for email-authenticated users without a bound wallet
  if (!isAuthenticated || user?.wallet_address || dismissed) return null;

  const handleBind = async () => {
    if (!selectedAccount) {
      setError('Select a wallet account first (visit the Wallet page)');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await bindWallet();
      setSuccess(true);
    } catch (err: any) {
      setError(err.message || 'Failed to bind wallet');
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="rounded-lg border border-emerald-700/50 bg-emerald-900/20 p-4">
        <p className="text-emerald-400 font-medium text-sm">Wallet linked!</p>
        <p className="text-zinc-400 text-xs mt-1 font-mono">
          {user?.wallet_address?.slice(0, 6)}...{user?.wallet_address?.slice(-4)}
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-amber-700/50 bg-amber-900/20 p-4">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-amber-300 font-medium text-sm">
            Connect your COINjecture wallet for full access
          </p>
          <p className="text-zinc-400 text-xs mt-1">
            Link your wallet to trade on the marketplace, submit PoUW tasks, and earn rewards.
          </p>
        </div>
        <button
          onClick={() => {
            setDismissed(true);
            localStorage.setItem(DISMISS_KEY, 'true');
          }}
          className="text-zinc-500 hover:text-zinc-300 text-lg leading-none ml-2"
        >
          &times;
        </button>
      </div>
      {error && <p className="text-red-400 text-xs mt-2">{error}</p>}
      <button
        onClick={handleBind}
        disabled={loading}
        className="mt-3 rounded-lg bg-amber-600 hover:bg-amber-500 px-4 py-1.5 text-sm
                   font-medium text-white transition-colors disabled:opacity-50"
      >
        {loading ? 'Linking...' : 'Connect Wallet'}
      </button>
    </div>
  );
}
