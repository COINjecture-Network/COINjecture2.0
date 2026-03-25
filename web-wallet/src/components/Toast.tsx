import { createContext, useContext, useState, useCallback, ReactNode } from 'react'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

interface ToastItem {
  id: string
  type: ToastType
  message: string
  detail?: string
}

interface ToastContextValue {
  showToast: (type: ToastType, message: string, detail?: string) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([])

  const showToast = useCallback((type: ToastType, message: string, detail?: string) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    setToasts(prev => [...prev, { id, type, message, detail }])
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id))
    }, type === 'error' ? 8000 : 5000)
  }, [])

  const dismiss = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id))
  }, [])

  return (
    <ToastContext.Provider value={{ showToast }}>
      {children}
      <ToastContainer toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  )
}

export function useToast() {
  const ctx = useContext(ToastContext)
  if (!ctx) throw new Error('useToast must be used within ToastProvider')
  return ctx
}

const TOAST_STYLES: Record<ToastType, { bg: string; border: string; iconColor: string; icon: string }> = {
  success: { bg: '#f0fff4', border: '#48bb78', iconColor: '#276749', icon: '✓' },
  error:   { bg: '#fff5f5', border: '#fc8181', iconColor: '#c53030', icon: '✕' },
  warning: { bg: '#fffaf0', border: '#f6ad55', iconColor: '#c05621', icon: '⚠' },
  info:    { bg: '#ebf8ff', border: '#63b3ed', iconColor: '#2b6cb0', icon: 'ℹ' },
}

function ToastContainer({ toasts, onDismiss }: { toasts: ToastItem[]; onDismiss: (id: string) => void }) {
  if (!toasts.length) return null
  return (
    <div
      role="region"
      aria-label="Notifications"
      aria-live="polite"
      aria-atomic="false"
      style={{
        position: 'fixed',
        top: 16,
        right: 16,
        zIndex: 9999,
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        maxWidth: 420,
        width: 'calc(100vw - 32px)',
        pointerEvents: 'none',
      }}
    >
      {toasts.map(toast => {
        const s = TOAST_STYLES[toast.type]
        return (
          <div
            key={toast.id}
            role="alert"
            style={{
              background: s.bg,
              border: `1px solid ${s.border}`,
              borderLeft: `4px solid ${s.border}`,
              borderRadius: 8,
              padding: '12px 16px',
              display: 'flex',
              gap: 12,
              alignItems: 'flex-start',
              boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
              pointerEvents: 'auto',
            }}
          >
            <span
              aria-hidden="true"
              style={{ fontWeight: 700, color: s.iconColor, flexShrink: 0, fontSize: 16, lineHeight: '20px' }}
            >
              {s.icon}
            </span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: 600, fontSize: 14, color: '#1a202c', wordBreak: 'break-word' }}>
                {toast.message}
              </div>
              {toast.detail && (
                <div style={{ fontSize: 12, color: '#4a5568', marginTop: 4, wordBreak: 'break-word', fontFamily: 'monospace' }}>
                  {toast.detail}
                </div>
              )}
            </div>
            <button
              onClick={() => onDismiss(toast.id)}
              aria-label="Dismiss notification"
              style={{
                background: 'transparent',
                color: '#718096',
                padding: '0 4px',
                fontSize: 20,
                lineHeight: 1,
                flexShrink: 0,
                border: '1px solid transparent',
                borderRadius: 4,
                cursor: 'pointer',
              }}
            >
              ×
            </button>
          </div>
        )
      })}
    </div>
  )
}
