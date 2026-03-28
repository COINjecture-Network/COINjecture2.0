export type AuthMethod = 'wallet' | 'email' | 'magic_link';

export interface AuthState {
  isAuthenticated: boolean;
  isLoading: boolean;
  token: string | null;
  user: AuthUser | null;
  walletConnected: boolean;
  walletAddress: string | null;
  emailVerified: boolean;
  email: string | null;
  authMethod: AuthMethod | null;
  isFullyLinked: boolean;
}

export interface AuthUser {
  id: string;
  email: string | null;
  wallet_address: string | null;
  display_name: string | null;
  created_at: string;
}

export interface AuthActions {
  signInWithWallet: () => Promise<void>;
  signUpWithEmail: (email: string, password: string) => Promise<{ needsConfirmation: boolean }>;
  signInWithEmail: (email: string, password: string) => Promise<void>;
  requestMagicLink: (email: string) => Promise<void>;
  bindWallet: () => Promise<void>;
  signOut: () => void;
  refreshSession: () => Promise<void>;
  openAuthModal: (tab?: 'wallet' | 'email') => void;
  closeAuthModal: () => void;
}

export type AuthContextType = AuthState & AuthActions & {
  authModalOpen: boolean;
  authModalTab: 'wallet' | 'email';
};
