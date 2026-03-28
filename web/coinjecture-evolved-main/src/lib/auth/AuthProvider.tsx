import { createContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { useWallet } from '@/contexts/WalletContext';
import { signMessage } from '@/lib/wallet-crypto';
import { performSiwbAuth, getMe } from '@/lib/wallet-adapter/siwb';
import type { AuthContextType, AuthMethod, AuthUser } from './types';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3030';
const TOKEN_KEY = 'coinjecture:auth_token';
const METHOD_KEY = 'coinjecture:auth_method';

export const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const { accounts, selectedAccount } = useWallet();

  const [token, setToken] = useState<string | null>(null);
  const [user, setUser] = useState<AuthUser | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [authMethod, setAuthMethod] = useState<AuthMethod | null>(null);
  const [authModalOpen, setAuthModalOpen] = useState(false);
  const [authModalTab, setAuthModalTab] = useState<'wallet' | 'email'>('wallet');

  // Restore session on mount
  useEffect(() => {
    const stored = localStorage.getItem(TOKEN_KEY);
    if (!stored) {
      setIsLoading(false);
      return;
    }
    getMe(stored)
      .then((me) => {
        setToken(stored);
        setUser({
          id: me.wallet_address || '',
          email: (me as any).email || null,
          wallet_address: me.wallet_address || null,
          display_name: null,
          created_at: me.issued_at || '',
        });
        setAuthMethod(
          (localStorage.getItem(METHOD_KEY) as AuthMethod) || 'wallet',
        );
      })
      .catch(() => {
        localStorage.removeItem(TOKEN_KEY);
        localStorage.removeItem(METHOD_KEY);
      })
      .finally(() => setIsLoading(false));
  }, []);

  const persist = (tok: string, method: AuthMethod) => {
    setToken(tok);
    setAuthMethod(method);
    localStorage.setItem(TOKEN_KEY, tok);
    localStorage.setItem(METHOD_KEY, method);
  };

  // ── Wallet SIWB auth ──────────────────────────────────────────────────

  const signInWithWallet = useCallback(async () => {
    if (!selectedAccount) throw new Error('No account selected');
    const account = accounts[selectedAccount];
    if (!account) throw new Error('Account not found');

    const result = await performSiwbAuth(account.publicKey, async (msg) =>
      signMessage(msg, account.privateKey),
    );

    persist(result.token, 'wallet');
    setUser({
      id: result.user.id,
      email: null,
      wallet_address: result.user.wallet_address,
      display_name: null,
      created_at: new Date().toISOString(),
    });
    setAuthModalOpen(false);
  }, [accounts, selectedAccount]);

  // ── Email auth ────────────────────────────────────────────────────────

  const signUpWithEmail = useCallback(
    async (email: string, password: string) => {
      const res = await fetch(`${API_BASE}/auth/email/signup`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data?.error?.message || 'Signup failed');

      if (data.token) {
        persist(data.token, 'email');
        setUser({
          id: data.user?.id || '',
          email,
          wallet_address: null,
          display_name: null,
          created_at: new Date().toISOString(),
        });
        return { needsConfirmation: false };
      }
      return { needsConfirmation: true };
    },
    [],
  );

  const signInWithEmail = useCallback(
    async (email: string, password: string) => {
      const res = await fetch(`${API_BASE}/auth/email/signin`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data?.error?.message || 'Sign-in failed');

      persist(data.token, 'email');
      setUser({
        id: data.user?.id || '',
        email: data.user?.email || email,
        wallet_address: data.user?.wallet_address || null,
        display_name: null,
        created_at: new Date().toISOString(),
      });
      setAuthModalOpen(false);
    },
    [],
  );

  const requestMagicLink = useCallback(async (email: string) => {
    const res = await fetch(`${API_BASE}/auth/email/magic-link`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    });
    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      throw new Error(data?.error?.message || 'Failed to send magic link');
    }
  }, []);

  // ── Wallet binding (for email users) ──────────────────────────────────

  const bindWallet = useCallback(async () => {
    if (!token || !user) throw new Error('Not authenticated');
    if (!selectedAccount) throw new Error('No wallet account selected');
    const account = accounts[selectedAccount];
    if (!account) throw new Error('Account not found');

    const timestamp = new Date().toISOString();
    const message = `I authorize binding wallet ${account.publicKey} to my COINjecture account.\n\nUser: ${user.id}\nTimestamp: ${timestamp}`;
    const signature = signMessage(new TextEncoder().encode(message), account.privateKey);

    const res = await fetch(`${API_BASE}/auth/email/bind-wallet`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({
        wallet_address: account.publicKey,
        signature,
        message,
      }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data?.error?.message || 'Binding failed');

    persist(data.token, authMethod || 'email');
    setUser((prev) =>
      prev ? { ...prev, wallet_address: data.wallet_address } : prev,
    );
  }, [token, user, accounts, selectedAccount, authMethod]);

  // ── Session ───────────────────────────────────────────────────────────

  const signOut = useCallback(() => {
    setToken(null);
    setUser(null);
    setAuthMethod(null);
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(METHOD_KEY);
  }, []);

  const refreshSession = useCallback(async () => {
    if (!token) return;
    try {
      const me = await getMe(token);
      setUser((prev) =>
        prev
          ? { ...prev, wallet_address: me.wallet_address || null }
          : prev,
      );
    } catch {
      signOut();
    }
  }, [token, signOut]);

  const openAuthModal = useCallback((tab: 'wallet' | 'email' = 'wallet') => {
    setAuthModalTab(tab);
    setAuthModalOpen(true);
  }, []);

  const closeAuthModal = useCallback(() => setAuthModalOpen(false), []);

  // ── Derived state ─────────────────────────────────────────────────────

  const account = selectedAccount ? accounts[selectedAccount] : null;

  const ctx: AuthContextType = {
    isAuthenticated: !!token && !!user,
    isLoading,
    token,
    user,
    walletConnected: !!account,
    walletAddress: account?.publicKey || null,
    emailVerified: !!user?.email,
    email: user?.email || null,
    authMethod,
    isFullyLinked: !!user?.email && !!user?.wallet_address,
    signInWithWallet,
    signUpWithEmail,
    signInWithEmail,
    requestMagicLink,
    bindWallet,
    signOut,
    refreshSession,
    openAuthModal,
    closeAuthModal,
    authModalOpen,
    authModalTab,
  };

  return <AuthContext.Provider value={ctx}>{children}</AuthContext.Provider>;
}
