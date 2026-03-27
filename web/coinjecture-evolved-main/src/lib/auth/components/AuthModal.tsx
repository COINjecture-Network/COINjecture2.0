import { useState } from 'react';
import { useAuth } from '../useAuth';
import { useWallet } from '@/contexts/WalletContext';
import { EmailSignupForm } from './EmailSignupForm';
import { EmailSigninForm } from './EmailSigninForm';
import { MagicLinkForm } from './MagicLinkForm';
import { BindWalletCard } from './BindWalletCard';

export function AuthModal() {
  const { authModalOpen, closeAuthModal, authModalTab, signInWithWallet, isAuthenticated } =
    useAuth();
  const { selectedAccount } = useWallet();

  const [tab, setTab] = useState<'wallet' | 'email'>(authModalTab);
  const [emailMode, setEmailMode] = useState<'signin' | 'signup' | 'magic'>('signin');
  const [walletLoading, setWalletLoading] = useState(false);
  const [walletError, setWalletError] = useState<string | null>(null);

  // Sync tab when modal opens
  if (authModalOpen && tab !== authModalTab) setTab(authModalTab);

  if (!authModalOpen) return null;

  const handleWalletSignIn = async () => {
    setWalletLoading(true);
    setWalletError(null);
    try {
      await signInWithWallet();
    } catch (err: any) {
      setWalletError(err.message || 'Wallet sign-in failed');
    } finally {
      setWalletLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={closeAuthModal}
      />

      {/* Modal */}
      <div className="relative w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl p-6">
        <button
          onClick={closeAuthModal}
          className="absolute top-3 right-3 text-zinc-500 hover:text-zinc-300 text-xl leading-none"
        >
          &times;
        </button>

        <h2 className="text-lg font-semibold text-zinc-100 mb-4">Sign In to COINjecture</h2>

        {/* Tabs */}
        <div className="flex gap-1 mb-4 rounded-lg bg-zinc-800 p-1">
          <button
            onClick={() => setTab('wallet')}
            className={`flex-1 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
              tab === 'wallet'
                ? 'bg-zinc-700 text-zinc-100'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            Wallet
          </button>
          <button
            onClick={() => setTab('email')}
            className={`flex-1 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
              tab === 'email'
                ? 'bg-zinc-700 text-zinc-100'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            Email
          </button>
        </div>

        {/* Wallet tab */}
        {tab === 'wallet' && (
          <div className="space-y-3">
            {!selectedAccount ? (
              <p className="text-zinc-400 text-sm text-center py-4">
                Create or select an account on the{' '}
                <span className="text-emerald-400">Wallet</span> page first.
              </p>
            ) : (
              <>
                <button
                  onClick={handleWalletSignIn}
                  disabled={walletLoading}
                  className="w-full rounded-lg bg-emerald-600 hover:bg-emerald-500 px-4 py-2.5
                             text-sm font-medium text-white transition-colors disabled:opacity-50
                             flex items-center justify-center gap-2"
                >
                  {walletLoading && (
                    <span className="h-3 w-3 animate-spin rounded-full border-2 border-white border-t-transparent" />
                  )}
                  Sign In with Wallet
                </button>
                {walletError && (
                  <p className="text-red-400 text-xs text-center">{walletError}</p>
                )}
                <p className="text-[10px] text-amber-500/80 text-center">
                  Development Wallet — Not for mainnet use
                </p>
              </>
            )}
          </div>
        )}

        {/* Email tab */}
        {tab === 'email' && (
          <div className="space-y-3">
            {emailMode === 'signin' && (
              <>
                <EmailSigninForm onSuccess={closeAuthModal} />
                <div className="flex items-center justify-between text-xs text-zinc-500">
                  <button
                    onClick={() => setEmailMode('signup')}
                    className="hover:text-zinc-300 transition-colors"
                  >
                    Create account
                  </button>
                  <button
                    onClick={() => setEmailMode('magic')}
                    className="hover:text-zinc-300 transition-colors"
                  >
                    Use magic link
                  </button>
                </div>
              </>
            )}
            {emailMode === 'signup' && (
              <>
                <EmailSignupForm onSuccess={closeAuthModal} />
                <button
                  onClick={() => setEmailMode('signin')}
                  className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                >
                  Already have an account? Sign in
                </button>
              </>
            )}
            {emailMode === 'magic' && (
              <>
                <MagicLinkForm />
                <button
                  onClick={() => setEmailMode('signin')}
                  className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                >
                  Use password instead
                </button>
              </>
            )}
          </div>
        )}

        {/* Bind wallet card (shown after email auth) */}
        {isAuthenticated && <div className="mt-4"><BindWalletCard /></div>}
      </div>
    </div>
  );
}
