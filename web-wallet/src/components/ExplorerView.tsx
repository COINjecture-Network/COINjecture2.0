import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Blocks, Search, ChevronLeft, ChevronRight, Network, Clock, Award, ChevronDown, ChevronUp } from 'lucide-react'
import { RpcClient, MetricsClient, Block, ChainInfo } from '../lib/rpc-client'

const rpcClient = new RpcClient()
const metricsClient = new MetricsClient()

export default function ExplorerView() {
  const [selectedBlock, setSelectedBlock] = useState<number | null>(null)
  const [searchHeight, setSearchHeight] = useState('')
  const [showPeerInfo, setShowPeerInfo] = useState(false)

  // Get chain info
  const { data: chainInfo } = useQuery({
    queryKey: ['chainInfo'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000
  })

  // Get dimensional pool balances
  const { data: poolBalances } = useQuery({
    queryKey: ['poolBalances'],
    queryFn: () => metricsClient.getPoolBalances(),
    refetchInterval: 5000
  })

  // Get Satoshi constants
  const { data: satoshiConstants } = useQuery({
    queryKey: ['satoshiConstants'],
    queryFn: () => metricsClient.getSatoshiConstants(),
    refetchInterval: 10000
  })

  const handleSearch = () => {
    const height = parseInt(searchHeight)
    if (!isNaN(height) && height >= 0) {
      setSelectedBlock(height)
    }
  }

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20 }}>
      {/* Left Column: Chain Overview & Search */}
      <div>
        <ChainOverview
          chainInfo={chainInfo}
          showPeerInfo={showPeerInfo}
          onTogglePeerInfo={() => setShowPeerInfo(!showPeerInfo)}
        />

        <div className="card" style={{ marginTop: 20 }}>
          <h3 className="card-header">Block Search</h3>
          <div style={{ display: 'flex', gap: 8 }}>
            <input
              type="number"
              value={searchHeight}
              onChange={(e) => setSearchHeight(e.target.value)}
              placeholder="Enter block height..."
              min="0"
              onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
            />
            <button onClick={handleSearch} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <Search size={16} /> Search
            </button>
          </div>
        </div>

        {/* Dimensional Economics */}
        {poolBalances && satoshiConstants && (
          <div className="card" style={{ marginTop: 20 }}>
            <h3 className="card-header">💎 Dimensional Economics</h3>

            <div style={{ marginBottom: 16, padding: 12, background: '#f0f4ff', borderRadius: 6 }}>
              <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8, color: '#667eea' }}>Satoshi Constants</div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12, fontSize: 12 }}>
                <div>
                  <div style={{ color: '#718096' }}>η (eta)</div>
                  <div style={{ fontWeight: 600, fontFamily: 'monospace' }}>{satoshiConstants.eta.toFixed(6)}</div>
                </div>
                <div>
                  <div style={{ color: '#718096' }}>λ (lambda)</div>
                  <div style={{ fontWeight: 600, fontFamily: 'monospace' }}>{satoshiConstants.lambda.toFixed(6)}</div>
                </div>
                <div>
                  <div style={{ color: '#718096' }}>η² + λ²</div>
                  <div style={{ fontWeight: 600, fontFamily: 'monospace' }}>{satoshiConstants.unit_circle_constraint.toFixed(6)}</div>
                </div>
              </div>
            </div>

            <div style={{ display: 'grid', gap: 12 }}>
              <div style={{ padding: 12, background: '#f0fff4', border: '2px solid #48bb78', borderRadius: 6 }}>
                <div style={{ fontSize: 11, color: '#718096', marginBottom: 4 }}>D₁ GENESIS (τ=0.00, D=1.000)</div>
                <div style={{ fontSize: 20, fontWeight: 700, color: '#48bb78' }}>
                  {poolBalances.d1.toLocaleString()} tokens
                </div>
              </div>
              <div style={{ padding: 12, background: '#f0f9ff', border: '2px solid #4299e1', borderRadius: 6 }}>
                <div style={{ fontSize: 11, color: '#718096', marginBottom: 4 }}>D₂ COUPLING (τ=0.20, D=0.867)</div>
                <div style={{ fontSize: 20, fontWeight: 700, color: '#4299e1' }}>
                  {poolBalances.d2.toLocaleString()} tokens
                </div>
              </div>
              <div style={{ padding: 12, background: '#faf5ff', border: '2px solid #9f7aea', borderRadius: 6 }}>
                <div style={{ fontSize: 11, color: '#718096', marginBottom: 4 }}>D₃ HARMONIC (τ=0.41, D=0.750)</div>
                <div style={{ fontSize: 20, fontWeight: 700, color: '#9f7aea' }}>
                  {poolBalances.d3.toLocaleString()} tokens
                </div>
              </div>
            </div>

            <div style={{ marginTop: 12, padding: 10, background: '#f7fafc', borderRadius: 6, fontSize: 11, color: '#4a5568' }}>
              💡 Formula: D<sub>n</sub> = e<sup>-η·τₙ</sup> where η = λ = 1/√2 ≈ 0.707107
            </div>
          </div>
        )}

        <LatestBlocks
          currentHeight={chainInfo?.best_height ?? 0}
          selectedBlock={selectedBlock}
          onSelectBlock={setSelectedBlock}
        />
      </div>

      {/* Right Column: Block Details */}
      <div>
        {selectedBlock !== null ? (
          <BlockDetails
            height={selectedBlock}
            onNavigate={setSelectedBlock}
            maxHeight={chainInfo?.best_height ?? 0}
          />
        ) : (
          <div className="card" style={{ textAlign: 'center', padding: 60, color: '#718096' }}>
            <Blocks size={48} style={{ margin: '0 auto 16px', opacity: 0.5 }} />
            <p>Select a block to view details</p>
          </div>
        )}
      </div>
    </div>
  )
}

interface ChainOverviewProps {
  chainInfo: ChainInfo | undefined
  showPeerInfo: boolean
  onTogglePeerInfo: () => void
}

function ChainOverview({ chainInfo, showPeerInfo, onTogglePeerInfo }: ChainOverviewProps) {
  return (
    <div className="card">
      <h3 className="card-header">Chain Overview</h3>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <StatCard
          icon={<Blocks size={20} />}
          label="Best Height"
          value={chainInfo?.best_height.toLocaleString() ?? '...'}
          color="#667eea"
        />
        <div
          onClick={onTogglePeerInfo}
          style={{ cursor: 'pointer', position: 'relative' }}
        >
          <StatCard
            icon={<Network size={20} />}
            label="Connected Peers"
            value={chainInfo?.peer_count.toString() ?? '...'}
            color="#48bb78"
          />
          <div style={{ position: 'absolute', top: 16, right: 16, color: '#48bb78' }}>
            {showPeerInfo ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
          </div>
        </div>
      </div>

      {/* Expandable Peer Info Panel */}
      {showPeerInfo && (
        <div style={{
          marginTop: 16,
          padding: 16,
          background: '#f0fff4',
          border: '2px solid #48bb78',
          borderRadius: 8,
          animation: 'slideDown 0.2s ease-out'
        }}>
          <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 12, color: '#48bb78', display: 'flex', alignItems: 'center', gap: 8 }}>
            <Network size={18} />
            Peer Network Information
          </div>

          {chainInfo && chainInfo.peer_count > 0 ? (
            <div>
              <div style={{ fontSize: 13, color: '#4a5568', marginBottom: 8 }}>
                Connected to <span style={{ fontWeight: 600, color: '#48bb78' }}>{chainInfo.peer_count}</span> peer{chainInfo.peer_count !== 1 ? 's' : ''}
              </div>
              <div style={{ padding: 12, background: 'white', borderRadius: 6, fontSize: 12 }}>
                <div style={{ color: '#718096', marginBottom: 4 }}>Bootnode Connection:</div>
                <code style={{ fontSize: 10, wordBreak: 'break-all', color: '#4a5568' }}>
                  /ip4/143.110.139.166/tcp/30333/p2p/12D3KooWMhjsLxLD7p8RkxEJ77uhFfkuoTQB2JtoqCMkznw83cz9
                </code>
              </div>
              <div style={{ marginTop: 12, padding: 10, background: 'white', borderRadius: 6, fontSize: 11, color: '#4a5568' }}>
                💡 This node is connected to the COINjecture Network B testnet. All peers use LibP2P for secure, decentralized communication.
              </div>
            </div>
          ) : (
            <div style={{ textAlign: 'center', padding: 20, color: '#718096' }}>
              <div style={{ fontSize: 32, marginBottom: 8 }}>🔍</div>
              <div style={{ fontSize: 13 }}>Not connected to any peers</div>
              <div style={{ fontSize: 11, marginTop: 4 }}>Check your network connection or bootnode configuration</div>
            </div>
          )}
        </div>
      )}

      <div style={{ marginTop: 16, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Best Block Hash</div>
        <code style={{ fontSize: 11, wordBreak: 'break-all' }}>
          {chainInfo?.best_hash ?? 'Loading...'}
        </code>
      </div>

      <div style={{ marginTop: 12, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Genesis Hash</div>
        <code style={{ fontSize: 11, wordBreak: 'break-all' }}>
          {chainInfo?.genesis_hash ?? 'Loading...'}
        </code>
      </div>

      <div style={{ marginTop: 12, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Chain ID</div>
        <code style={{ fontSize: 11 }}>{chainInfo?.chain_id ?? 'Loading...'}</code>
      </div>
    </div>
  )
}

interface StatCardProps {
  icon: React.ReactNode
  label: string
  value: string
  color: string
}

function StatCard({ icon, label, value, color }: StatCardProps) {
  return (
    <div style={{ padding: 16, background: '#f7fafc', borderRadius: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8, color }}>
        {icon}
        <span style={{ fontSize: 12, color: '#718096' }}>{label}</span>
      </div>
      <div style={{ fontSize: 24, fontWeight: 700, color }}>{value}</div>
    </div>
  )
}

interface LatestBlocksProps {
  currentHeight: number
  selectedBlock: number | null
  onSelectBlock: (height: number) => void
}

function LatestBlocks({ currentHeight, selectedBlock, onSelectBlock }: LatestBlocksProps) {
  const blocks = Array.from({ length: Math.min(10, currentHeight + 1) }, (_, i) => currentHeight - i)

  return (
    <div className="card" style={{ marginTop: 20 }}>
      <h3 className="card-header">Latest Blocks</h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {blocks.map(height => (
          <BlockListItem
            key={height}
            height={height}
            selected={selectedBlock === height}
            onClick={() => onSelectBlock(height)}
          />
        ))}
      </div>
    </div>
  )
}

interface BlockListItemProps {
  height: number
  selected: boolean
  onClick: () => void
}

function BlockListItem({ height, selected, onClick }: BlockListItemProps) {
  const { data: block } = useQuery({
    queryKey: ['block', height],
    queryFn: () => rpcClient.getBlock(height)
  })

  return (
    <div
      onClick={onClick}
      style={{
        padding: 12,
        border: selected ? '2px solid #667eea' : '1px solid #e2e8f0',
        borderRadius: 6,
        cursor: 'pointer',
        background: selected ? '#f7fafc' : 'white',
        transition: 'all 0.2s'
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <div style={{ fontWeight: 600 }}>Block #{height}</div>
          <div style={{ fontSize: 12, color: '#718096', marginTop: 2 }}>
            {block ? `${block.transactions.length} tx • Score: ${block.header.work_score.toFixed(2)}` : 'Loading...'}
          </div>
        </div>
        {block && (
          <div style={{ fontSize: 12, color: '#718096' }}>
            {new Date(block.header.timestamp * 1000).toLocaleTimeString()}
          </div>
        )}
      </div>
    </div>
  )
}

interface BlockDetailsProps {
  height: number
  onNavigate: (height: number) => void
  maxHeight: number
}

function BlockDetails({ height, onNavigate, maxHeight }: BlockDetailsProps) {
  const { data: block, isLoading, error } = useQuery({
    queryKey: ['block', height],
    queryFn: () => rpcClient.getBlock(height)
  })

  if (isLoading) {
    return (
      <div className="card" style={{ textAlign: 'center', padding: 60 }}>
        <div style={{ fontSize: 48, marginBottom: 16 }}>⏳</div>
        <p style={{ color: '#718096' }}>Loading block...</p>
      </div>
    )
  }

  if (error || !block) {
    return (
      <div className="card" style={{ textAlign: 'center', padding: 60 }}>
        <div style={{ fontSize: 48, marginBottom: 16 }}>❌</div>
        <p style={{ color: '#c53030' }}>Block not found</p>
      </div>
    )
  }

  return (
    <div className="card">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h3 className="card-header" style={{ marginBottom: 0 }}>Block #{height}</h3>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            onClick={() => onNavigate(height - 1)}
            disabled={height === 0}
            style={{ padding: 6 }}
          >
            <ChevronLeft size={16} />
          </button>
          <button
            onClick={() => onNavigate(height + 1)}
            disabled={height >= maxHeight}
            style={{ padding: 6 }}
          >
            <ChevronRight size={16} />
          </button>
        </div>
      </div>

      <div style={{ display: 'grid', gap: 12 }}>
        <DetailRow icon={<Clock size={16} />} label="Timestamp">
          {new Date(block.header.timestamp * 1000).toLocaleString()}
        </DetailRow>

        <DetailRow icon={<Award size={16} />} label="Work Score">
          <span style={{ fontWeight: 600, color: '#667eea' }}>{block.header.work_score.toFixed(4)}</span>
        </DetailRow>

        <DetailRow icon={<Award size={16} />} label="Coinbase Reward">
          <span style={{ fontWeight: 600, color: '#48bb78' }}>{block.coinbase.reward.toLocaleString()} tokens</span>
        </DetailRow>

        <DetailRow label="Nonce">
          <code>{block.header.nonce}</code>
        </DetailRow>

        <DetailRow label="Version">
          {block.header.version}
        </DetailRow>
      </div>

      {/* PoUW Transparency Metrics */}
      <div style={{ marginTop: 16, padding: 16, background: '#f0f4ff', borderRadius: 8, borderLeft: '4px solid #667eea' }}>
        <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 12, color: '#667eea' }}>
          🔬 Proof of Useful Work (PoUW) Transparency
        </div>
        <div style={{ display: 'grid', gap: 8, fontSize: 13 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Solve Time:</span>
            <span style={{ fontWeight: 600 }}>{block.header.solve_time_ms.toLocaleString()} ms</span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Verify Time:</span>
            <span style={{ fontWeight: 600 }}>{block.header.verify_time_ms} ms</span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Time Asymmetry:</span>
            <span style={{ fontWeight: 600, color: '#48bb78' }}>{block.header.time_asymmetry_ratio.toFixed(2)}x</span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Solution Quality:</span>
            <span style={{ fontWeight: 600, color: '#667eea' }}>{(block.header.solution_quality * 100).toFixed(2)}%</span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Complexity Weight:</span>
            <span style={{ fontWeight: 600 }}>{block.header.complexity_weight.toFixed(2)}</span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ color: '#718096' }}>Energy Estimate:</span>
            <span style={{ fontWeight: 600, color: '#ed8936' }}>{block.header.energy_estimate_joules.toFixed(2)} J</span>
          </div>
        </div>
        <div style={{ marginTop: 12, padding: 8, background: 'white', borderRadius: 4, fontSize: 11, color: '#4a5568' }}>
          💡 Time asymmetry proves useful computational work was performed. High ratio = hard to solve, easy to verify (NP-complete).
        </div>
      </div>

      <div style={{ marginTop: 16, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Block Hash</div>
        <code style={{ fontSize: 11, wordBreak: 'break-all' }}>
          {/* Note: Block hash would need to be computed or returned from RPC */}
          Computing...
        </code>
      </div>

      <div style={{ marginTop: 12, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Previous Hash</div>
        <code style={{ fontSize: 11, wordBreak: 'break-all' }}>
          {block.header.prev_hash}
        </code>
      </div>

      <div style={{ marginTop: 12, padding: 12, background: '#f7fafc', borderRadius: 6 }}>
        <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Miner Address</div>
        <code style={{ fontSize: 11, wordBreak: 'break-all' }}>
          {block.header.miner}
        </code>
      </div>

      <div style={{ marginTop: 16 }}>
        <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 8 }}>
          Transactions ({block.transactions.length})
        </div>
        {block.transactions.length === 0 ? (
          <div style={{ padding: 20, textAlign: 'center', color: '#718096', background: '#f7fafc', borderRadius: 6 }}>
            No transactions in this block
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {block.transactions.map((tx, i) => (
              <div key={i} style={{ padding: 12, background: '#f7fafc', borderRadius: 6 }}>
                <div style={{ fontSize: 12, color: '#718096', marginBottom: 4 }}>Transaction #{i + 1}</div>
                <div style={{ display: 'grid', gap: 4, fontSize: 12 }}>
                  <div>
                    <span style={{ color: '#718096' }}>From:</span>{' '}
                    <code style={{ fontSize: 10 }}>{tx.from.slice(0, 16)}...{tx.from.slice(-8)}</code>
                  </div>
                  <div>
                    <span style={{ color: '#718096' }}>To:</span>{' '}
                    <code style={{ fontSize: 10 }}>{tx.to.slice(0, 16)}...{tx.to.slice(-8)}</code>
                  </div>
                  <div>
                    <span style={{ color: '#718096' }}>Amount:</span>{' '}
                    <span style={{ fontWeight: 600 }}>{tx.amount.toLocaleString()} tokens</span>
                  </div>
                  <div>
                    <span style={{ color: '#718096' }}>Fee:</span> {tx.fee} tokens
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

interface DetailRowProps {
  icon?: React.ReactNode
  label: string
  children: React.ReactNode
}

function DetailRow({ icon, label, children }: DetailRowProps) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '8px 0', borderBottom: '1px solid #e2e8f0' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: '#718096', fontSize: 14 }}>
        {icon}
        <span>{label}</span>
      </div>
      <div style={{ fontSize: 14 }}>{children}</div>
    </div>
  )
}
