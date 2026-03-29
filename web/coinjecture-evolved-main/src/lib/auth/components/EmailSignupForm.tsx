import { useState } from 'react';
import { useAuth } from '../useAuth';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

export function EmailSignupForm({ onSuccess }: { onSuccess?: () => void }) {
  const { signUpWithEmail } = useAuth();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsConfirmation, setNeedsConfirmation] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      const result = await signUpWithEmail(email, password);
      if (result.needsConfirmation) {
        setNeedsConfirmation(true);
      } else {
        onSuccess?.();
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Signup failed');
    } finally {
      setLoading(false);
    }
  };

  if (needsConfirmation) {
    return (
      <div className="text-center py-4">
        <p className="text-primary font-medium">Check your email</p>
        <p className="text-muted-foreground text-sm mt-2">
          We sent a confirmation link to <strong className="text-foreground">{email}</strong>
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
      <Input
        type="password"
        placeholder="Password (min 8 characters)"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        minLength={8}
        autoComplete="new-password"
        required
      />
      {error && <p className="text-destructive text-xs">{error}</p>}
      <Button type="submit" disabled={loading} className="w-full">
        {loading ? 'Creating account…' : 'Create account'}
      </Button>
    </form>
  );
}
