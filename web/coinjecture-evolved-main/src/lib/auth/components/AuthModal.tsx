import { useState, useEffect } from 'react';
import { Mail, UserPlus, Wallet, Sparkles } from 'lucide-react';
import { useAuth } from '../useAuth';
import { useWallet } from '@/contexts/WalletContext';
import { EmailSignupForm } from './EmailSignupForm';
import { EmailSigninForm } from './EmailSigninForm';
import { MagicLinkForm } from './MagicLinkForm';
import { BindWalletCard } from './BindWalletCard';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { AuthModalEmailSection } from '../types';

type Phase = 'welcome' | 'wallet' | 'email';

export function AuthModal() {
  const {
    authModalOpen,
    closeAuthModal,
    authModalTab,
    authModalEmailSection,
    signInWithWallet,
    isAuthenticated,
  } = useAuth();
  const { selectedAccount } = useWallet();

  const [phase, setPhase] = useState<Phase>('welcome');
  const [emailMode, setEmailMode] = useState<AuthModalEmailSection>('signin');
  const [walletLoading, setWalletLoading] = useState(false);
  const [walletError, setWalletError] = useState<string | null>(null);

  useEffect(() => {
    if (!authModalOpen) return;
    if (authModalTab === 'welcome') {
      setPhase('welcome');
    } else if (authModalTab === 'wallet') {
      setPhase('wallet');
    } else {
      setPhase('email');
      setEmailMode(authModalEmailSection);
    }
  }, [authModalOpen, authModalTab, authModalEmailSection]);

  const handleWalletSignIn = async () => {
    setWalletLoading(true);
    setWalletError(null);
    try {
      await signInWithWallet();
    } catch (err: unknown) {
      setWalletError(err instanceof Error ? err.message : 'Wallet sign-in failed');
    } finally {
      setWalletLoading(false);
    }
  };

  const showBack = phase !== 'welcome';

  const emailTitle =
    emailMode === 'signup'
      ? 'Create your account'
      : emailMode === 'magic'
        ? 'Magic link sign-in'
        : 'Sign in with email';

  return (
    <Dialog
      open={authModalOpen}
      onOpenChange={(open) => {
        if (!open) closeAuthModal();
      }}
    >
      <DialogContent
        className={cn(
          'sm:max-w-[440px] p-0 gap-0 overflow-hidden',
          'border-border/60 bg-background/95 backdrop-blur-xl shadow-2xl',
        )}
      >
        <div className="max-h-[min(85vh,720px)] overflow-y-auto p-6 pt-8">
          {showBack && (
            <button
              type="button"
              onClick={() => setPhase('welcome')}
              className="mb-4 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              ← All sign-in options
            </button>
          )}

          {phase === 'welcome' && (
            <>
              <DialogHeader className="space-y-3 text-center sm:text-center pr-8">
                <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
                  <Sparkles className="h-6 w-6" aria-hidden />
                </div>
                <DialogTitle className="font-brand text-xl sm:text-2xl">
                  Welcome to COINjecture
                </DialogTitle>
                <DialogDescription className="text-base">
                  Sign in or create an account.
                </DialogDescription>
              </DialogHeader>

              <div className="mt-6 grid gap-2">
                <Button
                  type="button"
                  variant="default"
                  size="lg"
                  className="h-auto justify-start gap-3 py-3 px-4 glow-hover gentle-animation"
                  onClick={() => {
                    setPhase('email');
                    setEmailMode('signin');
                  }}
                >
                  <Mail className="h-5 w-5 shrink-0 opacity-90" />
                  <span className="text-left">
                    <span className="block font-medium">Sign in with email</span>
                    <span className="block text-xs font-normal opacity-90">
                      Password sign-in
                    </span>
                  </span>
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="lg"
                  className="h-auto justify-start gap-3 py-3 px-4 border-border/80 bg-background/50"
                  onClick={() => {
                    setPhase('email');
                    setEmailMode('signup');
                  }}
                >
                  <UserPlus className="h-5 w-5 shrink-0 opacity-90" />
                  <span className="text-left">
                    <span className="block font-medium">Create an account</span>
                    <span className="block text-xs font-normal text-muted-foreground">
                      New email & password
                    </span>
                  </span>
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="lg"
                  className="h-auto justify-start gap-3 py-3 px-4 border-border/80 bg-background/50"
                  onClick={() => setPhase('wallet')}
                >
                  <Wallet className="h-5 w-5 shrink-0 opacity-90" />
                  <span className="text-left">
                    <span className="block font-medium">Sign in with wallet</span>
                    <span className="block text-xs font-normal text-muted-foreground">
                      Dev wallet / SIWB
                    </span>
                  </span>
                </Button>
              </div>

              <p className="mt-5 text-center text-xs text-muted-foreground">
                <button
                  type="button"
                  className="underline underline-offset-2 hover:text-foreground"
                  onClick={() => {
                    setPhase('email');
                    setEmailMode('magic');
                  }}
                >
                  Prefer a magic link?
                </button>
              </p>
            </>
          )}

          {phase === 'email' && (
            <>
              <DialogHeader className="pr-8">
                <DialogTitle>{emailTitle}</DialogTitle>
                <DialogDescription>
                  {emailMode === 'signup' &&
                    'Choose a password (min. 8 characters). We may email you to confirm.'}
                  {emailMode === 'signin' && 'Use the email and password for your account.'}
                  {emailMode === 'magic' &&
                    'We’ll email you a one-time link if this address is registered.'}
                </DialogDescription>
              </DialogHeader>
              <div className="mt-4 space-y-4">
                {emailMode === 'signin' && (
                  <>
                    <EmailSigninForm onSuccess={closeAuthModal} />
                    <div className="flex flex-wrap justify-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
                      <button
                        type="button"
                        className="underline underline-offset-2 hover:text-foreground"
                        onClick={() => setEmailMode('signup')}
                      >
                        Create account
                      </button>
                      <button
                        type="button"
                        className="underline underline-offset-2 hover:text-foreground"
                        onClick={() => setEmailMode('magic')}
                      >
                        Magic link instead
                      </button>
                    </div>
                  </>
                )}
                {emailMode === 'signup' && (
                  <>
                    <EmailSignupForm onSuccess={closeAuthModal} />
                    <button
                      type="button"
                      className="text-xs text-muted-foreground hover:text-foreground underline underline-offset-2"
                      onClick={() => setEmailMode('signin')}
                    >
                      Already have an account? Sign in
                    </button>
                  </>
                )}
                {emailMode === 'magic' && (
                  <>
                    <MagicLinkForm />
                    <button
                      type="button"
                      className="text-xs text-muted-foreground hover:text-foreground underline underline-offset-2"
                      onClick={() => setEmailMode('signin')}
                    >
                      Use password instead
                    </button>
                  </>
                )}
              </div>
            </>
          )}

          {phase === 'wallet' && (
            <>
              <DialogHeader className="pr-8">
                <DialogTitle>Sign in with wallet</DialogTitle>
                <DialogDescription>
                  Use your COINjecture dev wallet to sign a challenge and get a session.
                </DialogDescription>
              </DialogHeader>
              <div className="mt-4 space-y-3">
                {!selectedAccount ? (
                  <p className="text-sm text-muted-foreground text-center py-4">
                    Create or select an account on the{' '}
                    <span className="text-primary font-medium">Wallet</span> page first.
                  </p>
                ) : (
                  <>
                    <Button
                      type="button"
                      onClick={handleWalletSignIn}
                      disabled={walletLoading}
                      className="w-full glow-hover gentle-animation"
                      size="lg"
                    >
                      {walletLoading && (
                        <span className="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary-foreground border-t-transparent" />
                      )}
                      Sign in with wallet
                    </Button>
                    {walletError && (
                      <p className="text-destructive text-xs text-center">{walletError}</p>
                    )}
                    <p className="text-[10px] text-center text-amber-600/90 dark:text-amber-500/80">
                      Development wallet — not for mainnet use
                    </p>
                  </>
                )}
              </div>
            </>
          )}

          {isAuthenticated && (
            <div className="mt-6 border-t border-border/60 pt-6">
              <BindWalletCard />
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
