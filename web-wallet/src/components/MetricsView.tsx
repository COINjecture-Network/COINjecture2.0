import { useQuery } from '@tanstack/react-query'
import { Activity, Database, Circle, TrendingUp, Lock, Unlock, Zap } from 'lucide-react'
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, Cell } from 'recharts'
import { MetricsClient, AllPoolsData, ConsensusState } from '../lib/rpc-client'

const metricsClient = new MetricsClient()

// Pool metadata: τ_n, D_n, and names
const POOL_INFO = [
  { name: 'D1', label: 'Genesis', tau: 0.00, d_n: 1.000, color: '#667eea' },
  { name: 'D2', label: 'Coupling', tau: 0.20, d_n: 0.867, color: '#48bb78' },
  { name: 'D3', label: 'First Harmonic', tau: 0.41, d_n: 0.750, color: '#ed8936' },
  { name: 'D4', label: 'Golden Ratio', tau: 0.68, d_n: 0.618, color: '#f6ad55' },
  { name: 'D5', label: 'Half-scale', tau: 0.98, d_n: 0.500, color: '#9f7aea' },
  { name: 'D6', label: 'Second Golden', tau: 1.36, d_n: 0.382, color: '#fc8181' },
  { name: 'D7', label: 'Quarter-scale', tau: 1.96, d_n: 0.250, color: '#63b3ed' },
  { name: 'D8', label: 'Euler', tau: 2.72, d_n: 0.146, color: '#68d391' }
]

// Format very small numbers in scientific notation, larger numbers with fixed decimals
function formatMagnitude(value: number): string {
  if (value === 0) return '0.0000'
  if (value < 0.0001) {
    return value.toExponential(4)
  }
  return value.toFixed(4)
}

export default function MetricsView() {
  // Fetch all dimensional pool data (locked/unlocked/etc)
  const { data: allPools } = useQuery({
    queryKey: ['allPoolsData'],
    queryFn: () => metricsClient.getAllPoolsData(),
    refetchInterval: 5000
  })

  // Fetch consensus state (τ, |ψ|, θ)
  const { data: consensus } = useQuery({
    queryKey: ['consensusState'],
    queryFn: () => metricsClient.getConsensusState(),
    refetchInterval: 5000
  })

  // Fetch Satoshi constants
  const { data: constants } = useQuery({
    queryKey: ['satoshiConstants'],
    queryFn: () => metricsClient.getSatoshiConstants(),
    refetchInterval: 10000
  })

  const { data: blockHeight } = useQuery({
    queryKey: ['metricsBlockHeight'],
    queryFn: () => metricsClient.getBlockHeight(),
    refetchInterval: 10000
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
      {/* Consensus State Dashboard */}
      <div className="card" style={{ background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)', color: 'white', padding: 24 }}>
        <h3 style={{ margin: 0, marginBottom: 16, fontSize: 18 }}>Live Consensus State</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16 }}>
          <ConsensusMetric
            label="τ (Tau)"
            value={consensus?.tau.toFixed(4) ?? '...'}
            description="Dimensionless time = height / τ_c"
          />
          <ConsensusMetric
            label="|ψ(τ)|"
            value={consensus ? formatMagnitude(consensus.magnitude) : '...'}
            description="Wavefunction magnitude = e^(-ητ)"
          />
          <ConsensusMetric
            label="θ(τ)"
            value={consensus ? `${consensus.phase.toFixed(4)} rad` : '...'}
            description="Phase angle = λτ"
          />
          <ConsensusMetric
            label="Block Height"
            value={blockHeight?.toLocaleString() ?? '...'}
            description="Current blockchain height"
          />
        </div>
      </div>

      {/* Satoshi Constants */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16 }}>
        <MetricCard
          icon={<Target size={20} />}
          label="η (Eta)"
          value={constants?.eta.toFixed(6) ?? '...'}
          subtitle={`Expected: ${(1 / Math.sqrt(2)).toFixed(6)}`}
          color="#48bb78"
        />
        <MetricCard
          icon={<Target size={20} />}
          label="λ (Lambda)"
          value={constants?.lambda.toFixed(6) ?? '...'}
          subtitle={`Expected: ${(1 / Math.sqrt(2)).toFixed(6)}`}
          color="#ed8936"
        />
        <MetricCard
          icon={<Circle size={20} />}
          label="|μ|²"
          value={constants?.unit_circle_constraint.toFixed(6) ?? '...'}
          subtitle="Expected: 1.0"
          color={constants && Math.abs(constants.unit_circle_constraint - 1.0) < 0.0001 ? '#48bb78' : '#f56565'}
        />
        <MetricCard
          icon={<Activity size={20} />}
          label="ζ (Damping)"
          value={constants?.damping_coefficient.toFixed(6) ?? '...'}
          subtitle="Critical damping = 1.0"
          color="#667eea"
        />
      </div>

      {/* All 8 Dimensional Pools */}
      <div className="card">
        <h3 className="card-header">All 8 Dimensional Pools - Locked/Unlocked Breakdown</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(350px, 1fr))', gap: 16, padding: 16 }}>
          {POOL_INFO.map(pool => (
            <PoolCard
              key={pool.name}
              poolName={pool.name}
              label={pool.label}
              tau={pool.tau}
              d_n={pool.d_n}
              color={pool.color}
              data={allPools?.[pool.name]}
            />
          ))}
        </div>
      </div>

      {/* Locked vs Unlocked Visualization */}
      <div className="card">
        <h3 className="card-header">Locked vs Unlocked Liquidity (All Pools)</h3>
        <LockedUnlockedChart allPools={allPools} />
      </div>

      {/* Unlock Progress Overview */}
      <div className="card">
        <h3 className="card-header">Unlock Progress - U_n(τ) = 1 - e^(-η(τ - τ_n))</h3>
        <div style={{ padding: 16 }}>
          {POOL_INFO.map(pool => (
            <UnlockProgressBar
              key={pool.name}
              poolName={pool.name}
              label={pool.label}
              color={pool.color}
              unlockFraction={allPools?.[pool.name]?.unlockFraction ?? 0}
            />
          ))}
        </div>
      </div>

      {/* Yield Rates Overview */}
      <div className="card">
        <h3 className="card-header">Yield Rates - r_n(τ) = η · e^(-ητ_n)</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))', gap: 16, padding: 16 }}>
          {POOL_INFO.map(pool => (
            <YieldRateCard
              key={pool.name}
              poolName={pool.name}
              color={pool.color}
              yieldRate={allPools?.[pool.name]?.yieldRate ?? 0}
            />
          ))}
        </div>
      </div>

      {/* System Info */}
      <div className="card" style={{ background: '#f7fafc', borderLeft: '4px solid #667eea' }}>
        <div style={{ fontSize: 14, color: '#4a5568', padding: 16 }}>
          <strong>WEB4 Dimensional Economics - Full Runtime Integration</strong>
          <p style={{ marginTop: 8, lineHeight: 1.6 }}>
            The COINjecture Network operates with 8 dimensional pools (D1-D8) governed by Satoshi constants η = λ = 1/√2.
            Tokens start locked and progressively unlock based on U_n(τ) = 1 - e^(-η(τ - τ_n)).
            Unlocked tokens generate yield at rate r_n(τ) = η · e^(-ητ_n), compounding back into pools.
            The consensus state evolves as τ increases, with wavefunction magnitude |ψ(τ)| = e^(-ητ) decaying exponentially.
          </p>
        </div>
      </div>
    </div>
  )
}

// Components

interface ConsensusMetricProps {
  label: string
  value: string
  description: string
}

function ConsensusMetric({ label, value, description }: ConsensusMetricProps) {
  return (
    <div>
      <div style={{ fontSize: 12, opacity: 0.9, marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 24, fontWeight: 700, marginBottom: 4 }}>{value}</div>
      <div style={{ fontSize: 11, opacity: 0.8 }}>{description}</div>
    </div>
  )
}

interface MetricCardProps {
  icon: React.ReactNode
  label: string
  value: string
  subtitle?: string
  color: string
}

function MetricCard({ icon, label, value, subtitle, color }: MetricCardProps) {
  return (
    <div className="card" style={{ padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8, color }}>
        {icon}
        <span style={{ fontSize: 13, color: '#718096', fontWeight: 600 }}>{label}</span>
      </div>
      <div style={{ fontSize: 18, fontWeight: 700, color: '#1a202c', wordBreak: 'break-all' }}>
        {value}
      </div>
      {subtitle && (
        <div style={{ fontSize: 10, color: '#a0aec0', marginTop: 4 }}>
          {subtitle}
        </div>
      )}
    </div>
  )
}

interface PoolCardProps {
  poolName: string
  label: string
  tau: number
  d_n: number
  color: string
  data?: {
    total: number
    locked: number
    unlocked: number
    unlockFraction: number
    yieldRate: number
  }
}

function PoolCard({ poolName, label, tau, d_n, color, data }: PoolCardProps) {
  if (!data) {
    return (
      <div className="card" style={{ padding: 16, borderLeft: `4px solid ${color}` }}>
        <div style={{ color: '#a0aec0' }}>Loading {poolName}...</div>
      </div>
    )
  }

  const unlockPercent = (data.unlockFraction * 100).toFixed(1)

  return (
    <div className="card" style={{ padding: 16, borderLeft: `4px solid ${color}` }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <div>
          <div style={{ fontSize: 16, fontWeight: 700, color }}>{poolName} - {label}</div>
          <div style={{ fontSize: 11, color: '#a0aec0' }}>τ_{poolName.substring(1)} = {tau.toFixed(2)}, D_{poolName.substring(1)} = {d_n.toFixed(3)}</div>
        </div>
        <Database size={24} style={{ color, opacity: 0.5 }} />
      </div>

      <div style={{ fontSize: 20, fontWeight: 700, color: '#1a202c', marginBottom: 8 }}>
        {data.total.toLocaleString()} <span style={{ fontSize: 12, color: '#a0aec0' }}>tokens</span>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginBottom: 12 }}>
        <div style={{ background: '#fef5e7', padding: 8, borderRadius: 6 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
            <Lock size={12} color="#f59e0b" />
            <span style={{ fontSize: 10, color: '#92400e' }}>Locked</span>
          </div>
          <div style={{ fontSize: 14, fontWeight: 600, color: '#92400e' }}>
            {data.locked.toLocaleString()}
          </div>
        </div>
        <div style={{ background: '#d1fae5', padding: 8, borderRadius: 6 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
            <Unlock size={12} color="#10b981" />
            <span style={{ fontSize: 10, color: '#065f46' }}>Unlocked</span>
          </div>
          <div style={{ fontSize: 14, fontWeight: 600, color: '#065f46' }}>
            {data.unlocked.toLocaleString()}
          </div>
        </div>
      </div>

      <div style={{ marginBottom: 8 }}>
        <div style={{ fontSize: 10, color: '#a0aec0', marginBottom: 4 }}>
          Unlock Progress: {unlockPercent}%
        </div>
        <div style={{ height: 6, background: '#e2e8f0', borderRadius: 3, overflow: 'hidden' }}>
          <div style={{ height: '100%', width: `${unlockPercent}%`, background: color, transition: 'width 0.5s' }} />
        </div>
      </div>

      <div style={{ fontSize: 10, color: '#a0aec0' }}>
        <Zap size={10} style={{ display: 'inline', marginRight: 4 }} />
        Yield Rate: {data.yieldRate.toFixed(6)}
      </div>
    </div>
  )
}

interface LockedUnlockedChartProps {
  allPools: AllPoolsData | undefined
}

function LockedUnlockedChart({ allPools }: LockedUnlockedChartProps) {
  if (!allPools) {
    return (
      <div style={{ height: 300, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#a0aec0' }}>
        Loading metrics...
      </div>
    )
  }

  const data = POOL_INFO.map(pool => ({
    name: pool.name,
    locked: allPools[pool.name]?.locked ?? 0,
    unlocked: allPools[pool.name]?.unlocked ?? 0
  }))

  return (
    <ResponsiveContainer width="100%" height={350}>
      <BarChart data={data}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="name" />
        <YAxis />
        <Tooltip formatter={(value: number) => value.toLocaleString()} />
        <Legend />
        <Bar dataKey="locked" stackId="a" fill="#f59e0b" name="Locked" />
        <Bar dataKey="unlocked" stackId="a" fill="#10b981" name="Unlocked" />
      </BarChart>
    </ResponsiveContainer>
  )
}

interface UnlockProgressBarProps {
  poolName: string
  label: string
  color: string
  unlockFraction: number
}

function UnlockProgressBar({ poolName, label, color, unlockFraction }: UnlockProgressBarProps) {
  const percent = (unlockFraction * 100).toFixed(1)

  return (
    <div style={{ marginBottom: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
        <span style={{ fontSize: 13, fontWeight: 600, color: '#4a5568' }}>
          {poolName} - {label}
        </span>
        <span style={{ fontSize: 12, fontWeight: 600, color }}>{percent}%</span>
      </div>
      <div style={{ height: 12, background: '#e2e8f0', borderRadius: 6, overflow: 'hidden' }}>
        <div
          style={{
            height: '100%',
            width: `${percent}%`,
            background: color,
            transition: 'width 0.5s ease',
            position: 'relative'
          }}
        >
          {unlockFraction > 0.05 && (
            <div style={{
              position: 'absolute',
              right: 8,
              top: '50%',
              transform: 'translateY(-50%)',
              fontSize: 9,
              color: 'white',
              fontWeight: 600
            }}>
              {percent}%
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

interface YieldRateCardProps {
  poolName: string
  color: string
  yieldRate: number
}

function YieldRateCard({ poolName, color, yieldRate }: YieldRateCardProps) {
  return (
    <div className="card" style={{ padding: 12, textAlign: 'center', borderTop: `3px solid ${color}` }}>
      <div style={{ fontSize: 12, color: '#a0aec0', marginBottom: 6 }}>{poolName}</div>
      <div style={{ fontSize: 16, fontWeight: 700, color }}>
        {yieldRate.toFixed(6)}
      </div>
    </div>
  )
}

function Target({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <circle cx="12" cy="12" r="10" />
      <circle cx="12" cy="12" r="6" />
      <circle cx="12" cy="12" r="2" />
    </svg>
  )
}
