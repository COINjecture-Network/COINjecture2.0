import { useState, useEffect, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Wallet, Copy, Upload, Send, Plus, Trash2, Eye, EyeOff, Loader2 } from 'lucide-react'
import { RpcClient } from '../lib/rpc-client'
import { generateKeyPair, importKeyPair, KeyStore, KeyPair } from '../lib/crypto'
import { useToast } from './Toast'
import TransactionModal from './TransactionModal'

const rpcClient = new RpcClient()

// ── Validation helpers ────────────────────────────────────────────────────────

/** Account name: 1–40 printable characters, no HTML special chars. */
function validateAccountName(name: string): string | null {
  const trimmed = name.trim()
  if (!trimmed) return 'Account name is required'
  if (trimmed.length > 40) return 'Account name must be 40 characters or fewer'
  if (/[<>"'`]/.test(trimmed)) return 'Account name contains invalid characters'
  return null
}

/** Private key: exactly 64 hex characters. */
function validatePrivateKeyHex(hex: string): string | null {
  const trimmed = hex.trim()
  if (!trimmed) return 'Private key is required'
  if (!/^[0-9a-fA-F]{64}$/.test(trimmed)) return 'Private key must be exactly 64 hex characters'
  return null
}

// ── Confirm dialog (inline, no browser confirm()) ────────────────────────────

interface ConfirmDialogProps {
  message: string
  detail?: string
  onConfirm: () => void
  onCancel: () => void
}

function ConfirmDialog({ message, detail, onConfirm, onCancel }: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    cancelRef.current?.focus()
  }, [])

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 2000,
      }}
    >
      <div
        style={{
          background: 'white',
          borderRadius: 12,
          padding: 24,
          maxWidth: 400,
          width: '90%',
          boxShadow: '0 20px 40px rgba(0,0,0,0.2)',
        }}
      >
        <h3 id="confirm-title" style={{ marginBottom: 8, fontSize: 18, fontWeight: 600, color: '#c53030' }}>
          Confirm Action
        </h3>
        <p style={{ marginBottom: detail ? 8 : 20, fontSize: 15, color: '#2d3748' }}>{message}</p>
        {detail && <p style={{ marginBottom: 20, fontSize: 13, color: '#718096' }}>{detail}</p>}
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            ref={cancelRef}
            onClick={onCancel}
            style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }}
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            style={{ flex: 1, background: '#c53030' }}
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function WalletView() {
  const [selectedAccount, setSelectedAccount] = useState<string | null>(null)
  const [accounts, setAccounts] = useState<Record<string, KeyPair>>({})
  const [showNewAccount, setShowNewAccount] = useState(false)
  const [showImport, setShowImport] = useState(false)
  const [showSend, setShowSend] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null)
  const { showToast } = useToast()

  useEffect(() => {
    setAccounts(KeyStore.list())
  }, [])

  const handleDeleteAccount = (name: string) => {
    KeyStore.delete(name)
    setAccounts(KeyStore.list())
    if (selectedAccount === name) setSelectedAccount(null)
    showToast('success', `Account "${name}" deleted`)
    setConfirmDelete(null)
  }

  return (
    <>
      <div
        className="wallet-grid"
        style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20 }}
      >
        {/* Left Column: Account list */}
        <section aria-labelledby="accounts-heading">
          <div className="card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h2 id="accounts-heading" className="card-header" style={{ marginBottom: 0 }}>
                My Accounts
              </h2>
              <button
                onClick={() => setShowNewAccount(true)}
                aria-label="Create new account"
                style={{ display: 'flex', alignItems: 'center', gap: 6 }}
              >
                <Plus size={16} aria-hidden="true" /> New
              </button>
            </div>

            {Object.keys(accounts).length === 0 ? (
              <div style={{ textAlign: 'center', padding: 40, color: '#718096' }} role="status">
                <Wallet size={48} style={{ margin: '0 auto 16px', opacity: 0.5 }} aria-hidden="true" />
                <p>No accounts yet</p>
                <p style={{ fontSize: 14, marginTop: 8 }}>Create or import an account to get started</p>
              </div>
            ) : (
              <ul
                role="list"
                aria-label="Accounts"
                style={{ display: 'flex', flexDirection: 'column', gap: 12, listStyle: 'none', padding: 0, margin: 0 }}
              >
                {Object.entries(accounts).map(([name, keyPair]) => (
                  <li key={name}>
                    <AccountCard
                      name={name}
                      keyPair={keyPair}
                      selected={selectedAccount === name}
                      onSelect={() => setSelectedAccount(name)}
                      onDelete={() => setConfirmDelete(name)}
                    />
                  </li>
                ))}
              </ul>
            )}

            <div style={{ marginTop: 16, display: 'flex', gap: 8 }}>
              <button
                onClick={() => setShowImport(true)}
                aria-label="Import account from private key"
                style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
              >
                <Upload size={16} aria-hidden="true" /> Import
              </button>
            </div>
          </div>
        </section>

        {/* Right Column: Account details & actions */}
        <section aria-labelledby="details-heading">
          {selectedAccount && accounts[selectedAccount] ? (
            <>
              <AccountDetails
                account={selectedAccount}
                keyPair={accounts[selectedAccount]}
              />
              <div style={{ marginTop: 20, display: 'flex', gap: 12 }}>
                <button
                  onClick={() => setShowSend(true)}
                  aria-label="Open send transaction dialog"
                  style={{
                    flex: 1,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    gap: 8,
                    padding: 16,
                    fontSize: 16,
                  }}
                >
                  <Send size={18} aria-hidden="true" /> Send Transaction
                </button>
              </div>
            </>
          ) : (
            <div
              className="card"
              style={{ textAlign: 'center', padding: 60, color: '#718096' }}
              role="status"
            >
              <h2 id="details-heading" className="card-header" style={{ color: '#a0aec0' }}>Account Details</h2>
              <p>Select an account to view details</p>
            </div>
          )}
        </section>
      </div>

      {/* Delete confirmation dialog */}
      {confirmDelete && (
        <ConfirmDialog
          message={`Delete account "${confirmDelete}"?`}
          detail="This will remove the account from this browser. Make sure you have backed up the private key."
          onConfirm={() => handleDeleteAccount(confirmDelete)}
          onCancel={() => setConfirmDelete(null)}
        />
      )}

      {/* Modals */}
      {showNewAccount && (
        <NewAccountModal
          onClose={() => setShowNewAccount(false)}
          onCreated={(name) => {
            setAccounts(KeyStore.list())
            setSelectedAccount(name)
            setShowNewAccount(false)
            showToast('success', `Account "${name}" created`, 'Your new account is ready to use')
          }}
        />
      )}

      {showImport && (
        <ImportAccountModal
          onClose={() => setShowImport(false)}
          onImported={(name) => {
            setAccounts(KeyStore.list())
            setSelectedAccount(name)
            setShowImport(false)
            showToast('success', `Account "${name}" imported`)
          }}
        />
      )}

      {showSend && selectedAccount && accounts[selectedAccount] && (
        <TransactionModal
          account={selectedAccount}
          keyPair={accounts[selectedAccount]}
          onClose={() => setShowSend(false)}
        />
      )}
    </>
  )
}

// ── Account card ──────────────────────────────────────────────────────────────

interface AccountCardProps {
  name: string
  keyPair: KeyPair
  selected: boolean
  onSelect: () => void
  onDelete: () => void
}

function AccountCard({ name, keyPair, selected, onSelect, onDelete }: AccountCardProps) {
  const { data: balance, isLoading } = useQuery({
    queryKey: ['balance', keyPair.address],
    queryFn: () => rpcClient.getAccountBalance(keyPair.address),
    refetchInterval: 10000,
  })

  return (
    <div
      onClick={onSelect}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect() } }}
      role="button"
      tabIndex={0}
      aria-pressed={selected}
      aria-label={`Select account ${name}`}
      style={{
        padding: 16,
        border: selected ? '2px solid #667eea' : '1px solid #e2e8f0',
        borderRadius: 8,
        cursor: 'pointer',
        background: selected ? '#f7fafc' : 'white',
        transition: 'all 0.2s',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>{name}</div>
          <div
            style={{ fontSize: 12, color: '#718096', fontFamily: 'monospace', wordBreak: 'break-all' }}
            aria-label={`Address: ${keyPair.address}`}
          >
            {keyPair.address.slice(0, 16)}…{keyPair.address.slice(-8)}
          </div>
          <div
            style={{ marginTop: 8, fontSize: 18, fontWeight: 700, color: '#667eea' }}
            aria-label={balance !== undefined ? `Balance: ${balance.toLocaleString()} tokens` : 'Balance loading'}
          >
            {isLoading ? (
              <span className="skeleton" style={{ display: 'inline-block', width: 80, height: 20 }} aria-hidden="true" />
            ) : balance !== undefined ? (
              `${balance.toLocaleString()} tokens`
            ) : (
              '—'
            )}
          </div>
        </div>
        <button
          onClick={(e) => { e.stopPropagation(); onDelete() }}
          aria-label={`Delete account ${name}`}
          style={{ background: '#fed7d7', color: '#c53030', padding: 6 }}
        >
          <Trash2 size={16} aria-hidden="true" />
        </button>
      </div>
    </div>
  )
}

// ── Account details ───────────────────────────────────────────────────────────

interface AccountDetailsProps {
  account: string
  keyPair: KeyPair
}

function AccountDetails({ account, keyPair }: AccountDetailsProps) {
  const [showPrivateKey, setShowPrivateKey] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const { showToast } = useToast()

  const { data: accountInfo, isLoading: infoLoading } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
    refetchInterval: 10000,
  })

  const faucetMutation = useMutation({
    mutationFn: () => rpcClient.faucetRequest(keyPair.address),
    onSuccess: (response) => {
      queryClient.invalidateQueries({ queryKey: ['accountInfo', keyPair.address] })
      queryClient.invalidateQueries({ queryKey: ['balance', keyPair.address] })
      if (response.success) {
        showToast(
          'success',
          'Faucet request successful',
          `Received ${response.amount?.toLocaleString() ?? ''} tokens. New balance: ${response.new_balance?.toLocaleString() ?? ''} tokens`,
        )
      } else {
        const cooldown = response.cooldown_remaining
          ? ` Try again in ${response.cooldown_remaining}s.`
          : ''
        showToast('warning', 'Faucet unavailable', (response.message ?? 'Request denied') + cooldown)
      }
    },
    onError: (error: Error) => {
      showToast('error', 'Faucet request failed', error.message ?? 'Network error')
    },
  })

  const copyToClipboard = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(label)
      setTimeout(() => setCopied(null), 2000)
    } catch {
      showToast('error', 'Copy failed', 'Could not access clipboard')
    }
  }

  return (
    <section className="card" aria-labelledby="details-heading">
      <h3 id="details-heading" className="card-header">
        Account Details: {account}
      </h3>

      {/* Balance */}
      <dl>
        <div style={{ marginBottom: 16 }}>
          <dt style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Balance</dt>
          <dd
            style={{ fontSize: 24, fontWeight: 700, color: '#667eea', margin: 0 }}
            aria-live="polite"
            aria-busy={infoLoading}
          >
            {infoLoading ? (
              <span className="skeleton" style={{ display: 'inline-block', width: 160, height: 28 }} aria-hidden="true" />
            ) : accountInfo ? (
              `${accountInfo.balance.toLocaleString()} tokens`
            ) : (
              '—'
            )}
          </dd>
        </div>

        {/* Nonce */}
        <div style={{ marginBottom: 16 }}>
          <dt style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Nonce</dt>
          <dd style={{ fontFamily: 'monospace', margin: 0 }}>{accountInfo?.nonce ?? '—'}</dd>
        </div>

        {/* Address */}
        <div style={{ marginBottom: 16 }}>
          <dt style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Address</dt>
          <dd style={{ margin: 0 }}>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <code
                style={{ flex: 1, fontSize: 11, wordBreak: 'break-all' }}
                aria-label={`Address: ${keyPair.address}`}
              >
                {keyPair.address}
              </code>
              <button
                onClick={() => copyToClipboard(keyPair.address, 'address')}
                aria-label="Copy address to clipboard"
                style={{ padding: 6 }}
              >
                <Copy size={14} aria-hidden="true" />
              </button>
            </div>
            {copied === 'address' && (
              <div role="status" style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>
            )}
          </dd>
        </div>

        {/* Public Key */}
        <div style={{ marginBottom: 16 }}>
          <dt style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Public Key</dt>
          <dd style={{ margin: 0 }}>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <code style={{ flex: 1, fontSize: 11, wordBreak: 'break-all' }}>{keyPair.publicKey}</code>
              <button
                onClick={() => copyToClipboard(keyPair.publicKey, 'public')}
                aria-label="Copy public key to clipboard"
                style={{ padding: 6 }}
              >
                <Copy size={14} aria-hidden="true" />
              </button>
            </div>
            {copied === 'public' && (
              <div role="status" style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>
            )}
          </dd>
        </div>

        {/* Private Key */}
        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
            <dt style={{ fontSize: 12, color: '#718096' }}>Private Key</dt>
            <button
              onClick={() => setShowPrivateKey(!showPrivateKey)}
              aria-label={showPrivateKey ? 'Hide private key' : 'Reveal private key'}
              aria-pressed={showPrivateKey}
              style={{ padding: 4, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              {showPrivateKey
                ? <><EyeOff size={12} aria-hidden="true" /> Hide</>
                : <><Eye size={12} aria-hidden="true" /> Show</>}
            </button>
          </div>
          <dd style={{ margin: 0 }}>
            {showPrivateKey ? (
              <>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <code
                    style={{ flex: 1, fontSize: 11, wordBreak: 'break-all', background: '#fed7d7', padding: '4px 8px', borderRadius: 4 }}
                    aria-label="Private key (sensitive)"
                  >
                    {keyPair.privateKey}
                  </code>
                  <button
                    onClick={() => copyToClipboard(keyPair.privateKey, 'private')}
                    aria-label="Copy private key to clipboard"
                    style={{ padding: 6 }}
                  >
                    <Copy size={14} aria-hidden="true" />
                  </button>
                </div>
                {copied === 'private' && (
                  <div role="status" style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>
                )}
                <p role="alert" style={{ fontSize: 11, color: '#c53030', marginTop: 4 }}>
                  ⚠ Never share your private key. Anyone with it controls your funds.
                </p>
              </>
            ) : (
              <code style={{ fontSize: 11 }} aria-label="Private key hidden">••••••••••••••••••••••••••••••••</code>
            )}
          </dd>
        </div>
      </dl>

      <button
        onClick={() => faucetMutation.mutate()}
        disabled={faucetMutation.isPending}
        aria-busy={faucetMutation.isPending}
        style={{
          width: '100%',
          marginTop: 20,
          padding: 12,
          background: faucetMutation.isPending ? '#cbd5e0' : '#48bb78',
          color: 'white',
          fontSize: 14,
          fontWeight: 600,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 8,
          cursor: faucetMutation.isPending ? 'not-allowed' : 'pointer',
        }}
      >
        {faucetMutation.isPending ? (
          <><Loader2 size={16} aria-hidden="true" style={{ animation: 'spin 0.7s linear infinite' }} /> Requesting…</>
        ) : (
          '💧 Request Testnet Tokens'
        )}
      </button>
    </section>
  )
}

// ── New account modal ─────────────────────────────────────────────────────────

interface ModalProps { onClose: () => void }

function NewAccountModal({ onClose, onCreated }: ModalProps & { onCreated: (name: string) => void }) {
  const [name, setName] = useState('')
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => { inputRef.current?.focus() }, [])

  const handleCreate = () => {
    const validationError = validateAccountName(name)
    if (validationError) { setError(validationError); return }
    const existing = KeyStore.list()
    if (existing[name.trim()]) { setError('An account with that name already exists'); return }
    const keyPair = generateKeyPair()
    KeyStore.save(name.trim(), keyPair)
    onCreated(name.trim())
  }

  return (
    <Modal onClose={onClose} title="Create New Account">
      {error && (
        <div role="alert" style={{ padding: 12, background: '#fed7d7', color: '#c53030', borderRadius: 6, marginBottom: 16, fontSize: 14 }}>
          {error}
        </div>
      )}
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="new-account-name" style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>
          Account Name <span aria-hidden="true" style={{ color: '#e53e3e' }}>*</span>
        </label>
        <input
          id="new-account-name"
          ref={inputRef}
          type="text"
          value={name}
          onChange={(e) => { setName(e.target.value); setError('') }}
          onKeyDown={(e) => { if (e.key === 'Enter') handleCreate() }}
          placeholder="e.g., My Testnet Account"
          maxLength={40}
          aria-required="true"
          aria-invalid={!!error}
          aria-describedby={error ? 'new-account-error' : undefined}
          autoComplete="off"
        />
        {error && <span id="new-account-error" style={{ display: 'none' }}>{error}</span>}
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <button onClick={onClose} style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }}>
          Cancel
        </button>
        <button onClick={handleCreate} style={{ flex: 1 }}>
          Create
        </button>
      </div>
    </Modal>
  )
}

// ── Import account modal ──────────────────────────────────────────────────────

function ImportAccountModal({ onClose, onImported }: ModalProps & { onImported: (name: string) => void }) {
  const [name, setName] = useState('')
  const [privateKey, setPrivateKey] = useState('')
  const [error, setError] = useState('')
  const nameRef = useRef<HTMLInputElement>(null)

  useEffect(() => { nameRef.current?.focus() }, [])

  const handleImport = () => {
    const nameError = validateAccountName(name)
    if (nameError) { setError(nameError); return }

    const keyError = validatePrivateKeyHex(privateKey)
    if (keyError) { setError(keyError); return }

    const existing = KeyStore.list()
    if (existing[name.trim()]) { setError('An account with that name already exists'); return }

    try {
      const keyPair = importKeyPair(privateKey.trim())
      KeyStore.save(name.trim(), keyPair)
      onImported(name.trim())
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Invalid private key')
    }
  }

  return (
    <Modal onClose={onClose} title="Import Account">
      {error && (
        <div role="alert" style={{ padding: 12, background: '#fed7d7', color: '#c53030', borderRadius: 6, marginBottom: 16, fontSize: 14 }}>
          {error}
        </div>
      )}
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="import-account-name" style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>
          Account Name <span aria-hidden="true" style={{ color: '#e53e3e' }}>*</span>
        </label>
        <input
          id="import-account-name"
          ref={nameRef}
          type="text"
          value={name}
          onChange={(e) => { setName(e.target.value); setError('') }}
          placeholder="e.g., Imported Account"
          maxLength={40}
          aria-required="true"
          autoComplete="off"
        />
      </div>
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="import-private-key" style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>
          Private Key (hex) <span aria-hidden="true" style={{ color: '#e53e3e' }}>*</span>
        </label>
        <textarea
          id="import-private-key"
          value={privateKey}
          onChange={(e) => { setPrivateKey(e.target.value); setError('') }}
          placeholder="Enter 64-character hex private key"
          rows={3}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
          aria-required="true"
          autoComplete="off"
          spellCheck={false}
        />
      </div>
      <p style={{ fontSize: 12, color: '#718096', marginBottom: 16 }}>
        ⚠ Only import private keys on a device you trust and control.
      </p>
      <div style={{ display: 'flex', gap: 8 }}>
        <button onClick={onClose} style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }}>
          Cancel
        </button>
        <button onClick={handleImport} style={{ flex: 1 }}>
          Import
        </button>
      </div>
    </Modal>
  )
}

// ── Base modal ────────────────────────────────────────────────────────────────

function Modal({ children, onClose, title }: { children: React.ReactNode; onClose: () => void; title: string }) {
  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="modal-content"
        style={{
          background: 'white',
          borderRadius: 12,
          padding: 24,
          maxWidth: 500,
          width: '90%',
          maxHeight: '90vh',
          overflow: 'auto',
        }}
      >
        <h3 id="modal-title" style={{ marginBottom: 16, fontSize: 20, fontWeight: 600 }}>{title}</h3>
        {children}
      </div>
    </div>
  )
}
