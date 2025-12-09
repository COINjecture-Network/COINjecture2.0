import { useState } from 'react'
import { Wallet, BarChart3, Network, ShoppingBag } from 'lucide-react'
import WalletView from './components/WalletView'
import ExplorerView from './components/ExplorerView'
import AdvancedMetricsView from './components/AdvancedMetricsView'
import Marketplace from './pages/Marketplace'

type Tab = 'wallet' | 'explorer' | 'metrics' | 'marketplace'

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('wallet')

  return (
    <div style={{ minHeight: '100vh' }}>
      {/* Header */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div style={{ textAlign: 'center' }}>
          <h1 style={{
            fontSize: 32,
            fontWeight: 700,
            marginBottom: 8,
            background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            backgroundClip: 'text'
          }}>
            COINjecture Network B
          </h1>
          <p style={{ color: '#718096', marginBottom: 16 }}>
            WEB4 Testnet • Dimensional Economics • η = λ = 1/√2
          </p>

          {/* Tab Navigation */}
          <div style={{
            display: 'flex',
            gap: 8,
            justifyContent: 'center',
            borderTop: '1px solid #e2e8f0',
            paddingTop: 16
          }}>
            <TabButton
              active={activeTab === 'wallet'}
              onClick={() => setActiveTab('wallet')}
              icon={<Wallet size={18} />}
              label="Wallet"
            />
            <TabButton
              active={activeTab === 'explorer'}
              onClick={() => setActiveTab('explorer')}
              icon={<Network size={18} />}
              label="Explorer"
            />
            <TabButton
              active={activeTab === 'metrics'}
              onClick={() => setActiveTab('metrics')}
              icon={<BarChart3 size={18} />}
              label="Metrics"
            />
            <TabButton
              active={activeTab === 'marketplace'}
              onClick={() => setActiveTab('marketplace')}
              icon={<ShoppingBag size={18} />}
              label="Marketplace"
            />
          </div>
        </div>
      </div>

      {/* Tab Content */}
      {activeTab === 'wallet' && <WalletView />}
      {activeTab === 'explorer' && <ExplorerView />}
      {activeTab === 'metrics' && <AdvancedMetricsView />}
      {activeTab === 'marketplace' && <Marketplace />}

      {/* Footer */}
      <div style={{
        textAlign: 'center',
        padding: 20,
        color: 'white',
        fontSize: 14
      }}>
        <p>COINjecture Network B v4.5.0 • Testnet</p>
        <p style={{ marginTop: 4, opacity: 0.8 }}>
          Proof of Useful Work (PoUW) • NP-Hard Consensus
        </p>
      </div>
    </div>
  )
}

interface TabButtonProps {
  active: boolean
  onClick: () => void
  icon: React.ReactNode
  label: string
}

function TabButton({ active, onClick, icon, label }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '10px 20px',
        background: active ? '#667eea' : 'transparent',
        color: active ? 'white' : '#4a5568',
        border: active ? 'none' : '1px solid #e2e8f0',
        borderRadius: 6,
        cursor: 'pointer',
        fontWeight: 500,
        transition: 'all 0.2s'
      }}
    >
      {icon}
      {label}
    </button>
  )
}

export default App
