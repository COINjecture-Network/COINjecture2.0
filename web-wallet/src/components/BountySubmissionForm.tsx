import { useState } from 'react';
import { BlockchainRPCClient, ProblemType } from '../lib/blockchain-rpc-client';
import { generatePrivacyCredentials, getProblemParamsForRPC } from '../lib/privacy-crypto';

const rpcClient = new BlockchainRPCClient();

export default function BountySubmissionForm({ onClose, onSuccess }: { onClose: () => void; onSuccess: () => void }) {
  const [problemType, setProblemType] = useState<'SubsetSum' | 'SAT' | 'TSP'>('SubsetSum');
  const [isPrivate, setIsPrivate] = useState(false);
  const [bounty, setBounty] = useState('1000');
  const [minWorkScore, setMinWorkScore] = useState('10.0');
  const [expirationDays, setExpirationDays] = useState('7');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedSalt, setSavedSalt] = useState<string | null>(null);

  // SubsetSum fields
  const [numbers, setNumbers] = useState('10, 20, 30, 40, 50');
  const [target, setTarget] = useState('60');

  // SAT fields
  const [variables, setVariables] = useState('3');
  const [clauses, setClauses] = useState('1, -2, 3\n-1, 2, 3');

  // TSP fields
  const [cities, setCities] = useState('4');
  const [distances, setDistances] = useState('0, 10, 15, 20\n10, 0, 35, 25\n15, 35, 0, 30\n20, 25, 30, 0');

  const parseProblem = (): ProblemType | null => {
    try {
      switch (problemType) {
        case 'SubsetSum':
          return {
            SubsetSum: {
              numbers: numbers.split(',').map(n => parseInt(n.trim())),
              target: parseInt(target),
            },
          };
        case 'SAT':
          return {
            SAT: {
              variables: parseInt(variables),
              clauses: clauses.split('\n').map(line => ({
                literals: line.split(',').map(l => parseInt(l.trim())),
              })),
            },
          };
        case 'TSP':
          const rows = distances.split('\n');
          const distanceMatrix = rows.map(row =>
            row.split(',').map(d => parseInt(d.trim()))
          );
          return {
            TSP: {
              cities: parseInt(cities),
              distances: distanceMatrix,
            },
          };
        default:
          return null;
      }
    } catch (err) {
      setError(`Failed to parse problem: ${err}`);
      return null;
    }
  };

  const handleSubmit = async () => {
    setError(null);
    setSubmitting(true);
    setSavedSalt(null);

    try {
      const problem = parseProblem();
      if (!problem) {
        throw new Error('Invalid problem definition');
      }

      const bountyAmount = parseInt(bounty);
      const workScore = parseFloat(minWorkScore);
      const days = parseInt(expirationDays);

      if (isPrivate) {
        // Generate privacy credentials
        const credentials = await generatePrivacyCredentials(problem);
        const params = getProblemParamsForRPC(problem);

        const privateProblemParams = {
          commitment: credentials.commitment,
          proof_bytes: credentials.proof.proof_bytes,
          vk_hash: credentials.proof.vk_hash,
          public_inputs: credentials.proof.public_inputs,
          problem_type: params.problem_type,
          size: params.size,
          complexity_estimate: params.complexity_estimate,
          bounty: bountyAmount,
          min_work_score: workScore,
          expiration_days: days,
        };

        const problemId = await rpcClient.submitPrivateProblem(privateProblemParams);

        // CRITICAL: Save salt for reveal!
        setSavedSalt(credentials.salt);
        alert(`Private bounty submitted!\nProblem ID: ${problemId}\n\nIMPORTANT: Save this salt to reveal later:\n${credentials.salt}`);
      } else {
        // Submit public problem
        const problemId = await rpcClient.submitPublicProblem(problem, bountyAmount, workScore, days);
        alert(`Public bounty submitted successfully!\nProblem ID: ${problemId}`);
      }

      onSuccess();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to submit bounty');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50">
      <div className="bg-gray-800 rounded-lg max-w-2xl w-full max-h-[90vh] overflow-y-auto p-6">
        <div className="flex justify-between items-center mb-6">
          <h2 className="text-2xl font-bold">Submit Bounty</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white">✕</button>
        </div>

        {error && (
          <div className="bg-red-500/20 border border-red-500 text-red-400 px-4 py-3 rounded mb-4">
            {error}
          </div>
        )}

        <div className="space-y-4">
          {/* Privacy Toggle */}
          <div className="flex items-center justify-between bg-gray-750 p-4 rounded">
            <div>
              <label className="font-medium">Private Submission</label>
              <p className="text-sm text-gray-400">Hide problem instance until reveal</p>
            </div>
            <button
              onClick={() => setIsPrivate(!isPrivate)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                isPrivate ? 'bg-blue-600' : 'bg-gray-600'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  isPrivate ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          {isPrivate && (
            <div className="bg-blue-500/10 border border-blue-500/30 p-3 rounded text-sm">
              <strong>Privacy Mode:</strong> Your problem will be hidden until you reveal it.
              You will receive a salt value - SAVE IT SECURELY to reveal the problem later!
            </div>
          )}

          {/* Problem Type */}
          <div>
            <label className="block text-sm font-medium mb-2">Problem Type</label>
            <select
              value={problemType}
              onChange={(e) => setProblemType(e.target.value as any)}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
            >
              <option value="SubsetSum">Subset Sum</option>
              <option value="SAT">SAT (Boolean Satisfiability)</option>
              <option value="TSP">TSP (Traveling Salesman)</option>
            </select>
          </div>

          {/* Problem-specific fields */}
          {problemType === 'SubsetSum' && (
            <>
              <div>
                <label className="block text-sm font-medium mb-2">Numbers (comma-separated)</label>
                <input
                  type="text"
                  value={numbers}
                  onChange={(e) => setNumbers(e.target.value)}
                  placeholder="10, 20, 30, 40, 50"
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm font-medium mb-2">Target Sum</label>
                <input
                  type="number"
                  value={target}
                  onChange={(e) => setTarget(e.target.value)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
                />
              </div>
            </>
          )}

          {problemType === 'SAT' && (
            <>
              <div>
                <label className="block text-sm font-medium mb-2">Number of Variables</label>
                <input
                  type="number"
                  value={variables}
                  onChange={(e) => setVariables(e.target.value)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm font-medium mb-2">Clauses (one per line)</label>
                <textarea
                  value={clauses}
                  onChange={(e) => setClauses(e.target.value)}
                  placeholder="1, -2, 3&#10;-1, 2, 3"
                  rows={4}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 font-mono text-sm"
                />
                <p className="text-xs text-gray-400 mt-1">Positive = variable, negative = negated</p>
              </div>
            </>
          )}

          {problemType === 'TSP' && (
            <>
              <div>
                <label className="block text-sm font-medium mb-2">Number of Cities</label>
                <input
                  type="number"
                  value={cities}
                  onChange={(e) => setCities(e.target.value)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm font-medium mb-2">Distance Matrix (one row per line)</label>
                <textarea
                  value={distances}
                  onChange={(e) => setDistances(e.target.value)}
                  placeholder="0, 10, 15, 20&#10;10, 0, 35, 25&#10;15, 35, 0, 30&#10;20, 25, 30, 0"
                  rows={4}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 font-mono text-sm"
                />
              </div>
            </>
          )}

          {/* Bounty Parameters */}
          <div className="grid grid-cols-3 gap-4">
            <div>
              <label className="block text-sm font-medium mb-2">Bounty</label>
              <input
                type="number"
                value={bounty}
                onChange={(e) => setBounty(e.target.value)}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-2">Min Work Score</label>
              <input
                type="number"
                step="0.1"
                value={minWorkScore}
                onChange={(e) => setMinWorkScore(e.target.value)}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-2">Expires (days)</label>
              <input
                type="number"
                value={expirationDays}
                onChange={(e) => setExpirationDays(e.target.value)}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2"
              />
            </div>
          </div>

          {/* Submit Button */}
          <div className="flex gap-3 pt-4">
            <button
              onClick={handleSubmit}
              disabled={submitting}
              className="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white font-bold py-3 px-4 rounded transition-colors"
            >
              {submitting ? 'Submitting...' : isPrivate ? 'Submit Private Bounty' : 'Submit Public Bounty'}
            </button>
            <button
              onClick={onClose}
              disabled={submitting}
              className="px-6 py-3 bg-gray-700 hover:bg-gray-600 rounded transition-colors"
            >
              Cancel
            </button>
          </div>

          {savedSalt && (
            <div className="bg-yellow-500/20 border border-yellow-500 p-4 rounded mt-4">
              <p className="font-bold text-yellow-400 mb-2">SAVE THIS SALT!</p>
              <code className="block bg-black/30 p-2 rounded text-xs break-all">{savedSalt}</code>
              <p className="text-sm text-yellow-300 mt-2">
                You need this salt to reveal your problem later. Store it securely!
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
