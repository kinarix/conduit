import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '../../context/AuthContext'
import { fetchOrgs, type Org } from '../../api/orgs'
import {
  createOrgUser,
  grantOrgRole,
  listOrgUsers,
  listBuiltinRoles,
  type OrgUser,
  type AdminRole,
} from '../../api/admin'
import InstanceSetup from './InstanceSetup'

type AdminUser = OrgUser

const listUsersInOrg = (orgId: string) => listOrgUsers(orgId)

type View =
  | { kind: 'list' }
  | { kind: 'wizard' }
  | { kind: 'org-users'; org: Org }

export default function PlatformShell() {
  const { user, logout } = useAuth()
  const [view, setView] = useState<View>({ kind: 'list' })

  const orgsQ = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh', background: 'var(--bg-primary)' }}>
      <header style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '12px 24px',
        borderBottom: '1px solid var(--color-border)',
        background: 'var(--bg-secondary)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ fontSize: 15, fontWeight: 700 }}>Conduit</span>
          <span style={{
            fontSize: 10, fontWeight: 600, letterSpacing: '0.06em',
            color: 'var(--color-primary)',
            padding: '2px 6px',
            border: '1px solid var(--color-primary)',
            borderRadius: 4,
            textTransform: 'uppercase',
          }}>
            Platform Admin
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>{user?.email}</span>
          <button className="btn-ghost" style={{ fontSize: 12, padding: '4px 10px' }} onClick={logout}>
            Sign out
          </button>
        </div>
      </header>

      <main style={{ flex: 1, padding: '24px 0' }}>
        {view.kind === 'wizard' && (
          <InstanceSetup
            onCancel={() => setView({ kind: 'list' })}
            onComplete={() => setView({ kind: 'list' })}
          />
        )}

        {view.kind === 'list' && (
          <OrgList
            orgs={orgsQ.data ?? []}
            isLoading={orgsQ.isLoading}
            isError={orgsQ.isError}
            onNewOrg={() => setView({ kind: 'wizard' })}
            onManageUsers={org => setView({ kind: 'org-users', org })}
          />
        )}

        {view.kind === 'org-users' && (
          <OrgUsers org={view.org} onBack={() => setView({ kind: 'list' })} />
        )}
      </main>
    </div>
  )
}

// ─── Org list ────────────────────────────────────────────────────────────────

function OrgList({
  orgs, isLoading, isError, onNewOrg, onManageUsers,
}: {
  orgs: Org[]
  isLoading: boolean
  isError: boolean
  onNewOrg: () => void
  onManageUsers: (org: Org) => void
}) {
  if (isLoading) {
    return <div style={{ textAlign: 'center', padding: 40 }}><div className="spinner" /></div>
  }
  if (isError) {
    return <div style={{ color: 'var(--status-error)', padding: 24 }}>Failed to load organisations.</div>
  }

  if (orgs.length === 0) {
    return (
      <div style={{ maxWidth: 480, margin: '60px auto', textAlign: 'center', padding: '0 24px' }}>
        <h2 style={{ fontSize: 18, fontWeight: 700, marginBottom: 8 }}>No organisations yet</h2>
        <p style={{ fontSize: 13, color: 'var(--color-text-muted)', lineHeight: 1.5, marginBottom: 24 }}>
          Conduit is freshly installed. Create your first organisation to get started — you'll seed it
          with an Org Admin who can then invite their team.
        </p>
        <button className="btn-primary" onClick={onNewOrg}>
          Create your first organisation
        </button>
      </div>
    )
  }

  return (
    <div style={{ maxWidth: 880, margin: '0 auto', padding: '0 24px' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
        <div>
          <h2 style={{ fontSize: 16, fontWeight: 700, margin: 0 }}>Organisations</h2>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
            {orgs.length} organisation{orgs.length === 1 ? '' : 's'}
          </div>
        </div>
        <button className="btn-primary" onClick={onNewOrg} style={{ fontSize: 12, padding: '6px 14px' }}>
          + New organisation
        </button>
      </div>

      <div style={{ border: '1px solid var(--color-border)', borderRadius: 6, overflow: 'hidden' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr style={{ background: 'var(--color-surface-2)', borderBottom: '1px solid var(--color-border)' }}>
              <th style={thStyle}>Name</th>
              <th style={thStyle}>Slug</th>
              <th style={thStyle}>Created</th>
              <th style={{ ...thStyle, width: 140 }}></th>
            </tr>
          </thead>
          <tbody>
            {orgs.map((org, idx) => (
              <tr key={org.id} style={{ borderBottom: idx < orgs.length - 1 ? '1px solid var(--color-border)' : 'none' }}>
                <td style={tdStyle}><span style={{ fontSize: 13, fontWeight: 500 }}>{org.name}</span></td>
                <td style={tdStyle}><code style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>{org.slug}</code></td>
                <td style={tdStyle}>
                  <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>
                    {new Date(org.created_at).toLocaleDateString()}
                  </span>
                </td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>
                  <button
                    className="btn-ghost"
                    style={{ fontSize: 12, padding: '3px 8px' }}
                    onClick={() => onManageUsers(org)}
                  >
                    Manage users
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ─── Per-org user management ─────────────────────────────────────────────────

function OrgUsers({ org, onBack }: { org: Org; onBack: () => void }) {
  const qc = useQueryClient()
  const usersQ = useQuery({
    queryKey: ['org-users', org.id],
    queryFn: () => listUsersInOrg(org.id),
  })
  const rolesQ = useQuery({ queryKey: ['builtin-roles'], queryFn: listBuiltinRoles })
  const [showAdd, setShowAdd] = useState(false)

  const createMut = useMutation({
    mutationFn: async (body: {
      org_id: string
      email: string
      auth_provider: 'internal' | 'external'
      password?: string
      external_id?: string
      role_ids?: string[]
    }) => {
      const user = await createOrgUser(body.org_id, {
        email: body.email,
        auth_provider: body.auth_provider,
        password: body.password,
        external_id: body.external_id,
      })
      for (const rid of body.role_ids ?? []) {
        await grantOrgRole(body.org_id, user.id, rid)
      }
      return user
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-users', org.id] })
      setShowAdd(false)
    },
  })

  const users = usersQ.data ?? []
  const roles = rolesQ.data ?? []

  return (
    <div style={{ maxWidth: 880, margin: '0 auto', padding: '0 24px' }}>
      <button
        onClick={onBack}
        style={{
          fontSize: 12, color: 'var(--color-text-muted)',
          background: 'none', border: 'none', cursor: 'pointer',
          padding: 0, marginBottom: 16,
        }}
      >
        ← Back to organisations
      </button>

      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
        <div>
          <h2 style={{ fontSize: 16, fontWeight: 700, margin: 0 }}>{org.name}</h2>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
            <code>{org.slug}</code> · {users.length} user{users.length === 1 ? '' : 's'}
          </div>
        </div>
        <button
          className="btn-primary"
          style={{ fontSize: 12, padding: '6px 14px' }}
          onClick={() => { createMut.reset(); setShowAdd(true) }}
        >
          + Add user
        </button>
      </div>

      {usersQ.isLoading ? (
        <div style={{ textAlign: 'center', padding: 40 }}><div className="spinner" /></div>
      ) : usersQ.isError ? (
        <div style={{ color: 'var(--status-error)' }}>Failed to load users.</div>
      ) : (
        <div style={{ border: '1px solid var(--color-border)', borderRadius: 6, overflow: 'hidden' }}>
          {users.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', fontSize: 13, color: 'var(--color-text-muted)' }}>
              No users in this organisation.
            </div>
          ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ background: 'var(--color-surface-2)', borderBottom: '1px solid var(--color-border)' }}>
                  <th style={thStyle}>Email</th>
                  <th style={thStyle}>Provider</th>
                </tr>
              </thead>
              <tbody>
                {users.map((u, idx) => (
                  <tr key={u.id} style={{ borderBottom: idx < users.length - 1 ? '1px solid var(--color-border)' : 'none' }}>
                    <td style={tdStyle}><span style={{ fontSize: 13 }}>{u.email}</span></td>
                    <td style={tdStyle}>
                      <span style={{
                        fontSize: 11, padding: '2px 6px', borderRadius: 4,
                        background: 'var(--color-surface-2)', color: 'var(--color-text-muted)',
                      }}>
                        {u.auth_provider}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {showAdd && (
        <AddUserToOrgModal
          orgId={org.id}
          orgName={org.name}
          roles={roles}
          pending={createMut.isPending}
          error={createMut.error as Error | null}
          onCancel={() => setShowAdd(false)}
          onSubmit={body => createMut.mutate(body)}
        />
      )}
    </div>
  )
}

// ─── Add-user modal (cross-org variant; targets a specific org_id) ───────────

function AddUserToOrgModal({
  orgId, orgName, roles, pending, error, onCancel, onSubmit,
}: {
  orgId: string
  orgName: string
  roles: AdminRole[]
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (body: {
    org_id: string
    email: string
    auth_provider: 'internal' | 'external'
    password?: string
    external_id?: string
    role_ids?: string[]
  }) => void
}) {
  const [email, setEmail] = useState('')
  const [provider, setProvider] = useState<'internal' | 'external'>('internal')
  const [password, setPassword] = useState('')
  const [externalId, setExternalId] = useState('')
  const [selectedRoleIds, setSelectedRoleIds] = useState<string[]>(() => {
    const orgAdmin = roles.find(r => r.name === 'OrgAdmin' && r.org_id === null)
    return orgAdmin ? [orgAdmin.id] : []
  })

  const toggleRole = (id: string) =>
    setSelectedRoleIds(prev =>
      prev.includes(id) ? prev.filter(x => x !== id) : [...prev, id]
    )

  const canSubmit =
    email.trim().length > 0 &&
    !pending &&
    (provider === 'internal' ? password.length > 0 : externalId.trim().length > 0)

  const handleSubmit = () => {
    onSubmit({
      org_id: orgId,
      email: email.trim(),
      auth_provider: provider,
      password: provider === 'internal' ? password : undefined,
      external_id: provider === 'external' ? externalId.trim() : undefined,
      role_ids: selectedRoleIds.length > 0 ? selectedRoleIds : undefined,
    })
  }

  return (
    <div
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
        display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000,
      }}
      onClick={e => { if (e.target === e.currentTarget) onCancel() }}
    >
      <div style={{
        background: 'var(--bg-secondary)', border: '1px solid var(--color-border)',
        borderRadius: 8, padding: 24, width: 420,
        maxHeight: '90vh', overflowY: 'auto',
        boxShadow: 'var(--shadow-md)',
      }}>
        <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Add user</h3>
        <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
          Adding to <strong style={{ color: 'var(--color-text)' }}>{orgName}</strong>.
        </p>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div>
            <label style={fieldLabelStyle}>Email</label>
            <input type="email" autoFocus value={email}
              onChange={e => setEmail(e.target.value)}
              placeholder="user@example.com" style={inputStyle} />
          </div>

          <div>
            <label style={fieldLabelStyle}>Auth provider</label>
            <div style={{ display: 'flex', gap: 8 }}>
              {(['internal', 'external'] as const).map(p => (
                <label key={p} style={{
                  flex: 1, display: 'flex', alignItems: 'center', gap: 6,
                  padding: '6px 10px',
                  border: `1px solid ${provider === p ? 'var(--color-primary)' : 'var(--color-border)'}`,
                  borderRadius: 5, cursor: 'pointer', fontSize: 12,
                  background: provider === p
                    ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                    : 'transparent',
                }}>
                  <input type="radio" name="prov" checked={provider === p}
                    onChange={() => setProvider(p)} />
                  {p === 'internal' ? 'Internal (password)' : 'External (OIDC)'}
                </label>
              ))}
            </div>
          </div>

          {provider === 'internal' ? (
            <div>
              <label style={fieldLabelStyle}>Password</label>
              <input type="password" value={password}
                onChange={e => setPassword(e.target.value)}
                placeholder="Set an initial password" style={inputStyle} />
            </div>
          ) : (
            <div>
              <label style={fieldLabelStyle}>External ID</label>
              <input type="text" value={externalId}
                onChange={e => setExternalId(e.target.value)}
                placeholder="Subject identifier from your IdP" style={inputStyle} />
            </div>
          )}

          <div>
            <label style={fieldLabelStyle}>Roles</label>
            {roles.length === 0 ? (
              <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>No roles available.</div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, maxHeight: 200, overflowY: 'auto' }}>
                {roles.map(role => (
                  <label key={role.id} style={{
                    display: 'flex', alignItems: 'center', gap: 6,
                    padding: '4px 8px',
                    border: `1px solid ${selectedRoleIds.includes(role.id) ? 'var(--color-primary)' : 'var(--color-border)'}`,
                    borderRadius: 4, cursor: 'pointer', fontSize: 12,
                    background: selectedRoleIds.includes(role.id)
                      ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                      : 'transparent',
                  }}>
                    <input type="checkbox" checked={selectedRoleIds.includes(role.id)}
                      onChange={() => toggleRole(role.id)} />
                    {role.name}
                    {role.org_id === null && (
                      <span style={{ fontSize: 10, color: 'var(--color-text-muted)' }}>built-in</span>
                    )}
                  </label>
                ))}
              </div>
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
          <button className="btn-primary" disabled={!canSubmit} onClick={handleSubmit}>
            {pending ? 'Adding…' : 'Add user'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── Shared styles ───────────────────────────────────────────────────────────

const thStyle: React.CSSProperties = {
  padding: '8px 12px', fontSize: 11, fontWeight: 600,
  textAlign: 'left', color: 'var(--color-text-muted)',
  textTransform: 'uppercase', letterSpacing: '0.04em',
}

const tdStyle: React.CSSProperties = {
  padding: '10px 12px', fontSize: 13, verticalAlign: 'middle',
}

const roleChipStyle: React.CSSProperties = {
  fontSize: 11, padding: '2px 6px', borderRadius: 4,
  background: 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 10%, transparent))',
  color: 'var(--color-primary)', fontWeight: 500,
}

const fieldLabelStyle: React.CSSProperties = {
  display: 'block', fontSize: 11, fontWeight: 600,
  color: 'var(--color-text-muted)',
  textTransform: 'uppercase', letterSpacing: '0.04em',
  marginBottom: 6,
}

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '6px 10px', fontSize: 13,
  border: '1px solid var(--color-border)', borderRadius: 5,
  background: 'var(--bg-primary)', color: 'var(--color-text)',
  boxSizing: 'border-box',
}
