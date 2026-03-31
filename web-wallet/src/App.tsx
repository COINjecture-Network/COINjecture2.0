import { useState } from 'react'
import { Wallet, BarChart3, Network, ShoppingBag } from 'lucide-react'
import WalletView from './components/WalletView'
import ExplorerView from './components/ExplorerView'
import AdvancedMetricsView from './components/AdvancedMetricsView'
import Marketplace from './pages/Marketplace'
import { ToastProvider } from './components/Toast'

type Tab = 'wallet' | 'explorer' | 'metrics' | 'marketplace'

const TAB_LABELS: Record<Tab, string> = {
  wallet: 'Wallet',
  explorer: 'Explorer',
  metrics: 'Metrics',
  marketplace: 'Marketplace',
}

function AppContent() {
  const [activeTab, setActiveTab] = useState<Tab>('wallet')

  return (
    <div style={{ minHeight: '100vh' }}>
      {/* Header */}
      <header>
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ textAlign: 'center' }}>
            <h1
              aria-label="COINjecture Network B"
              style={{
                fontSize: 32,
                fontWeight: 700,
                marginBottom: 8,
                background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
              }}
            >
              COINjecture Network B
            </h1>
            <p style={{ color: '#718096', marginBottom: 16 }}>
              WEB4 Testnet • Dimensional Economics • η = λ = 1/√2
            </p>

            {/* Tab Navigation */}
            <nav
              aria-label="Main navigation"
              role="navigation"
            >
              <div
                className="tab-nav"
                style={{
                  display: 'flex',
                  gap: 8,
                  justifyContent: 'center',
                  borderTop: '1px solid #e2e8f0',
                  paddingTop: 16,
                }}
              >
                {(['wallet', 'explorer', 'metrics', 'marketplace'] as Tab[]).map(tab => (
                  <TabButton
                    key={tab}
                    tab={tab}
                    active={activeTab === tab}
                    onClick={() => setActiveTab(tab)}
                  />
                ))}
              </div>
            </nav>
          </div>
        </div>
      </header>

      {/* Tab Content */}
      <main id="main-content" tabIndex={-1}>
        {activeTab === 'wallet'      && <WalletView />}
        {activeTab === 'explorer'    && <ExplorerView />}
        {activeTab === 'metrics'     && <AdvancedMetricsView />}
        {activeTab === 'marketplace' && <Marketplace />}
      </main>

      {/* Footer */}
      <footer
        role="contentinfo"
        style={{ textAlign: 'center', padding: 20, color: 'white', fontSize: 14 }}
      >
        <p>COINjecture Network B v4.5.0 • Testnet</p>
        <p style={{ marginTop: 4, opacity: 0.8 }}>
          Proof of Useful Work (PoUW) • NP-Hard Consensus
        </p>
        <p style={{ marginTop: 4, opacity: 0.6, fontSize: 12 }}>
          ⚠ Testnet only — do not use real funds
        </p>
      </footer>
    </div>
  )
}

// ── Tab icon map ──────────────────────────────────────────────────────────────
const TAB_ICONS: Record<Tab, JSX.Element> = {
  wallet:      <Wallet size={18} aria-hidden="true" />,
  explorer:    <Network size={18} aria-hidden="true" />,
  metrics:     <BarChart3 size={18} aria-hidden="true" />,
  marketplace: <ShoppingBag size={18} aria-hidden="true" />,
}

interface TabButtonProps {
  tab: Tab
  active: boolean
  onClick: () => void
}

function TabButton({ tab, active, onClick }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      role="tab"
      aria-selected={active}
      aria-controls={`tabpanel-${tab}`}
      id={`tab-${tab}`}
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
        transition: 'all 0.2s',
      }}
    >
      {TAB_ICONS[tab]}
      {TAB_LABELS[tab]}
    </button>
  )
}

// Wrap everything in the ToastProvider so any child can call useToast()
export default function App() {
  return (
    <ToastProvider>
      <AppContent />
    </ToastProvider>
  )
}
