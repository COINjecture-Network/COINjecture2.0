// Wallet adapter — SIWB auth integration for COINjecture
//
// Re-exports for convenient imports:
//   import { useSiwbAuth, SiwbConnectButton } from '@/lib/wallet-adapter';

export { useSiwbAuth } from './hooks';
export { SiwbConnectButton } from './components/ConnectButton';
export { requestChallenge, verifySignature, getMe, performSiwbAuth } from './siwb';
export type {
  WalletAdapter,
  WalletContextState,
  AuthUser,
  SiwbChallenge,
  SiwbVerifyResponse,
  AuthMeResponse,
} from './types';
