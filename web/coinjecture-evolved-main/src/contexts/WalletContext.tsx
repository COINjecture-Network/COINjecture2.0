import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { KeyPair, KeyStore, generateKeyPair, importKeyPair } from '@/lib/wallet-crypto';

interface WalletContextType {
  accounts: Record<string, KeyPair>;
  selectedAccount: string | null;
  setSelectedAccount: (name: string | null) => void;
  createAccount: (name: string) => KeyPair;
  importAccount: (name: string, privateKey: string) => KeyPair;
  deleteAccount: (name: string) => void;
  refreshAccounts: () => void;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export function WalletProvider({ children }: { children: ReactNode }) {
  const [accounts, setAccounts] = useState<Record<string, KeyPair>>({});
  const [selectedAccount, setSelectedAccount] = useState<string | null>(null);

  const refreshAccounts = () => {
    const loaded = KeyStore.list();
    setAccounts(loaded);
    // If selected account was deleted, clear selection
    if (selectedAccount && !loaded[selectedAccount]) {
      setSelectedAccount(null);
    }
  };

  useEffect(() => {
    refreshAccounts();
  }, []);

  const createAccount = (name: string): KeyPair => {
    if (accounts[name]) {
      throw new Error('Account name already exists');
    }
    const keyPair = generateKeyPair();
    KeyStore.save(name, keyPair);
    refreshAccounts();
    return keyPair;
  };

  const importAccount = (name: string, privateKey: string): KeyPair => {
    if (accounts[name]) {
      throw new Error('Account name already exists');
    }
    const keyPair = importKeyPair(privateKey);
    KeyStore.save(name, keyPair);
    refreshAccounts();
    return keyPair;
  };

  const deleteAccount = (name: string) => {
    KeyStore.delete(name);
    refreshAccounts();
    if (selectedAccount === name) {
      setSelectedAccount(null);
    }
  };

  return (
    <WalletContext.Provider
      value={{
        accounts,
        selectedAccount,
        setSelectedAccount,
        createAccount,
        importAccount,
        deleteAccount,
        refreshAccounts,
      }}
    >
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet() {
  const context = useContext(WalletContext);
  if (context === undefined) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return context;
}

