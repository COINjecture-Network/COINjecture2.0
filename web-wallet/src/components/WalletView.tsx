import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Wallet, Copy, Upload, Send, Plus, Trash2, Eye, EyeOff } from 'lucide-react'
import { RpcClient } from '../lib/rpc-client'
import { generateKeyPair, importKeyPair, KeyStore, KeyPair } from '../lib/crypto'
import TransactionModal from './TransactionModal'

const rpcClient = new RpcClient()

export default function WalletView() {
  const [selectedAccount, setSelectedAccount] = useState<string | null>(null)
  const [accounts, setAccounts] = useState<Record<string, KeyPair>>({})
  const [showNewAccount, setShowNewAccount] = useState(false)
  const [showImport, setShowImport] = useState(false)
  const [showSend, setShowSend] = useState(false)

  useEffect(() => {
    setAccounts(KeyStore.list())
  }, [])

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20 }}>
      {/* Left Column: Account Management */}
      <div>
        <div className="card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
            <h2 className="card-header" style={{ marginBottom: 0 }}>My Accounts</h2>
            <button onClick={() => setShowNewAccount(true)} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <Plus size={16} /> New
            </button>
          </div>

          {Object.keys(accounts).length === 0 ? (
            <div style={{ textAlign: 'center', padding: 40, color: '#718096' }}>
              <Wallet size={48} style={{ margin: '0 auto 16px', opacity: 0.5 }} />
              <p>No accounts yet</p>
              <p style={{ fontSize: 14, marginTop: 8 }}>Create or import an account to get started</p>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {Object.entries(accounts).map(([name, keyPair]) => (
                <AccountCard
                  key={name}
                  name={name}
                  keyPair={keyPair}
                  selected={selectedAccount === name}
                  onSelect={() => setSelectedAccount(name)}
                  onDelete={() => {
                    KeyStore.delete(name)
                    setAccounts(KeyStore.list())
                    if (selectedAccount === name) setSelectedAccount(null)
                  }}
                />
              ))}
            </div>
          )}

          <div style={{ marginTop: 16, display: 'flex', gap: 8 }}>
            <button onClick={() => setShowImport(true)} style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}>
              <Upload size={16} /> Import
            </button>
          </div>
        </div>
      </div>

      {/* Right Column: Account Details & Actions */}
      <div>
        {selectedAccount && accounts[selectedAccount] ? (
          <>
            <AccountDetails account={selectedAccount} keyPair={accounts[selectedAccount]} />
            <div style={{ marginTop: 20, display: 'flex', gap: 12 }}>
              <button
                onClick={() => setShowSend(true)}
                style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, padding: 16, fontSize: 16 }}
              >
                <Send size={18} /> Send Transaction
              </button>
            </div>
          </>
        ) : (
          <div className="card" style={{ textAlign: 'center', padding: 60, color: '#718096' }}>
            <p>Select an account to view details</p>
          </div>
        )}
      </div>

      {/* Modals */}
      {showNewAccount && (
        <NewAccountModal
          onClose={() => setShowNewAccount(false)}
          onCreated={(name) => {
            setAccounts(KeyStore.list())
            setSelectedAccount(name)
            setShowNewAccount(false)
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
    </div>
  )
}

interface AccountCardProps {
  name: string
  keyPair: KeyPair
  selected: boolean
  onSelect: () => void
  onDelete: () => void
}

function AccountCard({ name, keyPair, selected, onSelect, onDelete }: AccountCardProps) {
  const { data: balance } = useQuery({
    queryKey: ['balance', keyPair.address],
    queryFn: () => rpcClient.getAccountBalance(keyPair.address),
    refetchInterval: 10000
  })

  return (
    <div
      onClick={onSelect}
      style={{
        padding: 16,
        border: selected ? '2px solid #667eea' : '1px solid #e2e8f0',
        borderRadius: 8,
        cursor: 'pointer',
        background: selected ? '#f7fafc' : 'white',
        transition: 'all 0.2s'
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>{name}</div>
          <div style={{ fontSize: 12, color: '#718096', fontFamily: 'monospace', wordBreak: 'break-all' }}>
            {keyPair.address.slice(0, 16)}...{keyPair.address.slice(-8)}
          </div>
          <div style={{ marginTop: 8, fontSize: 18, fontWeight: 700, color: '#667eea' }}>
            {balance !== undefined ? `${balance.toLocaleString()} tokens` : 'Loading...'}
          </div>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation()
            if (confirm(`Delete account "${name}"?`)) onDelete()
          }}
          style={{ background: '#fed7d7', color: '#c53030', padding: 6 }}
        >
          <Trash2 size={16} />
        </button>
      </div>
    </div>
  )
}

interface AccountDetailsProps {
  account: string
  keyPair: KeyPair
}

function AccountDetails({ account, keyPair }: AccountDetailsProps) {
  const [showPrivateKey, setShowPrivateKey] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)
  const queryClient = useQueryClient()

  const { data: accountInfo } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
    refetchInterval: 10000
  })

  const faucetMutation = useMutation({
    mutationFn: () => rpcClient.faucetRequest(keyPair.address),
    onSuccess: (response) => {
      if (response.success) {
        alert(`✅ ${response.message}\nAmount: ${response.amount} tokens\nNew balance: ${response.new_balance} tokens`)
        // Refresh account balance
        queryClient.invalidateQueries({ queryKey: ['accountInfo', keyPair.address] })
        queryClient.invalidateQueries({ queryKey: ['balance', keyPair.address] })
      } else {
        alert(`❌ ${response.message}${response.cooldown_remaining ? `\nTry again in ${response.cooldown_remaining} seconds` : ''}`)
      }
    },
    onError: (error: any) => {
      alert(`❌ Faucet request failed: ${error.message || 'Unknown error'}`)
    }
  })

  const copyToClipboard = (text: string, label: string) => {
    navigator.clipboard.writeText(text)
    setCopied(label)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <div className="card">
      <h3 className="card-header">Account Details: {account}</h3>

      <div style={{ marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Balance</div>
        <div style={{ fontSize: 24, fontWeight: 700, color: '#667eea' }}>
          {accountInfo ? `${accountInfo.balance.toLocaleString()} tokens` : 'Loading...'}
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Nonce</div>
        <div style={{ fontFamily: 'monospace' }}>{accountInfo?.nonce ?? 'Loading...'}</div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Address</div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <code style={{ flex: 1, fontSize: 11, wordBreak: 'break-all' }}>{keyPair.address}</code>
          <button onClick={() => copyToClipboard(keyPair.address, 'address')} style={{ padding: 6 }}>
            <Copy size={14} />
          </button>
        </div>
        {copied === 'address' && <div style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>}
      </div>

      <div style={{ marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Public Key</div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <code style={{ flex: 1, fontSize: 11, wordBreak: 'break-all' }}>{keyPair.publicKey}</code>
          <button onClick={() => copyToClipboard(keyPair.publicKey, 'public')} style={{ padding: 6 }}>
            <Copy size={14} />
          </button>
        </div>
        {copied === 'public' && <div style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>}
      </div>

      <div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
          <div style={{ fontSize: 12, color: '#718096' }}>Private Key</div>
          <button
            onClick={() => setShowPrivateKey(!showPrivateKey)}
            style={{ padding: 4, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            {showPrivateKey ? <><EyeOff size={12} /> Hide</> : <><Eye size={12} /> Show</>}
          </button>
        </div>
        {showPrivateKey ? (
          <>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <code style={{ flex: 1, fontSize: 11, wordBreak: 'break-all', background: '#fed7d7' }}>
                {keyPair.privateKey}
              </code>
              <button onClick={() => copyToClipboard(keyPair.privateKey, 'private')} style={{ padding: 6 }}>
                <Copy size={14} />
              </button>
            </div>
            {copied === 'private' && <div style={{ fontSize: 12, color: '#48bb78', marginTop: 4 }}>✓ Copied!</div>}
            <div style={{ fontSize: 11, color: '#c53030', marginTop: 4 }}>
              ⚠️ Never share your private key!
            </div>
          </>
        ) : (
          <code style={{ fontSize: 11 }}>••••••••••••••••••••••••••••••••</code>
        )}
      </div>

      <button
        onClick={() => faucetMutation.mutate()}
        disabled={faucetMutation.isPending}
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
          cursor: faucetMutation.isPending ? 'not-allowed' : 'pointer'
        }}
      >
        💧 {faucetMutation.isPending ? 'Requesting...' : 'Request Testnet Tokens'}
      </button>
    </div>
  )
}

interface ModalProps {
  onClose: () => void
}

function NewAccountModal({ onClose, onCreated }: ModalProps & { onCreated: (name: string) => void }) {
  const [name, setName] = useState('')

  const handleCreate = () => {
    if (!name.trim()) {
      alert('Please enter an account name')
      return
    }

    const keyPair = generateKeyPair()
    KeyStore.save(name, keyPair)
    onCreated(name)
  }

  return (
    <Modal onClose={onClose}>
      <h3 style={{ marginBottom: 16, fontSize: 20, fontWeight: 600 }}>Create New Account</h3>
      <div style={{ marginBottom: 16 }}>
        <label style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>Account Name</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g., My Testnet Account"
          autoFocus
        />
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

function ImportAccountModal({ onClose, onImported }: ModalProps & { onImported: (name: string) => void }) {
  const [name, setName] = useState('')
  const [privateKey, setPrivateKey] = useState('')
  const [error, setError] = useState('')

  const handleImport = () => {
    if (!name.trim()) {
      setError('Please enter an account name')
      return
    }

    if (!privateKey.trim()) {
      setError('Please enter a private key')
      return
    }

    try {
      const keyPair = importKeyPair(privateKey.trim())
      KeyStore.save(name, keyPair)
      onImported(name)
    } catch (e: any) {
      setError(e.message || 'Invalid private key')
    }
  }

  return (
    <Modal onClose={onClose}>
      <h3 style={{ marginBottom: 16, fontSize: 20, fontWeight: 600 }}>Import Account</h3>
      {error && (
        <div style={{ padding: 12, background: '#fed7d7', color: '#c53030', borderRadius: 6, marginBottom: 16, fontSize: 14 }}>
          {error}
        </div>
      )}
      <div style={{ marginBottom: 16 }}>
        <label style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>Account Name</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g., Imported Account"
        />
      </div>
      <div style={{ marginBottom: 16 }}>
        <label style={{ display: 'block', marginBottom: 8, fontSize: 14 }}>Private Key (hex)</label>
        <textarea
          value={privateKey}
          onChange={(e) => setPrivateKey(e.target.value)}
          placeholder="Enter 64-character hex private key"
          rows={3}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
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


function Modal({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'white',
          borderRadius: 12,
          padding: 24,
          maxWidth: 500,
          width: '90%',
          maxHeight: '90vh',
          overflow: 'auto'
        }}
      >
        {children}
      </div>
    </div>
  )
}
