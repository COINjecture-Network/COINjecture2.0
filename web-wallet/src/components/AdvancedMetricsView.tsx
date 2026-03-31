import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Activity, Lock, TrendingUp, Calculator, Waves, Triangle, Target, CheckCircle } from 'lucide-react'
import { BarChart, Bar, LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, ScatterChart, Scatter, ReferenceLine } from 'recharts'
import { MetricsClient } from '../lib/rpc-client'

const metricsClient = new MetricsClient()

// Pool metadata
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

const ETA = 1 / Math.sqrt(2)
const LAMBDA = 1 / Math.sqrt(2)
const TAU_C = Math.sqrt(2) // τ_c = √2

export default function AdvancedMetricsView() {
  const [selectedTab, setSelectedTab] = useState<'overview' | 'timeseries' | 'staking' | 'calculator' | 'wavefunction' | 'oracle'>('overview')

  const { data: allPools } = useQuery({
    queryKey: ['allPoolsData'],
    queryFn: () => metricsClient.getAllPoolsData(),
    refetchInterval: 5000
  })

  const { data: consensus } = useQuery({
    queryKey: ['consensusState'],
    queryFn: () => metricsClient.getConsensusState(),
    refetchInterval: 5000
  })

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

  const { data: convergence } = useQuery({
    queryKey: ['convergenceMetrics'],
    queryFn: () => metricsClient.getConvergenceMetrics(),
    refetchInterval: 5000
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
      {/* Tab Navigation */}
      <div style={{ display: 'flex', gap: 8, borderBottom: '2px solid #e2e8f0', paddingBottom: 8 }}>
        <TabButton active={selectedTab === 'overview'} onClick={() => setSelectedTab('overview')}>
          <Activity size={16} /> Overview
        </TabButton>
        <TabButton active={selectedTab === 'timeseries'} onClick={() => setSelectedTab('timeseries')}>
          <TrendingUp size={16} /> Time Series
        </TabButton>
        <TabButton active={selectedTab === 'staking'} onClick={() => setSelectedTab('staking')}>
          <Lock size={16} /> Staking
        </TabButton>
        <TabButton active={selectedTab === 'calculator'} onClick={() => setSelectedTab('calculator')}>
          <Calculator size={16} /> Calculator
        </TabButton>
        <TabButton active={selectedTab === 'wavefunction'} onClick={() => setSelectedTab('wavefunction')}>
          <Waves size={16} /> Wavefunction
        </TabButton>
        <TabButton active={selectedTab === 'oracle'} onClick={() => setSelectedTab('oracle')}>
          <Triangle size={16} /> Oracle
        </TabButton>
      </div>

      {/* Tab Content */}
      {selectedTab === 'overview' && (
        <OverviewTab
          allPools={allPools}
          consensus={consensus}
          blockHeight={blockHeight}
        />
      )}
      {selectedTab === 'timeseries' && (
        <TimeSeriesTab consensus={consensus} blockHeight={blockHeight} />
      )}
      {selectedTab === 'staking' && (
        <StakingTab allPools={allPools} />
      )}
      {selectedTab === 'calculator' && (
        <CalculatorTab consensus={consensus} blockHeight={blockHeight} />
      )}
      {selectedTab === 'wavefunction' && (
        <WavefunctionTab consensus={consensus} />
      )}
      {selectedTab === 'oracle' && (
        <OracleTab constants={constants} convergence={convergence} />
      )}
    </div>
  )
}

// Tab Components

function OverviewTab({ allPools, consensus, blockHeight }: any) {
  return (
    <>
      {/* Consensus State Dashboard */}
      <div className="card" style={{ background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)', color: 'white', padding: 24 }}>
        <h3 style={{ margin: 0, marginBottom: 16, fontSize: 18 }}>Live Consensus State</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16 }}>
          <ConsensusMetric label="τ (Tau)" value={consensus?.tau.toFixed(4) ?? '...'} description="Dimensionless time" />
          <ConsensusMetric label="|ψ(τ)|" value={consensus?.magnitude.toFixed(4) ?? '...'} description="Wavefunction magnitude" />
          <ConsensusMetric label="θ(τ)" value={consensus ? `${consensus.phase.toFixed(4)} rad` : '...'} description="Phase angle" />
          <ConsensusMetric label="Block Height" value={blockHeight?.toLocaleString() ?? '...'} description="Current height" />
        </div>
      </div>

      {/* All 8 Pools Grid */}
      <div className="card">
        <h3 className="card-header">All 8 Dimensional Pools</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: 16, padding: 16 }}>
          {POOL_INFO.map(pool => (
            <PoolCard key={pool.name} {...pool} data={allPools?.[pool.name]} />
          ))}
        </div>
      </div>

      {/* Locked vs Unlocked Chart */}
      <div className="card">
        <h3 className="card-header">Locked vs Unlocked Liquidity</h3>
        <LockedUnlockedChart allPools={allPools} />
      </div>
    </>
  )
}

function TimeSeriesTab({ consensus, blockHeight }: any) {
  // Generate simulated historical data based on current state
  const generateHistoricalData = () => {
    if (!consensus || !blockHeight) return []

    const data = []
    const currentTau = consensus.tau
    const points = 50

    for (let i = 0; i < points; i++) {
      const tau = (currentTau / points) * i
      const magnitude = Math.exp(-ETA * tau)
      const phase = LAMBDA * tau

      // Simulate pool balances (they would grow over time with yields)
      const d1 = 1000000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.00)
      const d2 = 1000000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.20)
      const d3 = 1000000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.41)

      data.push({
        tau: parseFloat(tau.toFixed(4)),
        magnitude: parseFloat(magnitude.toFixed(4)),
        phase: parseFloat(phase.toFixed(4)),
        d1: Math.round(d1),
        d2: Math.round(d2),
        d3: Math.round(d3),
        totalSupply: Math.round(d1 + d2 + d3)
      })
    }

    return data
  }

  const historicalData = generateHistoricalData()

  return (
    <>
      <div className="card">
        <h3 className="card-header">Pool Balance Evolution Over τ</h3>
        <ResponsiveContainer width="100%" height={400}>
          <LineChart data={historicalData}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="tau" label={{ value: 'τ (Dimensionless Time)', position: 'insideBottom', offset: -5 }} />
            <YAxis label={{ value: 'Balance (tokens)', angle: -90, position: 'insideLeft' }} />
            <Tooltip />
            <Legend />
            <Line type="monotone" dataKey="d1" stroke="#667eea" name="D1 Balance" strokeWidth={2} />
            <Line type="monotone" dataKey="d2" stroke="#48bb78" name="D2 Balance" strokeWidth={2} />
            <Line type="monotone" dataKey="d3" stroke="#ed8936" name="D3 Balance" strokeWidth={2} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="card">
        <h3 className="card-header">Wavefunction Magnitude Decay</h3>
        <ResponsiveContainer width="100%" height={300}>
          <LineChart data={historicalData}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="tau" label={{ value: 'τ', position: 'insideBottom', offset: -5 }} />
            <YAxis domain={[0, 1]} label={{ value: '|ψ(τ)|', angle: -90, position: 'insideLeft' }} />
            <Tooltip />
            <Line type="monotone" dataKey="magnitude" stroke="#9f7aea" name="|ψ(τ)| = e^(-ητ)" strokeWidth={3} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="card">
        <h3 className="card-header">Total Supply Growth (Yield Compounding)</h3>
        <ResponsiveContainer width="100%" height={300}>
          <LineChart data={historicalData}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="tau" />
            <YAxis />
            <Tooltip formatter={(value: number) => value.toLocaleString()} />
            <Line type="monotone" dataKey="totalSupply" stroke="#f56565" name="Total Supply" strokeWidth={2} />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </>
  )
}

function StakingTab({ allPools }: any) {
  const [selectedPool, setSelectedPool] = useState('D1')
  const [stakeAmount, setStakeAmount] = useState('')

  return (
    <>
      <div className="card">
        <h3 className="card-header">Stake into Dimensional Pools</h3>
        <div style={{ padding: 20 }}>
          <div style={{ marginBottom: 20 }}>
            <label style={{ display: 'block', marginBottom: 8, fontWeight: 600 }}>Select Pool:</label>
            <select
              value={selectedPool}
              onChange={(e) => setSelectedPool(e.target.value)}
              style={{ width: '100%', padding: 12, borderRadius: 6, border: '1px solid #cbd5e0' }}
            >
              {POOL_INFO.map(pool => (
                <option key={pool.name} value={pool.name}>
                  {pool.name} - {pool.label} (τ={pool.tau.toFixed(2)})
                </option>
              ))}
            </select>
          </div>

          <div style={{ marginBottom: 20 }}>
            <label style={{ display: 'block', marginBottom: 8, fontWeight: 600 }}>Amount to Stake:</label>
            <input
              type="number"
              value={stakeAmount}
              onChange={(e) => setStakeAmount(e.target.value)}
              placeholder="Enter amount..."
              style={{ width: '100%', padding: 12, borderRadius: 6, border: '1px solid #cbd5e0' }}
            />
          </div>

          {allPools && allPools[selectedPool] && (
            <div style={{ background: '#f7fafc', padding: 16, borderRadius: 8, marginBottom: 20 }}>
              <div style={{ fontSize: 14, color: '#4a5568' }}>
                <div style={{ marginBottom: 8 }}>
                  <strong>Pool Status:</strong>
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                  <div>Total Liquidity: {allPools[selectedPool].total.toLocaleString()}</div>
                  <div>Unlocked: {allPools[selectedPool].unlocked.toLocaleString()}</div>
                  <div>Unlock Progress: {(allPools[selectedPool].unlockFraction * 100).toFixed(1)}%</div>
                  <div>Yield Rate: {allPools[selectedPool].yieldRate.toFixed(6)}</div>
                </div>
              </div>
            </div>
          )}

          <button
            style={{
              width: '100%',
              padding: 16,
              background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
              color: 'white',
              border: 'none',
              borderRadius: 8,
              fontSize: 16,
              fontWeight: 600,
              cursor: 'pointer'
            }}
          >
            <Lock size={20} style={{ display: 'inline', marginRight: 8 }} />
            Stake Tokens
          </button>

          <div style={{ marginTop: 16, padding: 12, background: '#edf2f7', borderRadius: 6, fontSize: 13, color: '#4a5568' }}>
            <strong>Note:</strong> Staking locks your tokens in the selected pool. They will unlock according to U_n(τ) = 1 - e^(-η(τ - τ_n)) and generate yields at rate r_n(τ) = η · e^(-ητ_n).
          </div>
        </div>
      </div>

      <div className="card">
        <h3 className="card-header">Your Stakes</h3>
        <div style={{ padding: 20, textAlign: 'center', color: '#a0aec0' }}>
          No active stakes yet. Stake tokens above to start earning yields!
        </div>
      </div>
    </>
  )
}

function CalculatorTab({ consensus, blockHeight }: any) {
  const [targetPool, setTargetPool] = useState('D7')
  const [targetUnlock, setTargetUnlock] = useState('90')

  const calculateUnlockTime = () => {
    if (!consensus || !blockHeight) return null

    const poolInfo = POOL_INFO.find(p => p.name === targetPool)
    if (!poolInfo) return null

    const targetFraction = parseFloat(targetUnlock) / 100
    const currentTau = consensus.tau

    // U_n(τ) = 1 - e^(-η(τ - τ_n))
    // Solve for τ when U_n(τ) = target
    // 1 - e^(-η(τ - τ_n)) = target
    // e^(-η(τ - τ_n)) = 1 - target
    // -η(τ - τ_n) = ln(1 - target)
    // τ = τ_n - ln(1 - target) / η

    const requiredTau = poolInfo.tau - Math.log(1 - targetFraction) / ETA
    const deltaTau = requiredTau - currentTau
    const deltaBlocks = Math.round(deltaTau * TAU_C)

    const currentUnlockFraction = 1 - Math.exp(-ETA * Math.max(0, currentTau - poolInfo.tau))

    return {
      requiredTau,
      deltaTau,
      deltaBlocks,
      currentUnlockFraction,
      alreadyUnlocked: currentUnlockFraction >= targetFraction
    }
  }

  const result = calculateUnlockTime()

  return (
    <div className="card">
      <h3 className="card-header">
        <Calculator size={20} style={{ display: 'inline', marginRight: 8 }} />
        Unlock Time Calculator
      </h3>
      <div style={{ padding: 20 }}>
        <div style={{ marginBottom: 20 }}>
          <label style={{ display: 'block', marginBottom: 8, fontWeight: 600 }}>Select Pool:</label>
          <select
            value={targetPool}
            onChange={(e) => setTargetPool(e.target.value)}
            style={{ width: '100%', padding: 12, borderRadius: 6, border: '1px solid #cbd5e0' }}
          >
            {POOL_INFO.map(pool => (
              <option key={pool.name} value={pool.name}>
                {pool.name} - {pool.label} (τ_n={pool.tau.toFixed(2)})
              </option>
            ))}
          </select>
        </div>

        <div style={{ marginBottom: 20 }}>
          <label style={{ display: 'block', marginBottom: 8, fontWeight: 600 }}>Target Unlock %:</label>
          <input
            type="number"
            value={targetUnlock}
            onChange={(e) => setTargetUnlock(e.target.value)}
            min="0"
            max="100"
            style={{ width: '100%', padding: 12, borderRadius: 6, border: '1px solid #cbd5e0' }}
          />
        </div>

        {result && (
          <div style={{ background: result.alreadyUnlocked ? '#d1fae5' : '#edf2f7', padding: 20, borderRadius: 8 }}>
            <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 16, color: result.alreadyUnlocked ? '#065f46' : '#1a202c' }}>
              {result.alreadyUnlocked ? '✓ Already Unlocked!' : 'Unlock Prediction'}
            </div>

            {result.alreadyUnlocked ? (
              <div style={{ fontSize: 14, color: '#065f46' }}>
                Current unlock level: {(result.currentUnlockFraction * 100).toFixed(2)}% ≥ {targetUnlock}%
              </div>
            ) : (
              <div style={{ display: 'grid', gap: 12, fontSize: 14 }}>
                <div>
                  <strong>Current τ:</strong> {consensus.tau.toFixed(4)}
                </div>
                <div>
                  <strong>Current unlock:</strong> {(result.currentUnlockFraction * 100).toFixed(2)}%
                </div>
                <div>
                  <strong>Required τ:</strong> {result.requiredTau.toFixed(4)}
                </div>
                <div>
                  <strong>Δτ remaining:</strong> {result.deltaTau.toFixed(4)}
                </div>
                <div style={{ fontSize: 18, fontWeight: 700, color: '#667eea', marginTop: 8 }}>
                  <strong>Blocks until unlock:</strong> {result.deltaBlocks.toLocaleString()}
                </div>
                <div style={{ fontSize: 12, color: '#718096', marginTop: 8 }}>
                  Assuming ~10 seconds/block: {Math.round(result.deltaBlocks * 10 / 60)} minutes
                </div>
              </div>
            )}
          </div>
        )}

        <div style={{ marginTop: 20, padding: 12, background: '#f7fafc', borderRadius: 6, fontSize: 13, color: '#4a5568' }}>
          <strong>Formula:</strong> U_n(τ) = 1 - e^(-η(τ - τ_n)) where η = 1/√2 ≈ 0.707107
        </div>
      </div>
    </div>
  )
}

function WavefunctionTab({ consensus }: any) {
  // Generate complex plane visualization
  const generateWavefunctionData = () => {
    if (!consensus) return []

    const points = 100
    const data = []
    const currentTau = consensus.tau

    for (let i = 0; i <= points; i++) {
      const tau = (currentTau / points) * i
      const magnitude = Math.exp(-ETA * tau)
      const phase = LAMBDA * tau

      // ψ(τ) = |ψ| · e^(iθ) = |ψ| · (cos(θ) + i·sin(θ))
      const real = magnitude * Math.cos(phase)
      const imag = magnitude * Math.sin(phase)

      data.push({
        tau,
        real,
        imag,
        magnitude
      })
    }

    return data
  }

  const wavefunctionData = generateWavefunctionData()

  return (
    <>
      <div className="card">
        <h3 className="card-header">
          <Waves size={20} style={{ display: 'inline', marginRight: 8 }} />
          Complex Wavefunction: ψ(τ) = |ψ|·e^(iθ)
        </h3>
        <ResponsiveContainer width="100%" height={500}>
          <ScatterChart margin={{ top: 20, right: 20, bottom: 60, left: 60 }}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis
              type="number"
              dataKey="real"
              name="Re(ψ)"
              domain={[-1, 1]}
              label={{ value: 'Real Component Re(ψ)', position: 'insideBottom', offset: -10 }}
            />
            <YAxis
              type="number"
              dataKey="imag"
              name="Im(ψ)"
              domain={[-1, 1]}
              label={{ value: 'Imaginary Component Im(ψ)', angle: -90, position: 'insideLeft' }}
            />
            <Tooltip cursor={{ strokeDasharray: '3 3' }} />
            <ReferenceLine x={0} stroke="#cbd5e0" />
            <ReferenceLine y={0} stroke="#cbd5e0" />
            <Scatter data={wavefunctionData} fill="#9f7aea" line={{ stroke: '#667eea', strokeWidth: 2 }} />
          </ScatterChart>
        </ResponsiveContainer>

        {consensus && (
          <div style={{ padding: 16, background: '#f7fafc', marginTop: 16, borderRadius: 8 }}>
            <div style={{ fontSize: 14, color: '#4a5568' }}>
              <strong>Current State:</strong>
              <div style={{ marginTop: 8, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                <div>τ = {consensus.tau.toFixed(4)}</div>
                <div>|ψ| = {consensus.magnitude.toFixed(4)}</div>
                <div>θ = {consensus.phase.toFixed(4)} rad</div>
                <div>θ = {(consensus.phase * 180 / Math.PI).toFixed(2)}°</div>
              </div>
            </div>
          </div>
        )}
      </div>

      <div style={{ fontSize: 14, color: '#718096', padding: 16, background: '#edf2f7', borderRadius: 8 }}>
        <strong>Visualization:</strong> The spiral shows the decay of the wavefunction magnitude |ψ(τ)| = e^(-ητ)
        combined with phase rotation θ(τ) = λτ. As τ increases, the magnitude decays exponentially while rotating
        around the origin, creating a logarithmic spiral pattern in the complex plane.
      </div>
    </>
  )
}

function OracleTab({ constants, convergence }: any) {
  if (!constants || !convergence) return <div>Loading...</div>

  // Use measured values for display
  const measured_eta = convergence.measured_eta
  const measured_lambda = convergence.measured_lambda
  const theoretical_eta = convergence.theoretical_eta
  const theoretical_lambda = convergence.theoretical_lambda

  // Equilateral triangle vertices
  const v1 = { x: 0, y: 0, label: '(0, 0)' }
  const v2 = { x: 1, y: 0, label: '(1, 0)' }
  const v3 = { x: 0.5, y: Math.sqrt(3)/2, label: '(1/2, √3/2)' }

  // Calculate distances using Viviani's theorem for MEASURED values
  const SQRT_3 = Math.sqrt(3)
  const ALTITUDE = SQRT_3 / 2

  // Measured distances
  const d1 = Math.abs(measured_lambda)
  const d2 = Math.abs(SQRT_3 * measured_eta - measured_lambda) / 2
  const d3 = Math.abs(SQRT_3 * measured_eta + measured_lambda - SQRT_3) / 2
  const sum = d1 + d2 + d3
  const measured_delta = sum / ALTITUDE - 1.0

  // Theoretical distances
  const d1_theo = Math.abs(theoretical_lambda)
  const d2_theo = Math.abs(SQRT_3 * theoretical_eta - theoretical_lambda) / 2
  const d3_theo = Math.abs(SQRT_3 * theoretical_eta + theoretical_lambda - SQRT_3) / 2
  const sum_theo = d1_theo + d2_theo + d3_theo
  const theoretical_delta = sum_theo / ALTITUDE - 1.0

  // Scale for visualization (SVG coordinates)
  const scale = 400
  const points = [
    { x: v1.x * scale, y: v1.y * scale },
    { x: v2.x * scale, y: v2.y * scale },
    { x: v3.x * scale, y: v3.y * scale }
  ]

  // Measured point
  const pointX = measured_eta * scale
  const pointY = measured_lambda * scale

  // Theoretical point (for comparison)
  const theoX = theoretical_eta * scale
  const theoY = theoretical_lambda * scale

  // Convergence status
  const isConverging = convergence.convergence_confidence > 0.8
  const eta_converged = convergence.eta_error < 0.01
  const lambda_converged = convergence.lambda_error < 0.01
  const fully_converged = eta_converged && lambda_converged && isConverging

  return (
    <>
      {/* Conjecture Status Panel */}
      <div className="card" style={{
        background: fully_converged
          ? 'linear-gradient(135deg, #10b981 0%, #059669 100%)'
          : isConverging
          ? 'linear-gradient(135deg, #f59e0b 0%, #d97706 100%)'
          : 'linear-gradient(135deg, #64748b 0%, #475569 100%)',
        color: 'white',
        padding: 24
      }}>
        <h3 style={{ margin: 0, marginBottom: 16, fontSize: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
          {fully_converged ? <CheckCircle size={24} /> : <Target size={24} />}
          The Conjecture: Testing η = λ = 1/√2
        </h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16 }}>
          <ConvergenceMetric
            label="η Convergence"
            measured={measured_eta}
            theoretical={theoretical_eta}
            converged={eta_converged}
          />
          <ConvergenceMetric
            label="λ Convergence"
            measured={measured_lambda}
            theoretical={theoretical_lambda}
            converged={lambda_converged}
          />
          <div>
            <div style={{ fontSize: 12, opacity: 0.9, marginBottom: 4 }}>Confidence (R²)</div>
            <div style={{ fontSize: 24, fontWeight: 700 }}>{(convergence.convergence_confidence * 100).toFixed(1)}%</div>
            <div style={{ fontSize: 11, opacity: 0.8 }}>
              {convergence.convergence_confidence > 0.9 ? 'Strong convergence' :
               convergence.convergence_confidence > 0.7 ? 'Moderate convergence' : 'Collecting data...'}
            </div>
          </div>
          <div>
            <div style={{ fontSize: 12, opacity: 0.9, marginBottom: 4 }}>Oracle Δ (measured)</div>
            <div style={{ fontSize: 24, fontWeight: 700 }}>{convergence.measured_oracle_delta.toFixed(6)}</div>
            <div style={{ fontSize: 11, opacity: 0.8 }}>Target: 0.231</div>
          </div>
        </div>
      </div>

      {/* Viviani Oracle Visualization */}
      <div className="card">
        <h3 className="card-header">
          <Triangle size={20} style={{ display: 'inline', marginRight: 8 }} />
          Viviani Oracle - Convergence Trajectory
        </h3>
        <div style={{ padding: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20 }}>
            {/* SVG Triangle */}
            <div>
              <svg width="100%" height="400" viewBox="-50 -350 500 450" style={{ border: '1px solid #e2e8f0', borderRadius: 8, background: 'white' }}>
                {/* Triangle */}
                <polygon
                  points={`${points[0].x},${-points[0].y} ${points[1].x},${-points[1].y} ${points[2].x},${-points[2].y}`}
                  fill="none"
                  stroke="#667eea"
                  strokeWidth="2"
                />

                {/* Vertices */}
                {points.map((p, i) => (
                  <circle key={i} cx={p.x} cy={-p.y} r="4" fill="#667eea" />
                ))}

                {/* Theoretical Point (target) */}
                <circle cx={theoX} cy={-theoY} r="8" fill="none" stroke="#10b981" strokeWidth="2" strokeDasharray="4,4" />
                <circle cx={theoX} cy={-theoY} r="2" fill="#10b981" />

                {/* Trajectory line from measured to theoretical */}
                <line
                  x1={pointX}
                  y1={-pointY}
                  x2={theoX}
                  y2={-theoY}
                  stroke="#f59e0b"
                  strokeWidth="1"
                  strokeDasharray="3,3"
                />

                {/* Measured Point (current) */}
                <circle cx={pointX} cy={-pointY} r="6" fill="#f56565" />

                {/* Distance lines for measured point */}
                <line x1={pointX} y1={-pointY} x2={pointX} y2="0" stroke="#48bb78" strokeWidth="1" strokeDasharray="5,5" opacity="0.5" />

                {/* Labels */}
                <text x={points[0].x - 20} y={-points[0].y + 20} fontSize="12" fill="#4a5568">{v1.label}</text>
                <text x={points[1].x + 10} y={-points[1].y + 20} fontSize="12" fill="#4a5568">{v2.label}</text>
                <text x={points[2].x - 30} y={-points[2].y - 10} fontSize="12" fill="#4a5568">{v3.label}</text>
                <text x={theoX + 10} y={-theoY - 15} fontSize="12" fill="#10b981" fontWeight="600">Target: 1/√2</text>
                <text x={pointX + 10} y={-pointY + 15} fontSize="12" fill="#f56565" fontWeight="600">Measured</text>
              </svg>
            </div>

            {/* Metrics */}
            <div>
              <div style={{ marginBottom: 20 }}>
                <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>Measured Values:</div>
                <div style={{ display: 'grid', gap: 12 }}>
                  <MetricRow label="η (measured)" value={measured_eta.toFixed(6)} color="#f56565" />
                  <MetricRow label="λ (measured)" value={measured_lambda.toFixed(6)} color="#f56565" />
                  <MetricRow label="η (theoretical)" value={theoretical_eta.toFixed(6)} color="#10b981" />
                  <MetricRow label="λ (theoretical)" value={theoretical_lambda.toFixed(6)} color="#10b981" />
                </div>
              </div>

              <div style={{ marginBottom: 20 }}>
                <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>Viviani Distances (Measured):</div>
                <div style={{ display: 'grid', gap: 12 }}>
                  <MetricRow label="d1 (to bottom)" value={d1.toFixed(6)} color="#48bb78" />
                  <MetricRow label="d2 (to left)" value={d2.toFixed(6)} color="#ed8936" />
                  <MetricRow label="d3 (to right)" value={d3.toFixed(6)} color="#9f7aea" />
                  <MetricRow label="Sum (d1 + d2 + d3)" value={sum.toFixed(6)} color="#667eea" />
                </div>
              </div>

              <div style={{
                padding: 16,
                background: Math.abs(measured_delta) < 0.05 ? '#d1fae5' : '#fff5f5',
                borderRadius: 8,
                border: `2px solid ${Math.abs(measured_delta) < 0.05 ? '#10b981' : '#f59e0b'}`
              }}>
                <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 8 }}>
                  Oracle Δ (Measured):
                </div>
                <div style={{ fontSize: 24, fontWeight: 700, color: Math.abs(measured_delta) < 0.05 ? '#065f46' : '#d97706' }}>
                  {measured_delta.toFixed(6)}
                </div>
                <div style={{ fontSize: 12, marginTop: 8, color: '#4a5568' }}>
                  Distance from theoretical: {Math.abs(measured_delta - theoretical_delta).toFixed(6)}
                </div>
              </div>
            </div>
          </div>

          <div style={{ marginTop: 20, padding: 16, background: '#edf2f7', borderRadius: 8, fontSize: 13, color: '#4a5568' }}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
              <div>
                <strong>The Conjecture:</strong> "Optimal decentralized consensus naturally converges to the critical constant η = λ = 1/√2 ≈ 0.707107" (White Paper Section 11)
              </div>
              <div>
                <strong>Viviani Oracle:</strong> Geometric validator using equilateral triangle distances. The measured point should converge to the theoretical target as the network grows.
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  )
}

// Convergence Metric Component
function ConvergenceMetric({ label, measured, theoretical, converged }: any) {
  const error = Math.abs(measured - theoretical)
  const errorPercent = (error / theoretical) * 100

  return (
    <div>
      <div style={{ fontSize: 12, opacity: 0.9, marginBottom: 4 }}>
        {label} {converged ? '✓' : '⏳'}
      </div>
      <div style={{ fontSize: 20, fontWeight: 700 }}>
        {measured.toFixed(6)}
      </div>
      <div style={{ fontSize: 11, opacity: 0.8 }}>
        Target: {theoretical.toFixed(6)} (±{errorPercent.toFixed(2)}%)
      </div>
    </div>
  )
}

// Helper Components

interface TabButtonProps {
  active: boolean
  onClick: () => void
  children: React.ReactNode
}

function TabButton({ active, onClick, children }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: '12px 20px',
        background: active ? 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)' : 'transparent',
        color: active ? 'white' : '#4a5568',
        border: 'none',
        borderRadius: '8px 8px 0 0',
        fontSize: 14,
        fontWeight: 600,
        cursor: 'pointer',
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        transition: 'all 0.2s'
      }}
    >
      {children}
    </button>
  )
}

function ConsensusMetric({ label, value, description }: { label: string; value: string; description: string }) {
  return (
    <div>
      <div style={{ fontSize: 12, opacity: 0.9, marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 24, fontWeight: 700, marginBottom: 4 }}>{value}</div>
      <div style={{ fontSize: 11, opacity: 0.8 }}>{description}</div>
    </div>
  )
}

function PoolCard({ name, label, color, data }: any) {
  if (!data) return <div className="card" style={{ padding: 16 }}>Loading...</div>

  return (
    <div className="card" style={{ padding: 16, borderLeft: `4px solid ${color}` }}>
      <div style={{ fontSize: 16, fontWeight: 700, color, marginBottom: 8 }}>{name} - {label}</div>
      <div style={{ fontSize: 20, fontWeight: 700 }}>{data.total.toLocaleString()}</div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 8, fontSize: 12 }}>
        <div style={{ background: '#fef5e7', padding: 6, borderRadius: 4 }}>
          🔒 {data.locked.toLocaleString()}
        </div>
        <div style={{ background: '#d1fae5', padding: 6, borderRadius: 4 }}>
          🔓 {data.unlocked.toLocaleString()}
        </div>
      </div>
    </div>
  )
}

function LockedUnlockedChart({ allPools }: any) {
  if (!allPools) return <div style={{ height: 300 }}>Loading...</div>

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

function MetricRow({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', padding: '8px 12px', background: '#f7fafc', borderRadius: 6, borderLeft: `3px solid ${color}` }}>
      <span style={{ fontSize: 14, color: '#4a5568' }}>{label}:</span>
      <span style={{ fontSize: 14, fontWeight: 600, color }}>{value}</span>
    </div>
  )
}
