import { useState } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { RpcClient } from '../lib/rpc-client'
import {
  KeyPair,
  createSignedTransferTransaction,
  createSignedTimeLockTransaction,
  createSignedEscrowCreateTransaction,
  createSignedEscrowReleaseTransaction,
  createSignedEscrowRefundTransaction,
  createSignedChannelOpenTransaction,
  createSignedChannelCloseTransaction,
  createSignedPoolSwapTransaction,
  createSignedTrustLineCreateTransaction
} from '../lib/crypto'

const rpcClient = new RpcClient()

type TransactionType = 'transfer' | 'timelock' | 'escrow_create' | 'escrow_release' | 'escrow_refund' | 'channel_open' | 'channel_close' | 'trustline_create' | 'pool_swap' | 'marketplace_problem' | 'marketplace_solution'

interface TransactionModalProps {
  account: string
  keyPair: KeyPair
  onClose: () => void
}

export default function TransactionModal({ onClose, account, keyPair }: TransactionModalProps) {
  const [txType, setTxType] = useState<TransactionType>('transfer')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState(false)

  const { data: accountInfo } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
    refetchInterval: 5000
  })

  if (success) {
    return (
      <Modal onClose={onClose}>
        <div style={{ textAlign: 'center', padding: 40 }}>
          <div style={{ fontSize: 48, marginBottom: 16, color: '#48bb78' }}>✓</div>
          <h3 style={{ color: '#48bb78', marginBottom: 8 }}>Transaction Submitted!</h3>
          <p style={{ color: '#718096', fontSize: 14 }}>Your transaction has been broadcast to the network</p>
        </div>
      </Modal>
    )
  }

  return (
    <Modal onClose={onClose}>
      <h3 style={{ marginBottom: 16, fontSize: 20, fontWeight: 600 }}>Create Transaction</h3>

      <div style={{ marginBottom: 20, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096' }}>From: {account}</div>
        <div style={{ fontSize: 14, fontWeight: 600, marginTop: 4 }}>
          Balance: {accountInfo?.balance.toLocaleString() ?? '...'} tokens (Nonce: {accountInfo?.nonce ?? '...'})
        </div>
      </div>

      {error && (
        <div style={{ padding: 12, background: '#fed7d7', color: '#c53030', borderRadius: 6, marginBottom: 16, fontSize: 14 }}>
          {error}
        </div>
      )}

      <div style={{ marginBottom: 20 }}>
        <label style={{ display: 'block', marginBottom: 8, fontSize: 14, fontWeight: 600 }}>Transaction Type</label>
        <select
          value={txType}
          onChange={(e) => setTxType(e.target.value as TransactionType)}
          style={{ width: '100%', padding: 10, fontSize: 14 }}
        >
          <option value="transfer">💸 Transfer - Send tokens</option>
          <option value="timelock">⏰ Time-Lock - Lock funds until a future time</option>
          <option value="escrow_create">🔒 Escrow Create - Create escrow with arbiter</option>
          <option value="escrow_release">✅ Escrow Release - Release escrowed funds</option>
          <option value="escrow_refund">↩️ Escrow Refund - Refund escrowed funds</option>
          <option value="channel_open">📺 Channel Open - Open payment channel</option>
          <option value="channel_close">🚪 Channel Close - Close payment channel</option>
          <option value="trustline_create">🤝 TrustLine Create - Create bilateral trustline</option>
          <option value="pool_swap">🔄 Pool Swap - Swap between dimensional pools</option>
          <option value="marketplace_problem">🎯 Marketplace - Submit problem with bounty</option>
          <option value="marketplace_solution">💡 Marketplace - Submit solution to problem</option>
        </select>
      </div>

      {txType === 'transfer' && (
        <TransferForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'timelock' && (
        <TimeLockForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'escrow_create' && (
        <EscrowCreateForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'escrow_release' && (
        <EscrowReleaseForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'escrow_refund' && (
        <EscrowRefundForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'channel_open' && (
        <ChannelOpenForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'channel_close' && (
        <ChannelCloseForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'trustline_create' && (
        <TrustLineCreateForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'pool_swap' && (
        <PoolSwapForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'marketplace_problem' && (
        <MarketplaceProblemForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
      {txType === 'marketplace_solution' && (
        <MarketplaceSolutionForm keyPair={keyPair} accountInfo={accountInfo} onSuccess={() => setSuccess(true)} onError={setError} onClose={onClose} />
      )}
    </Modal>
  )
}

// Transfer Form
function TransferForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      // Create and sign transaction using proper Ed25519 signature
      const signedTxJson = createSignedTransferTransaction(
        keyPair.address,
        recipient,
        parseInt(amount),
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      // Submit as JSON (RPC server will deserialize)
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!recipient.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid recipient address (must be 64-character hex)')
      return
    }
    if (!amount || parseInt(amount) <= 0) {
      onError('Invalid amount')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Recipient Address" required>
        <input
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder="64-character hex address"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 16 }}>
        <FormField label="Amount" required>
          <input type="number" value={amount} onChange={(e) => setAmount(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} min="0" />
        </FormField>
      </div>
    </FormContainer>
  )
}

// TimeLock Form
function TimeLockForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [unlockInHours, setUnlockInHours] = useState('24')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const unlockTime = Math.floor(Date.now() / 1000) + (parseInt(unlockInHours) * 3600)
      const signedTxJson = createSignedTimeLockTransaction(
        keyPair.address,
        recipient,
        parseInt(amount),
        unlockTime,
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!recipient.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid recipient address (must be 64-character hex)')
      return
    }
    if (!amount || parseInt(amount) <= 0) {
      onError('Invalid amount')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Recipient Address" required>
        <input type="text" value={recipient} onChange={(e) => setRecipient(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 16 }}>
        <FormField label="Amount" required>
          <input type="number" value={amount} onChange={(e) => setAmount(e.target.value)} min="1" />
        </FormField>
        <FormField label="Unlock In (hours)" required>
          <input type="number" value={unlockInHours} onChange={(e) => setUnlockInHours(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <div style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        💡 Funds will be locked and become available to recipient after {unlockInHours} hours
      </div>
    </FormContainer>
  )
}

// Escrow Create Form
function EscrowCreateForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [arbiter, setArbiter] = useState('')
  const [amount, setAmount] = useState('')
  const [timeoutHours, setTimeoutHours] = useState('168') // 1 week default
  const [conditions, setConditions] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const timeout = Math.floor(Date.now() / 1000) + (parseInt(timeoutHours) * 3600)
      const signedTxJson = createSignedEscrowCreateTransaction(
        keyPair.address,
        recipient,
        arbiter || null,
        parseInt(amount),
        timeout,
        conditions,
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!recipient.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid recipient address')
      return
    }
    if (arbiter && !arbiter.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid arbiter address')
      return
    }
    if (!amount || parseInt(amount) <= 0) {
      onError('Invalid amount')
      return
    }
    if (!conditions.trim()) {
      onError('Please enter conditions/terms')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Recipient Address" required>
        <input type="text" value={recipient} onChange={(e) => setRecipient(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <FormField label="Arbiter Address (optional)">
        <input type="text" value={arbiter} onChange={(e) => setArbiter(e.target.value)} placeholder="Leave empty for no arbiter" style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Amount" required>
          <input type="number" value={amount} onChange={(e) => setAmount(e.target.value)} min="1" />
        </FormField>
        <FormField label="Timeout (hours)" required>
          <input type="number" value={timeoutHours} onChange={(e) => setTimeoutHours(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <FormField label="Conditions/Terms" required>
        <textarea value={conditions} onChange={(e) => setConditions(e.target.value)} rows={3} placeholder="Description of escrow conditions" />
      </FormField>
    </FormContainer>
  )
}

// Escrow Release/Refund Forms
function EscrowReleaseForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [escrowId, setEscrowId] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedEscrowReleaseTransaction(
        escrowId,
        keyPair.address,
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!escrowId.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid escrow ID (must be 64-character hex)')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Escrow ID" required>
        <input type="text" value={escrowId} onChange={(e) => setEscrowId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </FormField>
      <FormField label="Fee">
        <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
    </FormContainer>
  )
}

function EscrowRefundForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [escrowId, setEscrowId] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedEscrowRefundTransaction(
        escrowId,
        keyPair.address,
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!escrowId.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid escrow ID (must be 64-character hex)')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Escrow ID" required>
        <input type="text" value={escrowId} onChange={(e) => setEscrowId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </FormField>
      <FormField label="Fee">
        <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
    </FormContainer>
  )
}

// Channel Forms
function ChannelOpenForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [participantB, setParticipantB] = useState('')
  const [depositA, setDepositA] = useState('')
  const [depositB, setDepositB] = useState('')
  const [timeoutSeconds, setTimeoutSeconds] = useState('86400') // 1 day
  const [fee, setFee] = useState('2000')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedChannelOpenTransaction(
        keyPair.address,
        participantB,
        parseInt(depositA),
        parseInt(depositB),
        parseInt(timeoutSeconds),
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!participantB.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid participant B address')
      return
    }
    if (!depositA || parseInt(depositA) <= 0) {
      onError('Invalid deposit A amount')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Participant B Address" required>
        <input type="text" value={participantB} onChange={(e) => setParticipantB(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="Your Deposit (A)" required>
          <input type="number" value={depositA} onChange={(e) => setDepositA(e.target.value)} min="1" />
        </FormField>
        <FormField label="Their Deposit (B)" required>
          <input type="number" value={depositB} onChange={(e) => setDepositB(e.target.value)} min="0" />
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12 }}>
        <FormField label="Timeout (seconds)" required>
          <input type="number" value={timeoutSeconds} onChange={(e) => setTimeoutSeconds(e.target.value)} min="60" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
    </FormContainer>
  )
}

function ChannelCloseForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [channelId, setChannelId] = useState('')
  const [finalBalanceA, setFinalBalanceA] = useState('')
  const [finalBalanceB, setFinalBalanceB] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedChannelCloseTransaction(
        channelId,
        keyPair.address,
        parseInt(finalBalanceA),
        parseInt(finalBalanceB),
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!channelId.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid channel ID (must be 64-character hex)')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Channel ID" required>
        <input type="text" value={channelId} onChange={(e) => setChannelId(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Final Balance A" required>
          <input type="number" value={finalBalanceA} onChange={(e) => setFinalBalanceA(e.target.value)} min="0" />
        </FormField>
        <FormField label="Final Balance B" required>
          <input type="number" value={finalBalanceB} onChange={(e) => setFinalBalanceB(e.target.value)} min="0" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
    </FormContainer>
  )
}

// TrustLine Form
function TrustLineCreateForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [accountB, setAccountB] = useState('')
  const [limitAtoB, setLimitAtoB] = useState('')
  const [limitBtoA, setLimitBtoA] = useState('')
  const [dimensionalScale, setDimensionalScale] = useState('3')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedTrustLineCreateTransaction(
        keyPair.address,
        accountB,
        parseInt(limitAtoB),
        parseInt(limitBtoA),
        parseInt(dimensionalScale),
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (!accountB.match(/^[0-9a-f]{64}$/i)) {
      onError('Invalid account B address')
      return
    }
    if (!limitAtoB || parseInt(limitAtoB) <= 0) {
      onError('Invalid credit limit A→B')
      return
    }
    if (!limitBtoA || parseInt(limitBtoA) <= 0) {
      onError('Invalid credit limit B→A')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Account B Address" required>
        <input type="text" value={accountB} onChange={(e) => setAccountB(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="Credit Limit A→B" required>
          <input type="number" value={limitAtoB} onChange={(e) => setLimitAtoB(e.target.value)} min="0" />
        </FormField>
        <FormField label="Credit Limit B→A" required>
          <input type="number" value={limitBtoA} onChange={(e) => setLimitBtoA(e.target.value)} min="0" />
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12 }}>
        <FormField label="Dimensional Scale (1-8)" required>
          <input type="number" value={dimensionalScale} onChange={(e) => setDimensionalScale(e.target.value)} min="1" max="8" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <div style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        💡 Creates bilateral trustline with dimensional economics (η = λ = 1/√2)
      </div>
    </FormContainer>
  )
}

// Pool Swap Form
function PoolSwapForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [poolFrom, setPoolFrom] = useState('D1')
  const [poolTo, setPoolTo] = useState('D2')
  const [amountIn, setAmountIn] = useState('')
  const [minAmountOut, setMinAmountOut] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedPoolSwapTransaction(
        keyPair.address,
        poolFrom as 'D1' | 'D2' | 'D3',
        poolTo as 'D1' | 'D2' | 'D3',
        parseInt(amountIn),
        parseInt(minAmountOut),
        parseInt(fee),
        accountInfo!.nonce,
        keyPair.privateKey,
        keyPair.publicKey
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: any) => onError(e.message || 'Transaction failed')
  })

  const handleSubmit = () => {
    if (poolFrom === poolTo) {
      onError('Cannot swap between the same pool')
      return
    }
    if (!amountIn || parseInt(amountIn) <= 0) {
      onError('Invalid amount to swap')
      return
    }
    if (!minAmountOut || parseInt(minAmountOut) <= 0) {
      onError('Invalid minimum amount out')
      return
    }
    sendMutation.mutate()
  }

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="From Pool" required>
          <select value={poolFrom} onChange={(e) => setPoolFrom(e.target.value)}>
            <option value="D1">D₁ Genesis (τ=0.00, D=1.000)</option>
            <option value="D2">D₂ Coupling (τ=0.20, D=0.867)</option>
            <option value="D3">D₃ Harmonic (τ=0.41, D=0.750)</option>
          </select>
        </FormField>
        <FormField label="To Pool" required>
          <select value={poolTo} onChange={(e) => setPoolTo(e.target.value)}>
            <option value="D1">D₁ Genesis (τ=0.00, D=1.000)</option>
            <option value="D2">D₂ Coupling (τ=0.20, D=0.867)</option>
            <option value="D3">D₃ Harmonic (τ=0.41, D=0.750)</option>
          </select>
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Amount In" required>
          <input type="number" value={amountIn} onChange={(e) => setAmountIn(e.target.value)} min="1" />
        </FormField>
        <FormField label="Min Amount Out" required>
          <input type="number" value={minAmountOut} onChange={(e) => setMinAmountOut(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <div style={{ fontSize: 12, color: '#667eea', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        💎 Dimensional pool swap using exponential tokenomics (Dn = e^(-η·τn))
      </div>
    </FormContainer>
  )
}

// Marketplace Forms
function MarketplaceProblemForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [problemType, setProblemType] = useState('TSP')
  const [bounty, setBounty] = useState('')
  const [minWorkScore, setMinWorkScore] = useState('0.5')
  const [expirationDays, setExpirationDays] = useState('30')
  const [problemData, setProblemData] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      return Promise.reject(new Error('Marketplace operations not yet implemented in GUI'))
    },
    onSuccess,
    onError: (e: any) => onError(e.message)
  })

  return (
    <FormContainer onSubmit={() => sendMutation.mutate()} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Problem Type" required>
        <select value={problemType} onChange={(e) => setProblemType(e.target.value)}>
          <option value="TSP">TSP - Traveling Salesman Problem</option>
          <option value="SAT">SAT - Boolean Satisfiability</option>
          <option value="Knapsack">Knapsack - 0/1 Knapsack Problem</option>
        </select>
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Bounty" required>
          <input type="number" value={bounty} onChange={(e) => setBounty(e.target.value)} min="1" />
        </FormField>
        <FormField label="Min Work Score" required>
          <input type="number" step="0.01" value={minWorkScore} onChange={(e) => setMinWorkScore(e.target.value)} min="0" max="1" />
        </FormField>
        <FormField label="Expires (days)" required>
          <input type="number" value={expirationDays} onChange={(e) => setExpirationDays(e.target.value)} min="1" max="365" />
        </FormField>
        <FormField label="Fee">
          <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <FormField label="Problem Data (JSON)" required>
        <textarea value={problemData} onChange={(e) => setProblemData(e.target.value)} rows={4} placeholder='{"cities": 10, "distances": [...]}' style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <div style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        🎯 Submit NP-hard problem with bounty. Solvers earn bounty by providing valid solutions meeting minimum work score.
      </div>
    </FormContainer>
  )
}

function MarketplaceSolutionForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [problemId, setProblemId] = useState('')
  const [solutionData, setSolutionData] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      return Promise.reject(new Error('Marketplace operations not yet implemented in GUI'))
    },
    onSuccess,
    onError: (e: any) => onError(e.message)
  })

  return (
    <FormContainer onSubmit={() => sendMutation.mutate()} onClose={onClose} isPending={sendMutation.isPending}>
      <FormField label="Problem ID" required>
        <input type="text" value={problemId} onChange={(e) => setProblemId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </FormField>
      <FormField label="Solution Data (JSON)" required>
        <textarea value={solutionData} onChange={(e) => setSolutionData(e.target.value)} rows={4} placeholder='{"route": [0, 1, 2, ...]}' style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <FormField label="Fee">
        <input type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
      <div style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        💡 Submit solution to open problem. Bounty auto-paid if solution meets minimum work score requirements.
      </div>
    </FormContainer>
  )
}

// Helper Components
interface FormProps {
  keyPair: KeyPair
  accountInfo: any
  onSuccess: () => void
  onError: (error: string) => void
  onClose: () => void
}

function FormContainer({ children, onSubmit, onClose, isPending }: { children: React.ReactNode, onSubmit: () => void, onClose: () => void, isPending: boolean }) {
  return (
    <>
      {children}
      <div style={{ display: 'flex', gap: 8, marginTop: 20 }}>
        <button onClick={onClose} style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }} disabled={isPending}>
          Cancel
        </button>
        <button onClick={onSubmit} style={{ flex: 1 }} disabled={isPending}>
          {isPending ? 'Submitting...' : 'Submit Transaction'}
        </button>
      </div>
    </>
  )
}

function FormField({ label, required, children }: { label: string, required?: boolean, children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 16 }}>
      <label style={{ display: 'block', marginBottom: 8, fontSize: 13, fontWeight: 500 }}>
        {label} {required && <span style={{ color: '#e53e3e' }}>*</span>}
      </label>
      {children}
    </div>
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
        background: 'rgba(0,0,0,0.6)',
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
          maxWidth: 600,
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

function hexToArray(hex: string): number[] {
  const bytes = hex.match(/.{1,2}/g)?.map(byte => parseInt(byte, 16)) || []
  return bytes
}
