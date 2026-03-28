import { useState, useRef, useEffect } from 'react';
import { useAuth } from '../useAuth';

function truncate(s: string, head = 6, tail = 4): string {
  if (s.length <= head + tail + 3) return s;
  return `${s.slice(0, head)}...${s.slice(-tail)}`;
}

export function UserMenu() {
  const {
    isAuthenticated,
    isLoading,
    user,
    authMethod,
    isFullyLinked,
    signOut,
    openAuthModal,
  } = useAuth();

  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  if (isLoading) {
    return (
      <div className="rounded-lg bg-zinc-800 px-4 py-2 text-sm text-zinc-500">
        Loading...
      </div>
    );
  }

  // Unauthenticated
  if (!isAuthenticated) {
    return (
      <button
        onClick={() => openAuthModal('wallet')}
        className="rounded-lg bg-emerald-600 hover:bg-emerald-500 px-4 py-2 text-sm
                   font-medium text-white transition-colors"
      >
        Sign In
      </button>
    );
  }

  // Status dot color
  const dotColor = isFullyLinked
    ? 'bg-blue-400'           // email + wallet
    : user?.wallet_address
    ? 'bg-emerald-400'        // wallet only
    : 'bg-amber-400';         // email only

  const displayLabel = user?.wallet_address
    ? truncate(user.wallet_address)
    : user?.email
    ? truncate(user.email, 10, 0)
    : 'Signed In';

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="rounded-lg bg-zinc-800 hover:bg-zinc-700 px-4 py-2 text-sm font-medium
                   text-zinc-200 border border-zinc-600 flex items-center gap-2 transition-colors"
      >
        <span className={`h-2 w-2 rounded-full ${dotColor}`} />
        <span className="font-mono">{displayLabel}</span>
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-52 rounded-lg bg-zinc-800 border
                        border-zinc-700 shadow-lg py-1 z-50">
          {/* Copy address (wallet users) */}
          {user?.wallet_address && (
            <button
              onClick={() => {
                navigator.clipboard.writeText(user.wallet_address!);
                setOpen(false);
              }}
              className="w-full px-3 py-2 text-left text-sm text-zinc-300 hover:bg-zinc-700
                         transition-colors"
            >
              Copy Address
            </button>
          )}

          {/* Link wallet (email-only users) */}
          {!user?.wallet_address && authMethod === 'email' && (
            <button
              onClick={() => {
                openAuthModal('wallet');
                setOpen(false);
              }}
              className="w-full px-3 py-2 text-left text-sm text-amber-400 hover:bg-zinc-700
                         transition-colors"
            >
              Link Wallet
            </button>
          )}

          {/* Add email (wallet-only users) */}
          {!user?.email && authMethod === 'wallet' && (
            <button
              onClick={() => {
                openAuthModal('email');
                setOpen(false);
              }}
              className="w-full px-3 py-2 text-left text-sm text-zinc-300 hover:bg-zinc-700
                         transition-colors"
            >
              Add Email
            </button>
          )}

          <hr className="border-zinc-700 my-1" />

          <button
            onClick={() => {
              signOut();
              setOpen(false);
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
