import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { ApiError } from '../api/client'
import { rolesWithPermission } from '../api/permissionHints'

type ToastKind = 'error' | 'info' | 'success'

interface Toast {
  id: number
  kind: ToastKind
  title: string
  body?: string
  hint?: string
}

interface ToastApi {
  show: (t: Omit<Toast, 'id'>) => void
  /**
   * Convert an unknown error into a toast. For ApiError(U403), pulls the
   * permission name out of the message and appends the role hint.
   */
  showError: (action: string, err: unknown) => void
}

const Ctx = createContext<ToastApi | null>(null)

export function useToast() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useToast must be used inside <ToastProvider>')
  return ctx
}

const AUTODISMISS_MS = 8000

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])
  const nextId = useRef(1)

  const show = useCallback((t: Omit<Toast, 'id'>) => {
    const id = nextId.current++
    setToasts(prev => [...prev, { ...t, id }])
  }, [])

  const dismiss = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id))
  }, [])

  const showError = useCallback((action: string, err: unknown) => {
    if (err instanceof ApiError) {
      const isForbidden = err.status === 403 || err.code === 'U403'
      const perm = isForbidden ? extractPermission(err.message) : null
      if (perm) {
        const roles = rolesWithPermission(perm)
        show({
          kind: 'error',
          title: action,
          body: `You don't have the \`${perm}\` permission in this org.`,
          hint: roles.length
            ? `Ask an admin to grant you a role with this permission (${roles.join(', ')}).`
            : 'Ask an admin to grant you a role with this permission.',
        })
        return
      }
      show({ kind: 'error', title: action, body: err.message })
      return
    }
    const message = err instanceof Error ? err.message : String(err)
    show({ kind: 'error', title: action, body: message })
  }, [show])

  return (
    <Ctx.Provider value={{ show, showError }}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismiss} />
    </Ctx.Provider>
  )
}

function ToastViewport({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div
      style={{
        position: 'fixed',
        top: 56,
        right: 16,
        zIndex: 1000,
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        pointerEvents: 'none',
        maxWidth: 380,
      }}
    >
      {toasts.map(t => (
        <ToastCard key={t.id} toast={t} onDismiss={() => onDismiss(t.id)} />
      ))}
    </div>
  )
}

function ToastCard({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  useEffect(() => {
    const h = setTimeout(onDismiss, AUTODISMISS_MS)
    return () => clearTimeout(h)
  }, [onDismiss])

  const accent =
    toast.kind === 'error' ? 'var(--status-error, #c0392b)'
    : toast.kind === 'success' ? 'var(--status-success, #2a8f3e)'
    : 'var(--accent, #2166dc)'

  return (
    <div
      role="alert"
      style={{
        pointerEvents: 'auto',
        background: 'var(--bg-secondary, #fff)',
        border: '1px solid var(--color-border)',
        borderLeft: `3px solid ${accent}`,
        borderRadius: 6,
        padding: '10px 12px',
        boxShadow: 'var(--shadow-md, 0 4px 12px rgba(0,0,0,0.12))',
        fontSize: 13,
        color: 'var(--color-text)',
        display: 'flex',
        gap: 10,
        alignItems: 'flex-start',
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontWeight: 600, marginBottom: toast.body ? 4 : 0 }}>{toast.title}</div>
        {toast.body && (
          <div style={{ color: 'var(--color-text-muted)', lineHeight: 1.45 }}>{toast.body}</div>
        )}
        {toast.hint && (
          <div style={{ color: 'var(--color-text-muted)', lineHeight: 1.45, marginTop: 4, fontSize: 12 }}>
            {toast.hint}
          </div>
        )}
      </div>
      <button
        aria-label="Dismiss"
        onClick={onDismiss}
        style={{
          background: 'transparent',
          border: 'none',
          color: 'var(--color-text-muted)',
          cursor: 'pointer',
          fontSize: 14,
          lineHeight: 1,
          padding: 0,
        }}
      >
        ×
      </button>
    </div>
  )
}

/**
 * Pull the permission name out of a backend "permission required: X" message.
 * The Principal.require() error formats as "permission required: <perm>" or
 * "permission required: <perm> in process_group <uuid>".
 */
function extractPermission(message: string): string | null {
  const m = message.match(/permission required:\s*([a-z_]+\.[a-z_]+)/)
  return m ? m[1] : null
}
