import { useState } from 'react';
import { BlockchainRPCClient, ProblemType } from '../lib/blockchain-rpc-client';
import { verifyReveal } from '../lib/privacy-crypto';

const rpcClient = new BlockchainRPCClient();

export default function RevealProblemForm({ onClose, onSuccess }: { onClose: () => void; onSuccess: () => void }) {
  const [problemId, setProblemId] = useState('');
  const [salt, setSalt] = useState('');
  const [problemJson, setProblemJson] = useState('');
  const [revealing, setRevealing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleReveal = async () => {
    setError(null);
    setRevealing(true);

    try {
      // Parse problem JSON
      const problem: ProblemType = JSON.parse(problemJson);

      // Get problem info to verify commitment
      const problemInfo = await rpcClient.getProblem(problemId);
      if (!problemInfo) {
        throw new Error('Problem not found');
      }

      if (!problemInfo.is_private) {
        throw new Error('This is not a private problem');
      }

      if (problemInfo.is_revealed) {
        throw new Error('Problem already revealed');
      }

      // Verify reveal locally before submitting
      // Note: We can't verify against commitment here without additional RPC method
      // In production, add getProble mCommitment RPC method

      // Submit reveal
      await rpcClient.revealProblem({
        problem_id: problemId,
        problem,
        salt,
      });

      alert('Problem revealed successfully!');
      onSuccess();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reveal problem');
    } finally {
      setRevealing(false);
    }
  };

  // Example problem for convenience
  const exampleProblem = JSON.stringify(
    {
      SubsetSum: {
        numbers: [10, 20, 30, 40, 50],
        target: 60,
      },
    },
    null,
    2
  );

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50">
      <div className="bg-gray-800 rounded-lg max-w-2xl w-full max-h-[90vh] overflow-y-auto p-6">
        <div className="flex justify-between items-center mb-6">
          <h2 className="text-2xl font-bold">Reveal Private Problem</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white">✕</button>
        </div>

        {error && (
          <div className="bg-red-500/20 border border-red-500 text-red-400 px-4 py-3 rounded mb-4">
            {error}
          </div>
        )}

        <div className="space-y-4">
          <div className="bg-blue-500/10 border border-blue-500/30 p-3 rounded text-sm">
            <strong>Reveal Process:</strong> Enter your problem ID, the original problem definition,
            and the salt you received when submitting. The blockchain will verify the reveal matches
            the commitment before accepting it.
          </div>

          <div>
            <label className="block text-sm font-medium mb-2">Problem ID</label>
            <input
              type="text"
              value={problemId}
              onChange={(e) => setProblemId(e.target.value)}
              placeholder="0x..."
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 font-mono text-sm"
            />
          </div>

          <div>
            <label className="block text-sm font-medium mb-2">Salt (from submission)</label>
            <input
              type="text"
              value={salt}
              onChange={(e) => setSalt(e.target.value)}
              placeholder="0x..."
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 font-mono text-sm"
            />
            <p className="text-xs text-gray-400 mt-1">
              The salt you received when submitting the private bounty
            </p>
          </div>

          <div>
            <label className="block text-sm font-medium mb-2">Problem Definition (JSON)</label>
            <textarea
              value={problemJson}
              onChange={(e) => setProblemJson(e.target.value)}
              placeholder={exampleProblem}
              rows={12}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 font-mono text-xs"
            />
            <p className="text-xs text-gray-400 mt-1">
              JSON format: {`{ "SubsetSum": { "numbers": [...], "target": X } }`}
            </p>
          </div>

          <div className="flex gap-3 pt-4">
            <button
              onClick={handleReveal}
              disabled={revealing || !problemId || !salt || !problemJson}
              className="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white font-bold py-3 px-4 rounded transition-colors"
            >
              {revealing ? 'Revealing...' : 'Reveal Problem'}
            </button>
            <button
              onClick={onClose}
              disabled={revealing}
              className="px-6 py-3 bg-gray-700 hover:bg-gray-600 rounded transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
