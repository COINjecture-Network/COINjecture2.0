import { useState, useEffect } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { Loader2 } from 'lucide-react'
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
  createSignedTrustLineCreateTransaction,
} from '../lib/crypto'

const rpcClient = new RpcClient()

// ── Types ─────────────────────────────────────────────────────────────────────

type TransactionType =
  | 'transfer' | 'timelock'
  | 'escrow_create' | 'escrow_release' | 'escrow_refund'
  | 'channel_open' | 'channel_close'
  | 'trustline_create' | 'pool_swap'
  | 'marketplace_problem' | 'marketplace_solution'

interface TransactionModalProps {
  account: string
  keyPair: KeyPair
  onClose: () => void
}

interface FormProps {
  keyPair: KeyPair
  accountInfo: { balance: number; nonce: number } | undefined
  onSuccess: () => void
  onError: (error: string) => void
  onClose: () => void
}

/** A row in the pre-submission confirmation summary. */
interface SummaryRow {
  label: string
  value: string
  /** Highlight this row (e.g. total cost). */
  highlight?: boolean
}

// ── Validation helpers ────────────────────────────────────────────────────────

function validateAddress(addr: string, label = 'Address'): string | null {
  if (!addr.trim()) return `${label} is required`
  if (!/^[0-9a-fA-F]{64}$/.test(addr.trim())) return `${label} must be a 64-character hex string`
  return null
}

function validateAmount(val: string, label = 'Amount', min = 1): string | null {
  if (!val.trim()) return `${label} is required`
  const n = parseInt(val, 10)
  if (isNaN(n) || n < min) return `${label} must be at least ${min}`
  return null
}

// ── Transaction modal shell ───────────────────────────────────────────────────

export default function TransactionModal({ onClose, account, keyPair }: TransactionModalProps) {
  const [txType, setTxType] = useState<TransactionType>('transfer')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState(false)

  const { data: accountInfo, isLoading: infoLoading } = useQuery({
    queryKey: ['accountInfo', keyPair.address],
    queryFn: () => rpcClient.getAccountInfo(keyPair.address),
    refetchInterval: 5000,
  })

  if (success) {
    return (
      <Modal onClose={onClose}>
        <div style={{ textAlign: 'center', padding: 40 }} role="status" aria-live="polite">
          <div style={{ fontSize: 48, marginBottom: 16, color: '#48bb78' }} aria-hidden="true">✓</div>
          <h3 style={{ color: '#48bb78', marginBottom: 8 }}>Transaction Submitted!</h3>
          <p style={{ color: '#718096', fontSize: 14 }}>
            Your transaction has been broadcast to the network.
          </p>
          <button onClick={onClose} style={{ marginTop: 20 }} autoFocus>
            Close
          </button>
        </div>
      </Modal>
    )
  }

  const formProps: FormProps = {
    keyPair,
    accountInfo,
    onSuccess: () => setSuccess(true),
    onError: setError,
    onClose,
  }

  return (
    <Modal onClose={onClose}>
      <h3 id="tx-modal-title" style={{ marginBottom: 16, fontSize: 20, fontWeight: 600 }}>
        Create Transaction
      </h3>

      {/* Account summary */}
      <div style={{ marginBottom: 20, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096' }}>From: {account}</div>
        <div style={{ fontSize: 14, fontWeight: 600, marginTop: 4 }} aria-live="polite">
          {infoLoading ? (
            <span className="skeleton" style={{ display: 'inline-block', width: 200, height: 18 }} aria-hidden="true" />
          ) : (
            <>Balance: {accountInfo?.balance.toLocaleString() ?? '—'} tokens (Nonce: {accountInfo?.nonce ?? '—'})</>
          )}
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          role="alert"
          style={{ padding: 12, background: '#fed7d7', color: '#c53030', borderRadius: 6, marginBottom: 16, fontSize: 14 }}
        >
          {error}
          <button
            onClick={() => setError('')}
            aria-label="Dismiss error"
            style={{ float: 'right', background: 'transparent', color: '#c53030', padding: 0, fontSize: 16 }}
          >
            ×
          </button>
        </div>
      )}

      {/* Transaction type selector */}
      <div style={{ marginBottom: 20 }}>
        <label htmlFor="tx-type-select" style={{ display: 'block', marginBottom: 8, fontSize: 14, fontWeight: 600 }}>
          Transaction Type
        </label>
        <select
          id="tx-type-select"
          value={txType}
          onChange={(e) => { setTxType(e.target.value as TransactionType); setError('') }}
          style={{ width: '100%', padding: 10, fontSize: 14 }}
        >
          <option value="transfer">Transfer — Send tokens</option>
          <option value="timelock">Time-Lock — Lock funds until a future time</option>
          <option value="escrow_create">Escrow Create — Create escrow with arbiter</option>
          <option value="escrow_release">Escrow Release — Release escrowed funds</option>
          <option value="escrow_refund">Escrow Refund — Refund escrowed funds</option>
          <option value="channel_open">Channel Open — Open payment channel</option>
          <option value="channel_close">Channel Close — Close payment channel</option>
          <option value="trustline_create">TrustLine Create — Create bilateral trustline</option>
          <option value="pool_swap">Pool Swap — Swap between dimensional pools</option>
          <option value="marketplace_problem">Marketplace — Submit problem with bounty</option>
          <option value="marketplace_solution">Marketplace — Submit solution to problem</option>
        </select>
      </div>

      {txType === 'transfer'            && <TransferForm       {...formProps} />}
      {txType === 'timelock'            && <TimeLockForm        {...formProps} />}
      {txType === 'escrow_create'       && <EscrowCreateForm    {...formProps} />}
      {txType === 'escrow_release'      && <EscrowReleaseForm   {...formProps} />}
      {txType === 'escrow_refund'       && <EscrowRefundForm    {...formProps} />}
      {txType === 'channel_open'        && <ChannelOpenForm     {...formProps} />}
      {txType === 'channel_close'       && <ChannelCloseForm    {...formProps} />}
      {txType === 'trustline_create'    && <TrustLineCreateForm {...formProps} />}
      {txType === 'pool_swap'           && <PoolSwapForm        {...formProps} />}
      {txType === 'marketplace_problem' && <MarketplaceProblemForm {...formProps} />}
      {txType === 'marketplace_solution' && <MarketplaceSolutionForm {...formProps} />}
    </Modal>
  )
}

// ── TransferForm ──────────────────────────────────────────────────────────────

function TransferForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedTransferTransaction(
        keyPair.address, recipient.trim(),
        parseInt(amount, 10), parseInt(fee, 10),
        accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(recipient, 'Recipient address')
    if (err) return onError(err)
    const amtErr = validateAmount(amount, 'Amount')
    if (amtErr) return onError(amtErr)
    const feeErr = validateAmount(fee, 'Fee', 0)
    if (feeErr) return onError(feeErr)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Recipient', value: recipient.trim() ? `${recipient.slice(0, 12)}…${recipient.slice(-8)}` : '—' },
    { label: 'Amount', value: `${parseInt(amount || '0', 10).toLocaleString()} tokens` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
    { label: 'Total', value: `${(parseInt(amount || '0', 10) + parseInt(fee || '0', 10)).toLocaleString()} tokens`, highlight: true },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Recipient Address" fieldId="tf-recipient" required>
        <input
          id="tf-recipient"
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder="64-character hex address"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
          aria-required="true"
          autoComplete="off"
          spellCheck={false}
          maxLength={64}
        />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 16 }}>
        <FormField label="Amount" fieldId="tf-amount" required>
          <input
            id="tf-amount"
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            min="1"
            aria-required="true"
          />
        </FormField>
        <FormField label="Fee" fieldId="tf-fee">
          <input
            id="tf-fee"
            type="number"
            value={fee}
            onChange={(e) => setFee(e.target.value)}
            min="0"
          />
        </FormField>
      </div>
    </FormContainer>
  )
}

// ── TimeLockForm ──────────────────────────────────────────────────────────────

function TimeLockForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [unlockInHours, setUnlockInHours] = useState('24')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const unlockTime = Math.floor(Date.now() / 1000) + parseInt(unlockInHours, 10) * 3600
      const signedTxJson = createSignedTimeLockTransaction(
        keyPair.address, recipient.trim(),
        parseInt(amount, 10), unlockTime,
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(recipient, 'Recipient address')
    if (err) return onError(err)
    const amtErr = validateAmount(amount, 'Amount')
    if (amtErr) return onError(amtErr)
    const hoursNum = parseInt(unlockInHours, 10)
    if (isNaN(hoursNum) || hoursNum < 1) return onError('Unlock time must be at least 1 hour')
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Recipient', value: recipient.trim() ? `${recipient.slice(0, 12)}…${recipient.slice(-8)}` : '—' },
    { label: 'Amount', value: `${parseInt(amount || '0', 10).toLocaleString()} tokens` },
    { label: 'Unlock after', value: `${unlockInHours} hours` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Recipient Address" fieldId="tl-recipient" required>
        <input
          id="tl-recipient"
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder="64-character hex"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
          autoComplete="off"
          spellCheck={false}
        />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 16 }}>
        <FormField label="Amount" fieldId="tl-amount" required>
          <input id="tl-amount" type="number" value={amount} onChange={(e) => setAmount(e.target.value)} min="1" />
        </FormField>
        <FormField label="Unlock In (hours)" fieldId="tl-hours" required>
          <input id="tl-hours" type="number" value={unlockInHours} onChange={(e) => setUnlockInHours(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee" fieldId="tl-fee">
          <input id="tl-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <p style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        Funds will be locked and become available to recipient after {unlockInHours} hours.
      </p>
    </FormContainer>
  )
}

// ── EscrowCreateForm ──────────────────────────────────────────────────────────

function EscrowCreateForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [recipient, setRecipient] = useState('')
  const [arbiter, setArbiter] = useState('')
  const [amount, setAmount] = useState('')
  const [timeoutHours, setTimeoutHours] = useState('168')
  const [conditions, setConditions] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const timeout = Math.floor(Date.now() / 1000) + parseInt(timeoutHours, 10) * 3600
      const signedTxJson = createSignedEscrowCreateTransaction(
        keyPair.address, recipient.trim(), arbiter.trim() || null,
        parseInt(amount, 10), timeout, conditions,
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const recipientErr = validateAddress(recipient, 'Recipient address')
    if (recipientErr) return onError(recipientErr)
    if (arbiter.trim()) {
      const arbiterErr = validateAddress(arbiter, 'Arbiter address')
      if (arbiterErr) return onError(arbiterErr)
    }
    const amtErr = validateAmount(amount, 'Amount')
    if (amtErr) return onError(amtErr)
    if (!conditions.trim()) return onError('Conditions/terms are required')
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Recipient', value: recipient.trim() ? `${recipient.slice(0, 12)}…${recipient.slice(-8)}` : '—' },
    { label: 'Arbiter', value: arbiter.trim() ? `${arbiter.slice(0, 12)}…` : 'None' },
    { label: 'Amount', value: `${parseInt(amount || '0', 10).toLocaleString()} tokens` },
    { label: 'Timeout', value: `${timeoutHours} hours` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Recipient Address" fieldId="ec-recipient" required>
        <input id="ec-recipient" type="text" value={recipient} onChange={(e) => setRecipient(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} autoComplete="off" />
      </FormField>
      <FormField label="Arbiter Address (optional)" fieldId="ec-arbiter">
        <input id="ec-arbiter" type="text" value={arbiter} onChange={(e) => setArbiter(e.target.value)} placeholder="Leave empty for no arbiter" style={{ fontFamily: 'monospace', fontSize: 11 }} autoComplete="off" />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Amount" fieldId="ec-amount" required>
          <input id="ec-amount" type="number" value={amount} onChange={(e) => setAmount(e.target.value)} min="1" />
        </FormField>
        <FormField label="Timeout (hours)" fieldId="ec-timeout" required>
          <input id="ec-timeout" type="number" value={timeoutHours} onChange={(e) => setTimeoutHours(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee" fieldId="ec-fee">
          <input id="ec-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <FormField label="Conditions / Terms" fieldId="ec-conditions" required>
        <textarea id="ec-conditions" value={conditions} onChange={(e) => setConditions(e.target.value)} rows={3} placeholder="Description of escrow conditions" />
      </FormField>
    </FormContainer>
  )
}

// ── EscrowReleaseForm ─────────────────────────────────────────────────────────

function EscrowReleaseForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [escrowId, setEscrowId] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedEscrowReleaseTransaction(
        escrowId.trim(), keyPair.address,
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(escrowId, 'Escrow ID')
    if (err) return onError(err)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Escrow ID', value: escrowId.trim() ? `${escrowId.slice(0, 12)}…${escrowId.slice(-8)}` : '—' },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Escrow ID" fieldId="er-id" required>
        <input id="er-id" type="text" value={escrowId} onChange={(e) => setEscrowId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} autoComplete="off" />
      </FormField>
      <FormField label="Fee" fieldId="er-fee">
        <input id="er-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
    </FormContainer>
  )
}

// ── EscrowRefundForm ──────────────────────────────────────────────────────────

function EscrowRefundForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [escrowId, setEscrowId] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedEscrowRefundTransaction(
        escrowId.trim(), keyPair.address,
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(escrowId, 'Escrow ID')
    if (err) return onError(err)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Escrow ID', value: escrowId.trim() ? `${escrowId.slice(0, 12)}…${escrowId.slice(-8)}` : '—' },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Escrow ID" fieldId="erf-id" required>
        <input id="erf-id" type="text" value={escrowId} onChange={(e) => setEscrowId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} autoComplete="off" />
      </FormField>
      <FormField label="Fee" fieldId="erf-fee">
        <input id="erf-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
    </FormContainer>
  )
}

// ── ChannelOpenForm ───────────────────────────────────────────────────────────

function ChannelOpenForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [participantB, setParticipantB] = useState('')
  const [depositA, setDepositA] = useState('')
  const [depositB, setDepositB] = useState('')
  const [timeoutSeconds, setTimeoutSeconds] = useState('86400')
  const [fee, setFee] = useState('2000')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedChannelOpenTransaction(
        keyPair.address, participantB.trim(),
        parseInt(depositA, 10), parseInt(depositB, 10),
        parseInt(timeoutSeconds, 10),
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(participantB, 'Participant B address')
    if (err) return onError(err)
    const dErr = validateAmount(depositA, 'Deposit A')
    if (dErr) return onError(dErr)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Participant B', value: participantB.trim() ? `${participantB.slice(0, 12)}…` : '—' },
    { label: 'Your deposit', value: `${parseInt(depositA || '0', 10).toLocaleString()} tokens` },
    { label: 'Their deposit', value: `${parseInt(depositB || '0', 10).toLocaleString()} tokens` },
    { label: 'Timeout', value: `${timeoutSeconds}s` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Participant B Address" fieldId="co-partb" required>
        <input id="co-partb" type="text" value={participantB} onChange={(e) => setParticipantB(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} autoComplete="off" />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="Your Deposit (A)" fieldId="co-da" required>
          <input id="co-da" type="number" value={depositA} onChange={(e) => setDepositA(e.target.value)} min="1" />
        </FormField>
        <FormField label="Their Deposit (B)" fieldId="co-db" required>
          <input id="co-db" type="number" value={depositB} onChange={(e) => setDepositB(e.target.value)} min="0" />
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12 }}>
        <FormField label="Timeout (seconds)" fieldId="co-timeout" required>
          <input id="co-timeout" type="number" value={timeoutSeconds} onChange={(e) => setTimeoutSeconds(e.target.value)} min="60" />
        </FormField>
        <FormField label="Fee" fieldId="co-fee">
          <input id="co-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
    </FormContainer>
  )
}

// ── ChannelCloseForm ──────────────────────────────────────────────────────────

function ChannelCloseForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [channelId, setChannelId] = useState('')
  const [finalBalanceA, setFinalBalanceA] = useState('')
  const [finalBalanceB, setFinalBalanceB] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedChannelCloseTransaction(
        channelId.trim(), keyPair.address,
        parseInt(finalBalanceA, 10), parseInt(finalBalanceB, 10),
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(channelId, 'Channel ID')
    if (err) return onError(err)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Channel ID', value: channelId.trim() ? `${channelId.slice(0, 12)}…` : '—' },
    { label: 'Final Balance A', value: `${parseInt(finalBalanceA || '0', 10).toLocaleString()} tokens` },
    { label: 'Final Balance B', value: `${parseInt(finalBalanceB || '0', 10).toLocaleString()} tokens` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Channel ID" fieldId="cc-id" required>
        <input id="cc-id" type="text" value={channelId} onChange={(e) => setChannelId(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} autoComplete="off" />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Final Balance A" fieldId="cc-bala" required>
          <input id="cc-bala" type="number" value={finalBalanceA} onChange={(e) => setFinalBalanceA(e.target.value)} min="0" />
        </FormField>
        <FormField label="Final Balance B" fieldId="cc-balb" required>
          <input id="cc-balb" type="number" value={finalBalanceB} onChange={(e) => setFinalBalanceB(e.target.value)} min="0" />
        </FormField>
        <FormField label="Fee" fieldId="cc-fee">
          <input id="cc-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
    </FormContainer>
  )
}

// ── TrustLineCreateForm ───────────────────────────────────────────────────────

function TrustLineCreateForm({ keyPair, accountInfo, onSuccess, onError, onClose }: FormProps) {
  const [accountB, setAccountB] = useState('')
  const [limitAtoB, setLimitAtoB] = useState('')
  const [limitBtoA, setLimitBtoA] = useState('')
  const [dimensionalScale, setDimensionalScale] = useState('3')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => {
      const signedTxJson = createSignedTrustLineCreateTransaction(
        keyPair.address, accountB.trim(),
        parseInt(limitAtoB, 10), parseInt(limitBtoA, 10),
        parseInt(dimensionalScale, 10),
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    const err = validateAddress(accountB, 'Account B address')
    if (err) return onError(err)
    if (!limitAtoB || parseInt(limitAtoB, 10) <= 0) return onError('Invalid credit limit A→B')
    if (!limitBtoA || parseInt(limitBtoA, 10) <= 0) return onError('Invalid credit limit B→A')
    const ds = parseInt(dimensionalScale, 10)
    if (isNaN(ds) || ds < 1 || ds > 8) return onError('Dimensional scale must be 1–8')
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'Account B', value: accountB.trim() ? `${accountB.slice(0, 12)}…` : '—' },
    { label: 'Limit A→B', value: `${parseInt(limitAtoB || '0', 10).toLocaleString()} tokens` },
    { label: 'Limit B→A', value: `${parseInt(limitBtoA || '0', 10).toLocaleString()} tokens` },
    { label: 'Dimensional scale', value: `D${dimensionalScale}` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Account B Address" fieldId="tl-acctb" required>
        <input id="tl-acctb" type="text" value={accountB} onChange={(e) => setAccountB(e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} autoComplete="off" />
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="Credit Limit A→B" fieldId="tl-lab" required>
          <input id="tl-lab" type="number" value={limitAtoB} onChange={(e) => setLimitAtoB(e.target.value)} min="0" />
        </FormField>
        <FormField label="Credit Limit B→A" fieldId="tl-lba" required>
          <input id="tl-lba" type="number" value={limitBtoA} onChange={(e) => setLimitBtoA(e.target.value)} min="0" />
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12 }}>
        <FormField label="Dimensional Scale (1–8)" fieldId="tl-ds" required>
          <input id="tl-ds" type="number" value={dimensionalScale} onChange={(e) => setDimensionalScale(e.target.value)} min="1" max="8" />
        </FormField>
        <FormField label="Fee" fieldId="tl-fee">
          <input id="tl-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <p style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        Creates bilateral trustline with dimensional economics (η = λ = 1/√2).
      </p>
    </FormContainer>
  )
}

// ── PoolSwapForm ──────────────────────────────────────────────────────────────

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
        parseInt(amountIn, 10), parseInt(minAmountOut, 10),
        parseInt(fee, 10), accountInfo!.nonce,
        keyPair.privateKey, keyPair.publicKey,
      )
      return rpcClient.submitTransaction(signedTxJson)
    },
    onSuccess,
    onError: (e: Error) => onError(e.message || 'Transaction failed'),
  })

  const handleSubmit = () => {
    if (poolFrom === poolTo) return onError('Cannot swap between the same pool')
    const ainErr = validateAmount(amountIn, 'Amount in')
    if (ainErr) return onError(ainErr)
    const aoutErr = validateAmount(minAmountOut, 'Minimum amount out')
    if (aoutErr) return onError(aoutErr)
    sendMutation.mutate()
  }

  const summary: SummaryRow[] = [
    { label: 'From pool', value: poolFrom },
    { label: 'To pool', value: poolTo },
    { label: 'Amount in', value: `${parseInt(amountIn || '0', 10).toLocaleString()} tokens` },
    { label: 'Min amount out', value: `${parseInt(minAmountOut || '0', 10).toLocaleString()} tokens` },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={handleSubmit} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <FormField label="From Pool" fieldId="ps-from" required>
          <select id="ps-from" value={poolFrom} onChange={(e) => setPoolFrom(e.target.value)}>
            <option value="D1">D₁ Genesis (τ=0.00)</option>
            <option value="D2">D₂ Coupling (τ=0.20)</option>
            <option value="D3">D₃ Harmonic (τ=0.41)</option>
          </select>
        </FormField>
        <FormField label="To Pool" fieldId="ps-to" required>
          <select id="ps-to" value={poolTo} onChange={(e) => setPoolTo(e.target.value)}>
            <option value="D1">D₁ Genesis (τ=0.00)</option>
            <option value="D2">D₂ Coupling (τ=0.20)</option>
            <option value="D3">D₃ Harmonic (τ=0.41)</option>
          </select>
        </FormField>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Amount In" fieldId="ps-in" required>
          <input id="ps-in" type="number" value={amountIn} onChange={(e) => setAmountIn(e.target.value)} min="1" />
        </FormField>
        <FormField label="Min Amount Out" fieldId="ps-out" required>
          <input id="ps-out" type="number" value={minAmountOut} onChange={(e) => setMinAmountOut(e.target.value)} min="1" />
        </FormField>
        <FormField label="Fee" fieldId="ps-fee">
          <input id="ps-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <p style={{ fontSize: 12, color: '#667eea', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        Dimensional pool swap using exponential tokenomics (Dₙ = e^(-η·τₙ)).
      </p>
    </FormContainer>
  )
}

// ── MarketplaceProblemForm ────────────────────────────────────────────────────

function MarketplaceProblemForm({ onSuccess, onError, onClose }: FormProps) {
  const [problemType, setProblemType] = useState('TSP')
  const [bounty, setBounty] = useState('')
  const [minWorkScore, setMinWorkScore] = useState('0.5')
  const [expirationDays, setExpirationDays] = useState('30')
  const [problemData, setProblemData] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => Promise.reject(new Error('Use the Marketplace tab to submit problems')),
    onSuccess,
    onError: (e: Error) => onError(e.message),
  })

  const summary: SummaryRow[] = [
    { label: 'Problem type', value: problemType },
    { label: 'Bounty', value: `${parseInt(bounty || '0', 10).toLocaleString()} tokens` },
    { label: 'Min work score', value: minWorkScore },
    { label: 'Expires in', value: `${expirationDays} days` },
  ]

  return (
    <FormContainer onSubmit={() => sendMutation.mutate()} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Problem Type" fieldId="mp-type" required>
        <select id="mp-type" value={problemType} onChange={(e) => setProblemType(e.target.value)}>
          <option value="TSP">TSP — Traveling Salesman Problem</option>
          <option value="SAT">SAT — Boolean Satisfiability</option>
          <option value="Knapsack">Knapsack — 0/1 Knapsack Problem</option>
        </select>
      </FormField>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr', gap: 12 }}>
        <FormField label="Bounty" fieldId="mp-bounty" required>
          <input id="mp-bounty" type="number" value={bounty} onChange={(e) => setBounty(e.target.value)} min="1" />
        </FormField>
        <FormField label="Min Work Score" fieldId="mp-wsc" required>
          <input id="mp-wsc" type="number" step="0.01" value={minWorkScore} onChange={(e) => setMinWorkScore(e.target.value)} min="0" max="1" />
        </FormField>
        <FormField label="Expires (days)" fieldId="mp-exp" required>
          <input id="mp-exp" type="number" value={expirationDays} onChange={(e) => setExpirationDays(e.target.value)} min="1" max="365" />
        </FormField>
        <FormField label="Fee" fieldId="mp-fee">
          <input id="mp-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
        </FormField>
      </div>
      <FormField label="Problem Data (JSON)" fieldId="mp-data" required>
        <textarea id="mp-data" value={problemData} onChange={(e) => setProblemData(e.target.value)} rows={4} placeholder='{"cities": 10, "distances": [...]}' style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <p style={{ fontSize: 12, color: '#718096', padding: 10, background: '#f7fafc', borderRadius: 4 }}>
        Tip: Use the Marketplace tab for the full bounty submission UI with privacy support.
      </p>
    </FormContainer>
  )
}

// ── MarketplaceSolutionForm ───────────────────────────────────────────────────

function MarketplaceSolutionForm({ onSuccess, onError, onClose }: FormProps) {
  const [problemId, setProblemId] = useState('')
  const [solutionData, setSolutionData] = useState('')
  const [fee, setFee] = useState('1500')

  const sendMutation = useMutation({
    mutationFn: async () => Promise.reject(new Error('Use the Marketplace tab to submit solutions')),
    onSuccess,
    onError: (e: Error) => onError(e.message),
  })

  const summary: SummaryRow[] = [
    { label: 'Problem ID', value: problemId.trim() ? `${problemId.slice(0, 12)}…` : '—' },
    { label: 'Fee', value: `${parseInt(fee || '0', 10).toLocaleString()} tokens` },
  ]

  return (
    <FormContainer onSubmit={() => sendMutation.mutate()} onClose={onClose} isPending={sendMutation.isPending} summary={summary}>
      <FormField label="Problem ID" fieldId="ms-id" required>
        <input id="ms-id" type="text" value={problemId} onChange={(e) => setProblemId(e.target.value)} placeholder="64-character hex" style={{ fontFamily: 'monospace', fontSize: 12 }} autoComplete="off" />
      </FormField>
      <FormField label="Solution Data (JSON)" fieldId="ms-data" required>
        <textarea id="ms-data" value={solutionData} onChange={(e) => setSolutionData(e.target.value)} rows={4} placeholder='{"route": [0, 1, 2, ...]}' style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </FormField>
      <FormField label="Fee" fieldId="ms-fee">
        <input id="ms-fee" type="number" value={fee} onChange={(e) => setFee(e.target.value)} />
      </FormField>
    </FormContainer>
  )
}

// ── FormContainer — handles confirm step + submit / cancel ────────────────────

interface FormContainerProps {
  children: React.ReactNode
  onSubmit: () => void
  onClose: () => void
  isPending: boolean
  summary?: SummaryRow[]
}

function FormContainer({ children, onSubmit, onClose, isPending, summary }: FormContainerProps) {
  const [showConfirm, setShowConfirm] = useState(false)

  const handleInitialSubmit = () => {
    // If we have summary rows, show confirmation step first
    if (summary && summary.length > 0) {
      setShowConfirm(true)
    } else {
      onSubmit()
    }
  }

  const handleConfirm = () => {
    setShowConfirm(false)
    onSubmit()
  }

  return (
    <>
      {children}

      {/* Confirmation overlay */}
      {showConfirm && summary && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-tx-title"
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.6)',
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
              maxWidth: 440,
              width: '90%',
              boxShadow: '0 20px 40px rgba(0,0,0,0.2)',
            }}
          >
            <h4 id="confirm-tx-title" style={{ marginBottom: 16, fontSize: 18, fontWeight: 600 }}>
              Confirm Transaction
            </h4>

            <div style={{ marginBottom: 20 }}>
              {summary.map((row) => (
                <div key={row.label} className="confirm-row">
                  <span className="confirm-row-label">{row.label}</span>
                  <span
                    className="confirm-row-value"
                    style={row.highlight ? { color: '#667eea', fontSize: 16, fontWeight: 700 } : undefined}
                  >
                    {row.value}
                  </span>
                </div>
              ))}
            </div>

            <p style={{ fontSize: 12, color: '#718096', marginBottom: 16 }}>
              This transaction will be signed and broadcast to the network. It cannot be reversed.
            </p>

            <div style={{ display: 'flex', gap: 8 }}>
              <button
                onClick={() => setShowConfirm(false)}
                style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }}
              >
                Back
              </button>
              <button
                onClick={handleConfirm}
                disabled={isPending}
                aria-busy={isPending}
                style={{ flex: 1 }}
                autoFocus
              >
                {isPending ? (
                  <><Loader2 size={14} aria-hidden="true" style={{ animation: 'spin 0.7s linear infinite', marginRight: 6 }} /> Submitting…</>
                ) : (
                  'Confirm & Sign'
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, marginTop: 20 }}>
        <button
          onClick={onClose}
          style={{ flex: 1, background: '#e2e8f0', color: '#4a5568' }}
          disabled={isPending}
        >
          Cancel
        </button>
        <button
          onClick={handleInitialSubmit}
          style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
          disabled={isPending}
          aria-busy={isPending}
        >
          {isPending ? (
            <><Loader2 size={14} aria-hidden="true" style={{ animation: 'spin 0.7s linear infinite' }} /> Submitting…</>
          ) : (
            'Review & Submit'
          )}
        </button>
      </div>
    </>
  )
}

// ── FormField ─────────────────────────────────────────────────────────────────

function FormField({ label, fieldId, required, children }: {
  label: string
  fieldId: string
  required?: boolean
  children: React.ReactNode
}) {
  return (
    <div style={{ marginBottom: 16 }}>
      <label
        htmlFor={fieldId}
        style={{ display: 'block', marginBottom: 8, fontSize: 13, fontWeight: 500 }}
      >
        {label}
        {required && <span aria-hidden="true" style={{ color: '#e53e3e', marginLeft: 2 }}>*</span>}
      </label>
      {children}
    </div>
  )
}

// ── Modal ─────────────────────────────────────────────────────────────────────

function Modal({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="tx-modal-title"
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.6)',
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
          maxWidth: 600,
          width: '90%',
          maxHeight: '90vh',
          overflow: 'auto',
        }}
      >
        {children}
      </div>
    </div>
  )
}
