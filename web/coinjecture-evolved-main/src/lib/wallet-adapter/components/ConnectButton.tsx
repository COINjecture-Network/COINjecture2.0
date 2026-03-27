// SIWB Connect / Sign-In button component
// Works alongside the existing WalletContext — does NOT replace it.
//
// States:
//   1. No account selected         → "Select Account"
//   2. Account selected, no JWT    → "Sign In"
//   3. Signing in                  → spinner
//   4. Authenticated               → truncated address + green dot + dropdown

import { useState, useRef, useEffect } from 'react';
import { useWallet } from '@/contexts/WalletContext';
import { useSiwbAuth } from '../hooks';

function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export function SiwbConnectButton() {
  const { selectedAccount, accounts } = useWallet();
  const { isAuthenticated, user, signingIn, error, signIn, signOut } =
    useSiwbAuth();

  const [dropdownOpen, setDropdownOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, []);

  const account = selectedAccount ? accounts[selectedAccount] : null;

  // State 1: No account selected
  if (!account) {
    return (
      <button
        className="rounded-lg bg-zinc-800 px-4 py-2 text-sm font-medium text-zinc-400
                   border border-zinc-700 cursor-default"
        disabled
      >
        Select Account
      </button>
    );
  }

  // State 3: Signing in
  if (signingIn) {
    return (
      <button
        className="rounded-lg bg-emerald-900/50 px-4 py-2 text-sm font-medium text-emerald-300
                   border border-emerald-700/50 flex items-center gap-2"
        disabled
      >
        <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-emerald-400 border-t-transparent" />
        Signing in...
      </button>
    );
  }

  // State 2: Account selected but not authenticated
  if (!isAuthenticated) {
    return (
      <div className="flex flex-col items-end gap-1">
        <button
          onClick={signIn}
          className="rounded-lg bg-emerald-600 hover:bg-emerald-500 px-4 py-2 text-sm
                     font-medium text-white transition-colors"
        >
          Sign In
        </button>
        {error && (
          <span className="text-xs text-red-400 max-w-[200px] truncate">
            {error}
          </span>
        )}
        <div className="text-[10px] text-amber-500/80 font-mono">
          Development Wallet — Not for mainnet use
        </div>
      </div>
    );
  }

  // State 4: Authenticated
  const displayAddr = user?.wallet_address || account.publicKey;

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setDropdownOpen(!dropdownOpen)}
        className="rounded-lg bg-zinc-800 hover:bg-zinc-700 px-4 py-2 text-sm font-medium
                   text-zinc-200 border border-zinc-600 flex items-center gap-2 transition-colors"
      >
        <span className="h-2 w-2 rounded-full bg-emerald-400" />
        <span className="font-mono">{truncateAddress(displayAddr)}</span>
      </button>

      {dropdownOpen && (
        <div
          className="absolute right-0 top-full mt-1 w-48 rounded-lg bg-zinc-800 border
                     border-zinc-700 shadow-lg py-1 z-50"
        >
          <button
            onClick={() => {
              navigator.clipboard.writeText(displayAddr);
              setDropdownOpen(false);
            }}
            className="w-full px-3 py-2 text-left text-sm text-zinc-300 hover:bg-zinc-700
                       transition-colors"
          >
            Copy Address
          </button>
          <button
            onClick={() => {
              signOut();
              setDropdownOpen(false);
            }}
            className="w-full px-3 py-2 text-left text-sm text-red-400 hover:bg-zinc-700
                       transition-colors"
          >
            Sign Out
          </button>
        </div>
      )}
    </div>
  );
}
