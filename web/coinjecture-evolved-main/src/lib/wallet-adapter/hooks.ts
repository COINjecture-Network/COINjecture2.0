// useSiwbAuth — hook that layers SIWB authentication on top of the existing WalletContext

import { useState, useCallback, useEffect } from 'react';
import { useWallet } from '@/contexts/WalletContext';
import { signMessage as cryptoSign } from '@/lib/wallet-crypto';
import { performSiwbAuth, getMe } from './siwb';
import type { AuthUser } from './types';

const TOKEN_KEY = 'coinjecture:siwb_token';

interface SiwbAuthState {
  /** JWT token (null until signed in) */
  token: string | null;
  /** Authenticated user info */
  user: AuthUser | null;
  /** True while the SIWB flow is in progress */
  signingIn: boolean;
  /** Last error from the SIWB flow */
  error: string | null;
  /** Run the full SIWB challenge → sign → verify flow */
  signIn: () => Promise<void>;
  /** Clear the JWT and user state */
  signOut: () => void;
  /** True when the user has a valid JWT */
  isAuthenticated: boolean;
}

/**
 * Hook that provides SIWB authentication state and actions.
 *
 * Requires the component to be inside a `<WalletProvider>` and to have a
 * selected account. Call `signIn()` to run the full flow.
 */
export function useSiwbAuth(): SiwbAuthState {
  const { accounts, selectedAccount } = useWallet();
  const [token, setToken] = useState<string | null>(() => {
    try {
      return localStorage.getItem(TOKEN_KEY);
    } catch {
      return null;
    }
  });
  const [user, setUser] = useState<AuthUser | null>(null);
  const [signingIn, setSigningIn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // On mount, validate an existing token
  useEffect(() => {
    if (!token) return;
    getMe(token)
      .then((me) =>
        setUser({ id: me.wallet_address, wallet_address: me.wallet_address }),
      )
      .catch(() => {
        // Token expired or invalid — clear it
        setToken(null);
        setUser(null);
        localStorage.removeItem(TOKEN_KEY);
      });
  }, [token]);

  const signIn = useCallback(async () => {
    if (!selectedAccount) {
      setError('No wallet account selected');
      return;
    }
    const account = accounts[selectedAccount];
    if (!account) {
      setError('Selected account not found');
      return;
    }

    setSigningIn(true);
    setError(null);

    try {
      const result = await performSiwbAuth(
        account.publicKey,
        async (messageBytes: Uint8Array) => {
          // Sign the message using the existing wallet-crypto utility
          return cryptoSign(messageBytes, account.privateKey);
        },
      );

      setToken(result.token);
      setUser(result.user);

      try {
        localStorage.setItem(TOKEN_KEY, result.token);
      } catch {
        // localStorage may be unavailable in some contexts
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Sign-in failed';
      setError(msg);
      setToken(null);
      setUser(null);
    } finally {
      setSigningIn(false);
    }
  }, [accounts, selectedAccount]);

  const signOut = useCallback(() => {
    setToken(null);
    setUser(null);
    setError(null);
    localStorage.removeItem(TOKEN_KEY);
  }, []);

  return {
    token,
    user,
    signingIn,
    error,
    signIn,
    signOut,
    isAuthenticated: !!token && !!user,
  };
}
