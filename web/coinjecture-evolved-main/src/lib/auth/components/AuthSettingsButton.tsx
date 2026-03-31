import { useEffect, useMemo, useState } from 'react';
import { Settings } from 'lucide-react';
import { useAuth } from '../useAuth';
import { isSupabaseConfigured, supabase } from '@/lib/supabase';
import { toast } from '@/components/ui/sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet';

type AuthSettingsButtonProps = {
  compact?: boolean;
};

function getRedirectUrl() {
  return window.location.origin;
}

function SectionHeader({ title, description }: { title: string; description: string }) {
  return (
    <div>
      <div className="font-medium">{title}</div>
      <div className="text-xs font-normal text-muted-foreground">{description}</div>
    </div>
  );
}

export function AuthSettingsButton({ compact = false }: AuthSettingsButtonProps) {
  const { email, requestMagicLink } = useAuth();
  const supabaseEnabled = isSupabaseConfigured();

  const [open, setOpen] = useState(false);
  const [loadingKey, setLoadingKey] = useState<string | null>(null);
  const [hasSupabaseSession, setHasSupabaseSession] = useState(false);

  const defaultEmail = useMemo(() => email || '', [email]);

  const [confirmEmail, setConfirmEmail] = useState(defaultEmail);
  const [inviteEmail, setInviteEmail] = useState('');
  const [magicEmail, setMagicEmail] = useState(defaultEmail);
  const [resetEmail, setResetEmail] = useState(defaultEmail);
  const [reauthEmail, setReauthEmail] = useState(defaultEmail);
  const [reauthPassword, setReauthPassword] = useState('');
  const [nextEmail, setNextEmail] = useState('');

  useEffect(() => {
    setConfirmEmail(defaultEmail);
    setMagicEmail(defaultEmail);
    setResetEmail(defaultEmail);
    setReauthEmail(defaultEmail);
  }, [defaultEmail]);

  useEffect(() => {
    if (!open || !supabase) return;

    let active = true;

    supabase.auth.getSession().then(({ data }) => {
      if (active) {
        setHasSupabaseSession(!!data.session);
      }
    });

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, session) => {
      setHasSupabaseSession(!!session);
    });

    return () => {
      active = false;
      subscription.unsubscribe();
    };
  }, [open]);

  const run = async (key: string, action: () => Promise<void>) => {
    setLoadingKey(key);
    try {
      await action();
      setOpen(false);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Request failed');
    } finally {
      setLoadingKey(null);
    }
  };

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className={
            compact
              ? 'border-border/60 bg-background/50 px-2 shrink-0'
              : 'border-border/60 bg-background/50 px-2.5 shrink-0'
          }
          aria-label="Open account settings"
          title="Account settings"
        >
          <Settings className="h-4 w-4" />
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-[min(100vw-1rem,26rem)] overflow-y-auto p-0 sm:max-w-[26rem]">
        <SheetHeader className="px-6 pt-6 pr-12 text-left">
          <SheetTitle>Account settings</SheetTitle>
          <SheetDescription>
            Manage sign-in links, email confirmation, password recovery, and sensitive account
            changes.
          </SheetDescription>
        </SheetHeader>

        <div className="px-6 pb-6">
          {!supabaseEnabled && (
            <Alert className="mb-4">
              <AlertTitle>Supabase email actions need env setup</AlertTitle>
              <AlertDescription>
                `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` are not configured in this app
                yet. Magic-link requests still work through the existing API, but confirmation
                resend, invites, password reset, reauthentication, and email change will stay
                disabled until those values are added.
              </AlertDescription>
            </Alert>
          )}

          <Accordion
            type="single"
            collapsible
            defaultValue="magic"
            className="rounded-lg border border-border/60 bg-background"
          >
            <AccordionItem value="magic" className="px-4">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader title="Magic link" description="Send a one-time sign-in link." />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('magic', async () => {
                      await requestMagicLink(magicEmail);
                      toast.success(`Magic link requested for ${magicEmail}`);
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="magic-email">Email</Label>
                    <Input
                      id="magic-email"
                      type="email"
                      value={magicEmail}
                      onChange={(e) => setMagicEmail(e.target.value)}
                      autoComplete="email"
                      required
                    />
                  </div>
                  <Button type="submit" className="w-full" disabled={loadingKey === 'magic'}>
                    {loadingKey === 'magic' ? 'Sending…' : 'Send magic link'}
                  </Button>
                </form>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="confirm" className="px-4">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader
                  title="Confirm sign up"
                  description="Resend the email confirmation link."
                />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('confirm', async () => {
                      if (!supabase) throw new Error('Supabase email auth is not configured');
                      const { error } = await supabase.auth.resend({
                        type: 'signup',
                        email: confirmEmail,
                        options: { emailRedirectTo: getRedirectUrl() },
                      });
                      if (error) throw error;
                      toast.success(`Confirmation email sent to ${confirmEmail}`);
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="confirm-email">Email</Label>
                    <Input
                      id="confirm-email"
                      type="email"
                      value={confirmEmail}
                      onChange={(e) => setConfirmEmail(e.target.value)}
                      autoComplete="email"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <Button
                    type="submit"
                    className="w-full"
                    disabled={!supabaseEnabled || loadingKey === 'confirm'}
                  >
                    {loadingKey === 'confirm' ? 'Sending…' : 'Resend confirmation'}
                  </Button>
                </form>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="invite" className="px-4">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader
                  title="Invite user"
                  description="Invite someone who does not have an account yet."
                />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('invite', async () => {
                      if (!supabase) throw new Error('Supabase email auth is not configured');
                      const { error } = await supabase.auth.signInWithOtp({
                        email: inviteEmail,
                        options: {
                          shouldCreateUser: true,
                          emailRedirectTo: getRedirectUrl(),
                          data: { invited_from: 'account_settings' },
                        },
                      });
                      if (error) throw error;
                      toast.success(`Invite email sent to ${inviteEmail}`);
                      setInviteEmail('');
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="invite-email">Email</Label>
                    <Input
                      id="invite-email"
                      type="email"
                      value={inviteEmail}
                      onChange={(e) => setInviteEmail(e.target.value)}
                      autoComplete="email"
                      placeholder="teammate@example.com"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <Button
                    type="submit"
                    variant="secondary"
                    className="w-full"
                    disabled={!supabaseEnabled || loadingKey === 'invite'}
                  >
                    {loadingKey === 'invite' ? 'Sending…' : 'Send invite'}
                  </Button>
                </form>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="reset" className="px-4">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader
                  title="Reset password"
                  description="Email a password-reset link."
                />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('reset', async () => {
                      if (!supabase) throw new Error('Supabase email auth is not configured');
                      const { error } = await supabase.auth.resetPasswordForEmail(resetEmail, {
                        redirectTo: getRedirectUrl(),
                      });
                      if (error) throw error;
                      toast.success(`Password reset email sent to ${resetEmail}`);
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="reset-email">Email</Label>
                    <Input
                      id="reset-email"
                      type="email"
                      value={resetEmail}
                      onChange={(e) => setResetEmail(e.target.value)}
                      autoComplete="email"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <Button
                    type="submit"
                    variant="secondary"
                    className="w-full"
                    disabled={!supabaseEnabled || loadingKey === 'reset'}
                  >
                    {loadingKey === 'reset' ? 'Sending…' : 'Send reset email'}
                  </Button>
                </form>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="reauth" className="px-4">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader
                  title="Reauthentication"
                  description="Re-authenticate before a sensitive change."
                />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('reauth', async () => {
                      if (!supabase) throw new Error('Supabase email auth is not configured');
                      const { error } = await supabase.auth.signInWithPassword({
                        email: reauthEmail,
                        password: reauthPassword,
                      });
                      if (error) throw error;
                      setHasSupabaseSession(true);
                      setReauthPassword('');
                      toast.success('Reauthentication complete');
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="reauth-email">Email</Label>
                    <Input
                      id="reauth-email"
                      type="email"
                      value={reauthEmail}
                      onChange={(e) => setReauthEmail(e.target.value)}
                      autoComplete="email"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="reauth-password">Password</Label>
                    <Input
                      id="reauth-password"
                      type="password"
                      value={reauthPassword}
                      onChange={(e) => setReauthPassword(e.target.value)}
                      autoComplete="current-password"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <Button
                    type="submit"
                    variant="secondary"
                    className="w-full"
                    disabled={!supabaseEnabled || loadingKey === 'reauth'}
                  >
                    {loadingKey === 'reauth' ? 'Checking…' : 'Reauthenticate'}
                  </Button>
                </form>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="change-email" className="px-4 border-b-0">
              <AccordionTrigger className="py-3 text-left text-sm hover:no-underline">
                <SectionHeader
                  title="Change email address"
                  description="Verify a new email before the change takes effect."
                />
              </AccordionTrigger>
              <AccordionContent>
                <form
                  className="space-y-3"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run('change-email', async () => {
                      if (!supabase) throw new Error('Supabase email auth is not configured');
                      if (!hasSupabaseSession) {
                        throw new Error('Reauthenticate first to change the account email');
                      }
                      const { error } = await supabase.auth.updateUser(
                        { email: nextEmail },
                        { emailRedirectTo: getRedirectUrl() },
                      );
                      if (error) throw error;
                      toast.success(`Verification email sent to ${nextEmail}`);
                      setNextEmail('');
                    });
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="next-email">New email</Label>
                    <Input
                      id="next-email"
                      type="email"
                      value={nextEmail}
                      onChange={(e) => setNextEmail(e.target.value)}
                      autoComplete="email"
                      placeholder="new-address@example.com"
                      required
                      disabled={!supabaseEnabled}
                    />
                  </div>
                  <Button
                    type="submit"
                    className="w-full"
                    disabled={
                      !supabaseEnabled || !hasSupabaseSession || loadingKey === 'change-email'
                    }
                  >
                    {loadingKey === 'change-email' ? 'Updating…' : 'Verify new email'}
                  </Button>
                  {!hasSupabaseSession && supabaseEnabled && (
                    <p className="text-xs text-muted-foreground">
                      Reauthenticate above before changing the account email.
                    </p>
                  )}
                </form>
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </SheetContent>
    </Sheet>
  );
}
