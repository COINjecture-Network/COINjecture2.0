// Core types for the SIWB (Sign-In With BEANS) wallet adapter

export interface WalletAdapter {
  publicKey: Uint8Array | null;
  address: string | null; // hex-encoded
  connected: boolean;
  connecting: boolean;

  connect(): Promise<void>;
  disconnect(): Promise<void>;
  signMessage(message: Uint8Array): Promise<Uint8Array>;
}

export interface WalletContextState {
  wallet: WalletAdapter | null;
  publicKey: Uint8Array | null;
  address: string | null;
  connected: boolean;
  connecting: boolean;
  token: string | null; // JWT after SIWB auth
  user: AuthUser | null;

  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  signIn: () => Promise<void>; // Full SIWB flow
}

export interface AuthUser {
  id: string;
  wallet_address: string;
}

export interface SiwbChallenge {
  message: string;
  nonce: string;
}

export interface SiwbVerifyResponse {
  token: string;
  user: AuthUser;
}

export interface AuthMeResponse {
  wallet_address: string;
  network: string;
  issued_at: string | null;
  expires_at: string | null;
}
