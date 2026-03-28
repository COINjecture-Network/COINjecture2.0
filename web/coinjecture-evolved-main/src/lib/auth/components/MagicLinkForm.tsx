import { useState } from 'react';
import { useAuth } from '../useAuth';

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
    } catch (err: any) {
      setError(err.message || 'Failed to send magic link');
    } finally {
      setLoading(false);
    }
  };

  if (sent) {
    return (
      <div className="text-center py-4">
        <p className="text-emerald-400 font-medium">Check your email</p>
        <p className="text-zinc-400 text-sm mt-2">
          If an account exists for <strong>{email}</strong>, we sent a sign-in link.
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
      {error && <p className="text-red-400 text-xs">{error}</p>}
      <button
        type="submit"
        disabled={loading}
        className="w-full rounded-lg bg-zinc-700 hover:bg-zinc-600 px-4 py-2 text-sm
                   font-medium text-zinc-200 transition-colors disabled:opacity-50"
      >
        {loading ? 'Sending...' : 'Send Magic Link'}
      </button>
    </form>
  );
}
