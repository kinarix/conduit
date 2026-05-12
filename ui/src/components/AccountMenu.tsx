import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { useAuth } from '../context/AuthContext'

/**
 * Top-right avatar/initials button with a dropdown menu containing
 * Profile and Logout. Mounted as a floating element by Layout and
 * PlatformShell so it sits in the same screen position regardless of
 * which view is rendering underneath.
 */
export default function AccountMenu() {
  const { user, logout } = useAuth()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDoc)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  if (!user) return null

  const label = user.name || user.email
  const initials = initialsOf(user.name, user.email)

  return (
    <div
      ref={ref}
      style={{
        position: 'fixed',
        top: 12,
        right: 16,
        zIndex: 100,
      }}
    >
      <button
        aria-label="Account menu"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(v => !v)}
        style={{
          width: 32,
          height: 32,
          borderRadius: '50%',
          border: '1px solid var(--color-border)',
          background: open ? 'var(--color-primary)' : 'var(--color-surface-2)',
          color: open ? '#fff' : 'var(--color-text)',
          fontSize: 12,
          fontWeight: 600,
          cursor: 'pointer',
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: 0,
          boxShadow: 'var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.08))',
          transition: 'background 0.15s, color 0.15s',
        }}
      >
        {initials}
      </button>

      {open && (
        <div
          role="menu"
          style={{
            position: 'absolute',
            top: 38,
            right: 0,
            minWidth: 200,
            background: 'var(--bg-secondary, var(--color-surface))',
            border: '1px solid var(--color-border)',
            borderRadius: 6,
            boxShadow: 'var(--shadow-md, 0 4px 12px rgba(0,0,0,0.12))',
            padding: '6px 0',
            fontSize: 13,
          }}
        >
          <div style={{
            padding: '6px 12px',
            color: 'var(--color-text-muted)',
            fontSize: 11,
            borderBottom: '1px solid var(--color-border)',
            marginBottom: 4,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}>
            {label}
          </div>
          <Link
            to="/account"
            role="menuitem"
            onClick={() => setOpen(false)}
            style={menuItemStyle}
            onMouseEnter={e => (e.currentTarget.style.background = 'var(--color-surface-2)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
          >
            Profile
          </Link>
          <button
            role="menuitem"
            onClick={() => { setOpen(false); logout() }}
            style={{
              ...menuItemStyle,
              width: '100%',
              textAlign: 'left',
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              fontFamily: 'inherit',
              fontSize: 'inherit',
              color: 'var(--status-error, #c0392b)',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = 'var(--color-surface-2)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
          >
            Logout
          </button>
        </div>
      )}
    </div>
  )
}

function initialsOf(name: string | null, email: string): string {
  if (name) {
    const parts = name.trim().split(/\s+/).filter(Boolean)
    if (parts.length >= 2) return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase()
    if (parts.length === 1 && parts[0].length > 0) return parts[0][0].toUpperCase()
  }
  return (email[0] || '?').toUpperCase()
}

const menuItemStyle: React.CSSProperties = {
  display: 'block',
  padding: '6px 12px',
  color: 'var(--color-text)',
  textDecoration: 'none',
  transition: 'background 0.1s',
}
