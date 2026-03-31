import { useState } from 'react';
import { useAuth } from '../useAuth';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

export function MagicLinkForm() {
  const { requestMagicLink } = useAuth();
  const [email, setEmail] = useState('');
  const [loading, setLoading] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      await requestMagicLink(email);
      setSent(true);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to send magic link');
    } finally {
      setLoading(false);
    }
  };

  if (sent) {
    return (
      <div className="text-center py-4">
        <p className="text-primary font-medium">Check your email</p>
        <p className="text-muted-foreground text-sm mt-2">
          If an account exists for <strong className="text-foreground">{email}</strong>, we sent a
          sign-in link.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <Input
        type="email"
        placeholder="Email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        autoComplete="email"
        required
      />
      {error && <p className="text-destructive text-xs">{error}</p>}
      <Button type="submit" variant="secondary" disabled={loading} className="w-full">
        {loading ? 'Sending…' : 'Send magic link'}
      </Button>
    </form>
  );
}
