import { useEffect, useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { changeOwnPassword, updateMe } from '../../api/auth'
import { useAuth } from '../../context/AuthContext'

/**
 * Self-service account settings: edit profile (name + phone), change
 * password (internal-auth users only), and view your effective roles.
 * External-auth users see an explanation for password changes — they
 * rotate at their identity provider.
 */
export default function AdminAccount() {
  const { user } = useAuth()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [confirm, setConfirm] = useState('')
  const [success, setSuccess] = useState(false)

  const mut = useMutation({
    mutationFn: () => changeOwnPassword(current, next),
    onSuccess: () => {
      setCurrent(''); setNext(''); setConfirm('')
      setSuccess(true)
    },
  })

  const tooShort = next.length > 0 && next.length < 8
  const mismatch = confirm.length > 0 && next !== confirm
  const canSubmit =
    current.length > 0 &&
    next.length >= 8 &&
    next === confirm &&
    !mut.isPending

  if (user?.auth_provider !== 'internal') {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <ProfilePanel />
        <div style={panelStyle}>
          <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0, marginBottom: 8 }}>Password</h2>
          <p style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>
            You signed in via your identity provider. Password changes are managed
            there, not in Conduit.
          </p>
        </div>
        <AccessSummary />
      </div>
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
    <ProfilePanel />
    <div style={panelStyle}>
      <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0, marginBottom: 4 }}>Change password</h2>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 20 }}>
        Signed in as {user?.email}.
      </p>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 360 }}>
        <div>
          <label style={labelStyle}>Current password</label>
          <input
            type="password"
            value={current}
            onChange={e => { setSuccess(false); setCurrent(e.target.value) }}
            style={inputStyle}
            autoComplete="current-password"
          />
        </div>

        <div>
          <label style={labelStyle}>New password</label>
          <input
            type="password"
            value={next}
            onChange={e => { setSuccess(false); setNext(e.target.value) }}
            placeholder="At least 8 characters"
            style={inputStyle}
            autoComplete="new-password"
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
            onChange={e => { setSuccess(false); setConfirm(e.target.value) }}
            placeholder="Re-enter the new password"
            style={inputStyle}
            autoComplete="new-password"
          />
          {mismatch && (
            <div style={hintErrStyle}>Passwords do not match.</div>
          )}
        </div>

        {mut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)' }}>
            {(mut.error as Error).message}
          </div>
        )}
        {success && (
          <div style={{ fontSize: 12, color: 'var(--status-success, #2a8f3e)' }}>
            Password updated. Existing sessions remain signed in until your token expires.
          </div>
        )}

        <div>
          <button
            className="btn-primary"
            disabled={!canSubmit}
            onClick={() => mut.mutate()}
          >
            {mut.isPending ? 'Saving…' : 'Update password'}
          </button>
        </div>
      </div>
    </div>
    <AccessSummary />
    </div>
  )
}

/**
 * Edit your own display name and phone number. Both fields are
 * optional; clearing a value writes NULL on save. Email and auth
 * provider are immutable here — admins can change those.
 */
function ProfilePanel() {
  const { user, refreshUser } = useAuth()
  const [name, setName] = useState(user?.name ?? '')
  const [phone, setPhone] = useState(user?.phone ?? '')
  const [savedAt, setSavedAt] = useState<number | null>(null)

  // Re-sync local form state when refreshUser() lands new values.
  useEffect(() => {
    setName(user?.name ?? '')
    setPhone(user?.phone ?? '')
  }, [user?.name, user?.phone])

  const mut = useMutation({
    mutationFn: () => updateMe({ name, phone }),
    onSuccess: async () => {
      await refreshUser()
      setSavedAt(Date.now())
    },
  })

  const dirty =
    (name ?? '') !== (user?.name ?? '') ||
    (phone ?? '') !== (user?.phone ?? '')

  return (
    <div style={panelStyle}>
      <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0, marginBottom: 4 }}>Profile</h2>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 20 }}>
        These appear on your account menu and in admin listings.
      </p>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 360 }}>
        <div>
          <label style={labelStyle}>Email</label>
          <input
            type="email"
            value={user?.email ?? ''}
            disabled
            style={{ ...inputStyle, opacity: 0.7, cursor: 'not-allowed' }}
          />
        </div>
        <div>
          <label style={labelStyle}>Name</label>
          <input
            type="text"
            value={name}
            placeholder="e.g. Jane Doe"
            onChange={e => { setSavedAt(null); setName(e.target.value) }}
            style={inputStyle}
          />
        </div>
        <div>
          <label style={labelStyle}>Phone</label>
          <input
            type="tel"
            value={phone}
            placeholder="e.g. +1 555 123 4567"
            onChange={e => { setSavedAt(null); setPhone(e.target.value) }}
            style={inputStyle}
          />
        </div>

        {mut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)' }}>
            {(mut.error as Error).message}
          </div>
        )}
        {savedAt !== null && !mut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-success, #2a8f3e)' }}>
            Profile updated.
          </div>
        )}

        <div>
          <button
            className="btn-primary"
            disabled={!dirty || mut.isPending}
            onClick={() => mut.mutate()}
          >
            {mut.isPending ? 'Saving…' : 'Save changes'}
          </button>
        </div>
      </div>
    </div>
  )
}

/**
 * Read-only summary of the caller's effective role grants. Renders the
 * three scope levels — global, per-org, per-process-group — so the user
 * can see exactly where their access comes from.
 */
function AccessSummary() {
  const { user } = useAuth()
  if (!user) return null

  const hasGlobal = user.is_global_admin || user.global_roles.length > 0
  const hasOrgs = user.orgs.length > 0

  return (
    <div style={panelStyle}>
      <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0, marginBottom: 4 }}>Your access</h2>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
        Roles you currently hold. Read-only — contact an admin to change them.
      </p>

      {hasGlobal && (
        <div style={{ marginBottom: 14 }}>
          <div style={sectionLabelStyle}>Platform-wide</div>
          <div style={chipsRowStyle}>
            {user.is_global_admin && <span style={badgeAdminStyle}>Platform admin</span>}
            {user.global_roles.map(r => <span key={r} style={chipStyle}>{r}</span>)}
          </div>
        </div>
      )}

      {hasOrgs ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {user.orgs.map(o => (
            <div key={o.id}>
              <div style={sectionLabelStyle}>{o.name}</div>
              {o.roles.length === 0 && o.pg_roles.length === 0 ? (
                <div style={{ fontSize: 12, color: 'var(--color-text-muted)', fontStyle: 'italic' }}>
                  {user.is_global_admin
                    ? 'Access cascades from your platform-wide grant.'
                    : 'Member, no explicit role.'}
                </div>
              ) : (
                <div style={chipsRowStyle}>
                  {o.roles.map(r => <span key={r} style={chipOrgStyle}>{r}</span>)}
                  {o.pg_roles.map((pg, i) => (
                    <span key={`${pg.process_group_id}-${pg.role_name}-${i}`} style={chipPgStyle}>
                      {pg.role_name} <span style={chipScopeStyle}>in {pg.process_group_name}</span>
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      ) : !hasGlobal && (
        <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>No org memberships.</div>
      )}
    </div>
  )
}

const panelStyle: React.CSSProperties = {
  padding: '24px 28px',
  border: '1px solid var(--color-border)',
  borderRadius: 8,
  background: 'var(--bg-secondary)',
  maxWidth: 480,
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

const sectionLabelStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  color: 'var(--color-text-muted)',
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  marginBottom: 6,
}

const chipsRowStyle: React.CSSProperties = {
  display: 'flex',
  flexWrap: 'wrap',
  gap: 6,
}

const chipStyle: React.CSSProperties = {
  fontSize: 12,
  padding: '2px 8px',
  borderRadius: 4,
  background: 'var(--bg-primary)',
  border: '1px solid var(--color-border)',
  color: 'var(--color-text)',
}

const chipOrgStyle: React.CSSProperties = {
  ...chipStyle,
  background: 'var(--bg-accent-soft, rgba(33, 102, 220, 0.08))',
  borderColor: 'var(--color-accent, #2166dc)',
  color: 'var(--color-accent, #2166dc)',
}

const chipPgStyle: React.CSSProperties = {
  ...chipStyle,
  background: 'var(--bg-warn-soft, rgba(180, 100, 0, 0.08))',
  borderColor: 'var(--color-warn, #b46400)',
  color: 'var(--color-warn, #b46400)',
}

const chipScopeStyle: React.CSSProperties = {
  fontSize: 11,
  fontStyle: 'italic',
  opacity: 0.75,
  fontWeight: 400,
}

const badgeAdminStyle: React.CSSProperties = {
  ...chipStyle,
  background: 'var(--status-success, #2a8f3e)',
  borderColor: 'var(--status-success, #2a8f3e)',
  color: '#fff',
  fontWeight: 600,
}
