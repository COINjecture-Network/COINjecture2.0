// Unified auth system — re-exports for convenient imports
//
//   import { AuthProvider, useAuth, UserMenu, AuthModal } from '@/lib/auth';

export { AuthProvider } from './AuthProvider';
export { useAuth } from './useAuth';
export { AuthModal } from './components/AuthModal';
export { UserMenu } from './components/UserMenu';
export { EmailSignupForm } from './components/EmailSignupForm';
export { EmailSigninForm } from './components/EmailSigninForm';
export { MagicLinkForm } from './components/MagicLinkForm';
export { BindWalletCard } from './components/BindWalletCard';
export type { AuthContextType, AuthState, AuthActions, AuthUser, AuthMethod } from './types';
