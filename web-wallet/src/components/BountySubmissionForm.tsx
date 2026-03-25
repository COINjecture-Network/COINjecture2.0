import { useState, useRef } from 'react';
import { Loader2, Copy } from 'lucide-react';
import { BlockchainRPCClient, ProblemType } from '../lib/blockchain-rpc-client';
import { generatePrivacyCredentials, getProblemParamsForRPC } from '../lib/privacy-crypto';
import { useToast } from './Toast';

const rpcClient = new BlockchainRPCClient();

// ── Validation ────────────────────────────────────────────────────────────────

function validatePositiveInt(val: string, label: string, min = 1): string | null {
  const n = parseInt(val, 10);
  if (isNaN(n) || n < min) return `${label} must be at least ${min}`;
  return null;
}

function validatePositiveFloat(val: string, label: string, min = 0): string | null {
  const n = parseFloat(val);
  if (isNaN(n) || n < min) return `${label} must be ≥ ${min}`;
  return null;
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function BountySubmissionForm({
  onClose,
  onSuccess,
}: {
  onClose: () => void;
  onSuccess: () => void;
}) {
  const { showToast } = useToast();

  const [problemType, setProblemType] = useState<'SubsetSum' | 'SAT' | 'TSP'>('SubsetSum');
  const [isPrivate, setIsPrivate] = useState(false);
  const [bounty, setBounty] = useState('1000');
  const [minWorkScore, setMinWorkScore] = useState('10.0');
  const [expirationDays, setExpirationDays] = useState('7');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedSalt, setSavedSalt] = useState<string | null>(null);
  const [saltCopied, setSaltCopied] = useState(false);
  const saltRef = useRef<HTMLElement>(null);

  // SubsetSum fields
  const [numbers, setNumbers] = useState('10, 20, 30, 40, 50');
  const [target, setTarget] = useState('60');

  // SAT fields
  const [variables, setVariables] = useState('3');
  const [clauses, setClauses] = useState('1, -2, 3\n-1, 2, 3');

  // TSP fields
  const [cities, setCities] = useState('4');
  const [distances, setDistances] = useState(
    '0, 10, 15, 20\n10, 0, 35, 25\n15, 35, 0, 30\n20, 25, 30, 0',
  );

  // ── Problem parsing ──────────────────────────────────────────────────────────

  const parseProblem = (): ProblemType | null => {
    try {
      switch (problemType) {
        case 'SubsetSum': {
          const nums = numbers.split(',').map(n => parseInt(n.trim(), 10));
          if (nums.some(isNaN)) { setError('Numbers must be comma-separated integers'); return null; }
          const t = parseInt(target, 10);
          if (isNaN(t)) { setError('Target must be an integer'); return null; }
          return { SubsetSum: { numbers: nums, target: t } };
        }
        case 'SAT': {
          const varCount = parseInt(variables, 10);
          if (isNaN(varCount) || varCount < 1) { setError('Variables must be a positive integer'); return null; }
          const parsedClauses = clauses.split('\n').map(line => ({
            literals: line.split(',').map(l => parseInt(l.trim(), 10)),
          }));
          if (parsedClauses.some(c => c.literals.some(isNaN))) {
            setError('Clauses contain invalid literals'); return null;
          }
          return { SAT: { variables: varCount, clauses: parsedClauses } };
        }
        case 'TSP': {
          const cityCount = parseInt(cities, 10);
          if (isNaN(cityCount) || cityCount < 2) { setError('Cities must be at least 2'); return null; }
          const rows = distances.split('\n');
          const distMatrix = rows.map(row => row.split(',').map(d => parseInt(d.trim(), 10)));
          if (distMatrix.some(r => r.some(isNaN))) {
            setError('Distance matrix contains invalid values'); return null;
          }
          return { TSP: { cities: cityCount, distances: distMatrix } };
        }
        default:
          return null;
      }
    } catch (err) {
      setError(`Failed to parse problem: ${err}`);
      return null;
    }
  };

  // ── Validation ───────────────────────────────────────────────────────────────

  const validate = (): boolean => {
    const bountyErr = validatePositiveInt(bounty, 'Bounty');
    if (bountyErr) { setError(bountyErr); return false; }

    const scoreErr = validatePositiveFloat(minWorkScore, 'Min work score');
    if (scoreErr) { setError(scoreErr); return false; }

    const daysErr = validatePositiveInt(expirationDays, 'Expiration days');
    if (daysErr) { setError(daysErr); return false; }

    return true;
  };

  // ── Submit ───────────────────────────────────────────────────────────────────

  const handleSubmit = async () => {
    setError(null);
    if (!validate()) return;

    const problem = parseProblem();
    if (!problem) return;

    setSubmitting(true);
    setSavedSalt(null);

    try {
      const bountyAmount = parseInt(bounty, 10);
      const workScore = parseFloat(minWorkScore);
      const days = parseInt(expirationDays, 10);

      if (isPrivate) {
        const credentials = await generatePrivacyCredentials(problem);
        const params = getProblemParamsForRPC(problem);

        const problemId = await rpcClient.submitPrivateProblem({
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
        });

        // Show the salt in the UI — do NOT use alert()
        setSavedSalt(credentials.salt);
        showToast(
          'success',
          'Private bounty submitted',
          `Problem ID: ${problemId} — Save the salt shown below!`,
        );
      } else {
        const problemId = await rpcClient.submitPublicProblem(problem, bountyAmount, workScore, days);
        showToast('success', 'Public bounty submitted', `Problem ID: ${problemId}`);
        onSuccess();
        onClose();
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to submit bounty';
      setError(msg);
      showToast('error', 'Submission failed', msg);
    } finally {
      setSubmitting(false);
    }
  };

  const copySalt = async () => {
    if (!savedSalt) return;
    try {
      await navigator.clipboard.writeText(savedSalt);
      setSaltCopied(true);
      setTimeout(() => setSaltCopied(false), 2000);
    } catch {
      showToast('error', 'Copy failed', 'Please copy the salt manually');
    }
  };

  // ── Render ───────────────────────────────────────────────────────────────────

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="bounty-modal-title"
      className="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50"
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.6)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 16,
        zIndex: 50,
      }}
    >
      <div
        className="bg-gray-800 rounded-lg max-w-2xl w-full max-h-[90vh] overflow-y-auto p-6"
        style={{
          background: '#1a202c',
          borderRadius: 12,
          maxWidth: 640,
          width: '100%',
          maxHeight: '90vh',
          overflowY: 'auto',
          padding: 24,
          color: 'white',
        }}
      >
        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
          <h2 id="bounty-modal-title" style={{ fontSize: 22, fontWeight: 700 }}>Submit Bounty</h2>
          <button
            onClick={onClose}
            aria-label="Close dialog"
            style={{ background: 'transparent', color: '#a0aec0', fontSize: 22, padding: 4 }}
          >
            ×
          </button>
        </div>

        {/* Error message */}
        {error && (
          <div
            role="alert"
            style={{
              background: 'rgba(245,101,101,0.15)',
              border: '1px solid #f56565',
              color: '#fc8181',
              padding: '12px 16px',
              borderRadius: 6,
              marginBottom: 16,
              fontSize: 14,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <span>{error}</span>
            <button
              onClick={() => setError(null)}
              aria-label="Dismiss error"
              style={{ background: 'transparent', color: '#fc8181', padding: 0, fontSize: 18 }}
            >
              ×
            </button>
          </div>
        )}

        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

          {/* Privacy toggle */}
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              background: 'rgba(255,255,255,0.05)',
              padding: 16,
              borderRadius: 8,
            }}
          >
            <div>
              <label
                htmlFor="privacy-toggle"
                style={{ fontWeight: 600, cursor: 'pointer' }}
              >
                Private Submission
              </label>
              <p style={{ fontSize: 12, color: '#a0aec0', marginTop: 2 }}>
                Hide problem instance until reveal
              </p>
            </div>
            <button
              id="privacy-toggle"
              role="switch"
              aria-checked={isPrivate}
              onClick={() => setIsPrivate(!isPrivate)}
              style={{
                position: 'relative',
                display: 'inline-flex',
                height: 24,
                width: 44,
                alignItems: 'center',
                borderRadius: 12,
                background: isPrivate ? '#3182ce' : '#4a5568',
                border: 'none',
                cursor: 'pointer',
                flexShrink: 0,
                padding: 0,
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  display: 'inline-block',
                  height: 16,
                  width: 16,
                  borderRadius: '50%',
                  background: 'white',
                  transform: isPrivate ? 'translateX(24px)' : 'translateX(4px)',
                  transition: 'transform 0.2s',
                }}
              />
              <span className="sr-only" style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0,0,0,0)' }}>
                {isPrivate ? 'Private mode on' : 'Private mode off'}
              </span>
            </button>
          </div>

          {isPrivate && (
            <div
              role="note"
              style={{
                background: 'rgba(99,179,237,0.1)',
                border: '1px solid rgba(99,179,237,0.3)',
                padding: 12,
                borderRadius: 6,
                fontSize: 13,
              }}
            >
              <strong>Privacy Mode:</strong> Your problem will be hidden until you reveal it.
              You will receive a salt value — save it securely to reveal the problem later!
            </div>
          )}

          {/* Problem type */}
          <div>
            <label htmlFor="problem-type-select" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
              Problem Type <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
            </label>
            <select
              id="problem-type-select"
              value={problemType}
              onChange={(e) => setProblemType(e.target.value as typeof problemType)}
              aria-required="true"
              style={{
                width: '100%',
                background: '#2d3748',
                border: '1px solid #4a5568',
                borderRadius: 6,
                padding: '8px 12px',
                color: 'white',
                fontSize: 14,
              }}
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
                <label htmlFor="subset-numbers" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Numbers (comma-separated) <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="subset-numbers"
                  type="text"
                  value={numbers}
                  onChange={(e) => setNumbers(e.target.value)}
                  placeholder="10, 20, 30, 40, 50"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
              <div>
                <label htmlFor="subset-target" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Target Sum <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="subset-target"
                  type="number"
                  value={target}
                  onChange={(e) => setTarget(e.target.value)}
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
            </>
          )}

          {problemType === 'SAT' && (
            <>
              <div>
                <label htmlFor="sat-vars" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Number of Variables <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="sat-vars"
                  type="number"
                  value={variables}
                  onChange={(e) => setVariables(e.target.value)}
                  min="1"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
              <div>
                <label htmlFor="sat-clauses" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Clauses (one per line, comma-separated literals) <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <textarea
                  id="sat-clauses"
                  value={clauses}
                  onChange={(e) => setClauses(e.target.value)}
                  placeholder={'1, -2, 3\n-1, 2, 3'}
                  rows={4}
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontFamily: 'monospace', fontSize: 12, width: '100%' }}
                />
                <p style={{ fontSize: 11, color: '#a0aec0', marginTop: 4 }}>
                  Positive literal = variable, negative = negated variable.
                </p>
              </div>
            </>
          )}

          {problemType === 'TSP' && (
            <>
              <div>
                <label htmlFor="tsp-cities" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Number of Cities <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="tsp-cities"
                  type="number"
                  value={cities}
                  onChange={(e) => setCities(e.target.value)}
                  min="2"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
              <div>
                <label htmlFor="tsp-distances" style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
                  Distance Matrix (one row per line) <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <textarea
                  id="tsp-distances"
                  value={distances}
                  onChange={(e) => setDistances(e.target.value)}
                  placeholder={'0, 10, 15, 20\n10, 0, 35, 25'}
                  rows={4}
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontFamily: 'monospace', fontSize: 12, width: '100%' }}
                />
              </div>
            </>
          )}

          {/* Bounty parameters */}
          <fieldset style={{ border: 'none', padding: 0, margin: 0 }}>
            <legend style={{ fontSize: 13, fontWeight: 600, marginBottom: 8, color: '#e2e8f0' }}>Bounty Parameters</legend>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
              <div>
                <label htmlFor="bounty-amount" style={{ display: 'block', fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
                  Bounty <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="bounty-amount"
                  type="number"
                  value={bounty}
                  onChange={(e) => setBounty(e.target.value)}
                  min="1"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
              <div>
                <label htmlFor="min-work-score" style={{ display: 'block', fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
                  Min Work Score <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="min-work-score"
                  type="number"
                  step="0.1"
                  value={minWorkScore}
                  onChange={(e) => setMinWorkScore(e.target.value)}
                  min="0"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
              <div>
                <label htmlFor="expiry-days" style={{ display: 'block', fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
                  Expires (days) <span aria-hidden="true" style={{ color: '#f56565' }}>*</span>
                </label>
                <input
                  id="expiry-days"
                  type="number"
                  value={expirationDays}
                  onChange={(e) => setExpirationDays(e.target.value)}
                  min="1"
                  max="365"
                  aria-required="true"
                  style={{ background: '#2d3748', border: '1px solid #4a5568', borderRadius: 6, padding: '8px 12px', color: 'white', fontSize: 14, width: '100%' }}
                />
              </div>
            </div>
          </fieldset>

          {/* Submit / Cancel */}
          <div style={{ display: 'flex', gap: 12, paddingTop: 8 }}>
            <button
              onClick={handleSubmit}
              disabled={submitting}
              aria-busy={submitting}
              style={{
                flex: 1,
                background: submitting ? '#4a5568' : '#3182ce',
                color: 'white',
                fontWeight: 700,
                padding: '12px 16px',
                borderRadius: 6,
                fontSize: 14,
                cursor: submitting ? 'not-allowed' : 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 8,
              }}
            >
              {submitting ? (
                <><Loader2 size={16} aria-hidden="true" style={{ animation: 'spin 0.7s linear infinite' }} /> Submitting…</>
              ) : isPrivate ? (
                'Submit Private Bounty'
              ) : (
                'Submit Public Bounty'
              )}
            </button>
            <button
              onClick={onClose}
              disabled={submitting}
              style={{
                padding: '12px 24px',
                background: '#4a5568',
                color: 'white',
                borderRadius: 6,
                cursor: submitting ? 'not-allowed' : 'pointer',
              }}
            >
              Cancel
            </button>
          </div>

          {/* Salt display — shown after successful private submission */}
          {savedSalt && (
            <div
              role="alert"
              aria-live="assertive"
              style={{
                background: 'rgba(236,201,75,0.15)',
                border: '1px solid #d69e2e',
                padding: 16,
                borderRadius: 8,
                marginTop: 8,
              }}
            >
              <p style={{ fontWeight: 700, color: '#d69e2e', marginBottom: 8, fontSize: 14 }}>
                ⚠ Save this salt — required to reveal your problem later!
              </p>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <code
                  ref={saltRef}
                  style={{
                    flex: 1,
                    display: 'block',
                    background: 'rgba(0,0,0,0.3)',
                    padding: '8px 12px',
                    borderRadius: 4,
                    fontSize: 11,
                    fontFamily: 'monospace',
                    wordBreak: 'break-all',
                    color: '#f6e05e',
                  }}
                  aria-label="Salt value"
                >
                  {savedSalt}
                </code>
                <button
                  onClick={copySalt}
                  aria-label="Copy salt to clipboard"
                  style={{ background: '#d69e2e', color: 'black', padding: '8px 12px', borderRadius: 4, fontSize: 12, flexShrink: 0 }}
                >
                  {saltCopied ? '✓ Copied' : <><Copy size={14} aria-hidden="true" /> Copy</>}
                </button>
              </div>
              <p style={{ fontSize: 12, color: '#b7791f', marginTop: 8 }}>
                Store this salt in a safe place. It cannot be recovered if lost.
              </p>
              <button
                onClick={() => { onSuccess(); onClose(); }}
                style={{ marginTop: 12, width: '100%', background: '#276749', color: 'white', padding: '10px 16px', borderRadius: 6, fontWeight: 600 }}
              >
                I have saved the salt — Close
              </button>
            </div>
          )}

        </div>
      </div>
    </div>
  );
}
