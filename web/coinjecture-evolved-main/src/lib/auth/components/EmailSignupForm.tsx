import { useState } from 'react';
import { useAuth } from '../useAuth';

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
    } catch (err: any) {
      setError(err.message || 'Signup failed');
    } finally {
      setLoading(false);
    }
  };

  if (needsConfirmation) {
    return (
      <div className="text-center py-4">
        <p className="text-emerald-400 font-medium">Check your email</p>
        <p className="text-zinc-400 text-sm mt-2">
          We sent a confirmation link to <strong>{email}</strong>
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <input
        type="email"
        placeholder="Email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        className="w-full rounded-lg bg-zinc-900 border border-zinc-700 px-3 py-2 text-sm
                   text-zinc-200 placeholder-zinc-500 focus:border-emerald-500 focus:outline-none"
        required
      />
      <input
        type="password"
        placeholder="Password (min 8 characters)"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        minLength={8}
        className="w-full rounded-lg bg-zinc-900 border border-zinc-700 px-3 py-2 text-sm
                   text-zinc-200 placeholder-zinc-500 focus:border-emerald-500 focus:outline-none"
        required
      />
      {error && <p className="text-red-400 text-xs">{error}</p>}
      <button
        type="submit"
        disabled={loading}
        className="w-full rounded-lg bg-emerald-600 hover:bg-emerald-500 px-4 py-2 text-sm
                   font-medium text-white transition-colors disabled:opacity-50"
      >
        {loading ? 'Creating account...' : 'Sign Up'}
      </button>
    </form>
  );
}
