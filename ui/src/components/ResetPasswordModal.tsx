import { useState } from 'react'

/**
 * Admin-side "reset password" modal. Used from AdminUsers and the
 * platform-shell user list. The caller passes the mutation; the modal
 * owns input state, confirm-match validation, and the 8-char minimum.
 */
export default function ResetPasswordModal({
  email,
  pending,
  error,
  onCancel,
  onSubmit,
}: {
  email: string
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (newPassword: string) => void
}) {
  const [pw, setPw] = useState('')
  const [confirm, setConfirm] = useState('')

  const tooShort = pw.length > 0 && pw.length < 8
  const mismatch = confirm.length > 0 && pw !== confirm
  const canSubmit = pw.length >= 8 && pw === confirm && !pending

  return (
    <div
      style={{
        position: 'fixed', inset: 0,
        background: 'rgba(0,0,0,0.4)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        zIndex: 1000,
      }}
      onClick={e => { if (e.target === e.currentTarget) onCancel() }}
    >
      <div
        style={{
          background: 'var(--bg-secondary)',
          border: '1px solid var(--color-border)',
          borderRadius: 8,
          padding: 24,
          width: 360,
          boxShadow: 'var(--shadow-md)',
        }}
      >
        <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Reset password</h3>
        <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
          {email}
        </p>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div>
            <label style={labelStyle}>New password</label>
            <input
              type="password"
              autoFocus
              value={pw}
              onChange={e => setPw(e.target.value)}
              placeholder="At least 8 characters"
              style={inputStyle}
            />
            {tooShort && (
              <div style={hintErrStyle}>Must be at least 8 characters.</div>
            )}
          </div>

          <div>
            <label style={labelStyle}>Confirm</label>
            <input
              type="password"
              value={confirm}
              onChange={e => setConfirm(e.target.value)}
              placeholder="Re-enter the password"
              style={inputStyle}
            />
            {mismatch && (
              <div style={hintErrStyle}>Passwords do not match.</div>
            )}
          </div>
        </div>

        {error && (
          <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 12 }}>
            {error.message}
          </div>
        )}

        <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
          <button className="btn-ghost" onClick={onCancel}>Cancel</button>
          <button
            className="btn-primary"
            disabled={!canSubmit}
            onClick={() => onSubmit(pw)}
          >
            {pending ? 'Saving…' : 'Reset password'}
          </button>
        </div>
      </div>
    </div>
  )
}

const labelStyle: React.CSSProperties = {
  display: 'block',
  fontSize: 11,
  fontWeight: 600,
  color: 'var(--color-text-muted)',
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  marginBottom: 6,
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '6px 10px',
  fontSize: 13,
  border: '1px solid var(--color-border)',
  borderRadius: 5,
  background: 'var(--bg-primary)',
  color: 'var(--color-text)',
  boxSizing: 'border-box',
}

const hintErrStyle: React.CSSProperties = {
  fontSize: 11,
  color: 'var(--status-error)',
  marginTop: 4,
}
