import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listAdminUsers, createAdminUser, removeAdminUser, setUserRoles, listAdminRoles,
  type AdminUser, type AdminRole,
} from '../../api/admin'

export default function AdminUsers() {
  const qc = useQueryClient()
  const usersQ = useQuery({ queryKey: ['admin-users'], queryFn: listAdminUsers })
  const rolesQ = useQuery({ queryKey: ['admin-roles'], queryFn: listAdminRoles })

  const [editingUser, setEditingUser] = useState<AdminUser | null>(null)
  const [selectedRoleIds, setSelectedRoleIds] = useState<string[]>([])
  const [removingId, setRemovingId] = useState<string | null>(null)
  const [showAdd, setShowAdd] = useState(false)

  const removeMut = useMutation({
    mutationFn: removeAdminUser,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin-users'] })
      setRemovingId(null)
    },
  })

  const setRolesMut = useMutation({
    mutationFn: ({ userId, roleIds }: { userId: string; roleIds: string[] }) =>
      setUserRoles(userId, roleIds),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin-users'] })
      setEditingUser(null)
    },
  })

  const createMut = useMutation({
    mutationFn: createAdminUser,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin-users'] })
      setShowAdd(false)
    },
  })

  const openEdit = (user: AdminUser) => {
    const roles = rolesQ.data ?? []
    const currentIds = roles
      .filter(r => user.roles.includes(r.name))
      .map(r => r.id)
    setSelectedRoleIds(currentIds)
    setEditingUser(user)
  }

  const toggleRole = (id: string) =>
    setSelectedRoleIds(prev =>
      prev.includes(id) ? prev.filter(x => x !== id) : [...prev, id]
    )

  if (usersQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (usersQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load users.</div>

  const users = usersQ.data ?? []
  const roles = rolesQ.data ?? []

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0 }}>Users</h2>
        <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
          <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>{users.length} member{users.length !== 1 ? 's' : ''}</span>
          <button
            className="btn-primary"
            style={{ fontSize: 12, padding: '5px 12px' }}
            onClick={() => { createMut.reset(); setShowAdd(true) }}
          >
            Add user
          </button>
        </div>
      </div>

      <div style={{ border: '1px solid var(--color-border)', borderRadius: 6, overflow: 'hidden' }}>
        {users.length === 0 ? (
          <div style={{ padding: '24px 16px', textAlign: 'center', fontSize: 13, color: 'var(--color-text-muted)' }}>
            No users found.
          </div>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ background: 'var(--color-surface-2)', borderBottom: '1px solid var(--color-border)' }}>
                <th style={thStyle}>Email</th>
                <th style={thStyle}>Provider</th>
                <th style={thStyle}>Roles</th>
                <th style={{ ...thStyle, width: 120 }}></th>
              </tr>
            </thead>
            <tbody>
              {users.map((user, idx) => (
                <tr
                  key={user.id}
                  style={{
                    borderBottom: idx < users.length - 1 ? '1px solid var(--color-border)' : 'none',
                    background: removingId === user.id ? 'var(--status-error-soft)' : 'transparent',
                  }}
                >
                  <td style={tdStyle}>
                    <span style={{ fontSize: 13 }}>{user.email}</span>
                  </td>
                  <td style={tdStyle}>
                    <span style={{
                      fontSize: 11,
                      padding: '2px 6px',
                      borderRadius: 4,
                      background: 'var(--color-surface-2)',
                      color: 'var(--color-text-muted)',
                    }}>
                      {user.auth_provider}
                    </span>
                  </td>
                  <td style={tdStyle}>
                    <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
                      {user.roles.length === 0
                        ? <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>No roles</span>
                        : user.roles.map(r => (
                          <span key={r} style={roleChipStyle}>{r}</span>
                        ))
                      }
                    </div>
                  </td>
                  <td style={{ ...tdStyle, textAlign: 'right' }}>
                    {removingId === user.id ? (
                      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                        <button
                          className="btn-ghost"
                          style={{ fontSize: 12, padding: '3px 8px', color: 'var(--status-error)' }}
                          disabled={removeMut.isPending}
                          onClick={() => removeMut.mutate(user.id)}
                        >
                          {removeMut.isPending ? 'Removing…' : 'Confirm remove'}
                        </button>
                        <button
                          className="btn-ghost"
                          style={{ fontSize: 12, padding: '3px 8px' }}
                          onClick={() => setRemovingId(null)}
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                        <button
                          className="btn-ghost"
                          style={{ fontSize: 12, padding: '3px 8px' }}
                          onClick={() => openEdit(user)}
                        >
                          Edit roles
                        </button>
                        <button
                          className="btn-ghost"
                          style={{ fontSize: 12, padding: '3px 8px', color: 'var(--status-error)' }}
                          onClick={() => setRemovingId(user.id)}
                        >
                          Remove
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Add user modal */}
      {showAdd && (
        <AddUserModal
          roles={roles}
          pending={createMut.isPending}
          error={createMut.error as Error | null}
          onCancel={() => setShowAdd(false)}
          onSubmit={body => createMut.mutate(body)}
        />
      )}

      {/* Role edit modal */}
      {editingUser && (
        <div style={{
          position: 'fixed', inset: 0,
          background: 'rgba(0,0,0,0.4)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          zIndex: 1000,
        }} onClick={e => { if (e.target === e.currentTarget) setEditingUser(null) }}>
          <div style={{
            background: 'var(--bg-secondary)',
            border: '1px solid var(--color-border)',
            borderRadius: 8,
            padding: 24,
            width: 360,
            boxShadow: 'var(--shadow-md)',
          }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Edit roles</h3>
            <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
              {editingUser.email}
            </p>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 300, overflowY: 'auto' }}>
              {roles.map(role => (
                <label
                  key={role.id}
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 8,
                    padding: '8px 10px',
                    border: `1px solid ${selectedRoleIds.includes(role.id) ? 'var(--color-primary)' : 'var(--color-border)'}`,
                    borderRadius: 5,
                    cursor: 'pointer',
                    background: selectedRoleIds.includes(role.id)
                      ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                      : 'transparent',
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedRoleIds.includes(role.id)}
                    onChange={() => toggleRole(role.id)}
                    style={{ marginTop: 2 }}
                  />
                  <div>
                    <div style={{ fontSize: 13, fontWeight: 500 }}>{role.name}</div>
                    {role.org_id === null && (
                      <div style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>Global built-in</div>
                    )}
                  </div>
                </label>
              ))}
            </div>

            {setRolesMut.isError && (
              <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 12 }}>
                {(setRolesMut.error as Error).message}
              </div>
            )}

            <div style={{ display: 'flex', gap: 8, marginTop: 16, justifyContent: 'flex-end' }}>
              <button className="btn-ghost" onClick={() => setEditingUser(null)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={setRolesMut.isPending}
                onClick={() => setRolesMut.mutate({ userId: editingUser.id, roleIds: selectedRoleIds })}
              >
                {setRolesMut.isPending ? 'Saving…' : 'Save roles'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

const thStyle: React.CSSProperties = {
  padding: '8px 12px',
  fontSize: 11,
  fontWeight: 600,
  textAlign: 'left',
  color: 'var(--color-text-muted)',
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
}

const tdStyle: React.CSSProperties = {
  padding: '10px 12px',
  fontSize: 13,
  verticalAlign: 'middle',
}

const roleChipStyle: React.CSSProperties = {
  fontSize: 11,
  padding: '2px 6px',
  borderRadius: 4,
  background: 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 10%, transparent))',
  color: 'var(--color-primary)',
  fontWeight: 500,
}

function AddUserModal({
  roles,
  pending,
  error,
  onCancel,
  onSubmit,
}: {
  roles: AdminRole[]
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (body: {
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
  const [selectedRoleIds, setSelectedRoleIds] = useState<string[]>([])

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
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.4)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
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
          width: 420,
          maxHeight: '90vh',
          overflowY: 'auto',
          boxShadow: 'var(--shadow-md)',
        }}
      >
        <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Add user</h3>
        <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
          The user will be added to your organisation. You can assign roles now or later.
        </p>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div>
            <label style={fieldLabelStyle}>Email</label>
            <input
              type="email"
              autoFocus
              value={email}
              onChange={e => setEmail(e.target.value)}
              placeholder="you@example.com"
              style={inputStyle}
            />
          </div>

          <div>
            <label style={fieldLabelStyle}>Auth provider</label>
            <div style={{ display: 'flex', gap: 8 }}>
              {(['internal', 'external'] as const).map(p => (
                <label
                  key={p}
                  style={{
                    flex: 1,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                    padding: '6px 10px',
                    border: `1px solid ${provider === p ? 'var(--color-primary)' : 'var(--color-border)'}`,
                    borderRadius: 5,
                    cursor: 'pointer',
                    fontSize: 12,
                    background: provider === p
                      ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                      : 'transparent',
                  }}
                >
                  <input
                    type="radio"
                    name="add-user-provider"
                    checked={provider === p}
                    onChange={() => setProvider(p)}
                  />
                  {p === 'internal' ? 'Internal (password)' : 'External (OIDC)'}
                </label>
              ))}
            </div>
          </div>

          {provider === 'internal' ? (
            <div>
              <label style={fieldLabelStyle}>Password</label>
              <input
                type="password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                placeholder="Set an initial password"
                style={inputStyle}
              />
            </div>
          ) : (
            <div>
              <label style={fieldLabelStyle}>External ID</label>
              <input
                type="text"
                value={externalId}
                onChange={e => setExternalId(e.target.value)}
                placeholder="Subject identifier from your IdP"
                style={inputStyle}
              />
            </div>
          )}

          <div>
            <label style={fieldLabelStyle}>Roles (optional)</label>
            {roles.length === 0 ? (
              <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>
                No roles available.
              </div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, maxHeight: 200, overflowY: 'auto' }}>
                {roles.map(role => (
                  <label
                    key={role.id}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 6,
                      padding: '4px 8px',
                      border: `1px solid ${selectedRoleIds.includes(role.id) ? 'var(--color-primary)' : 'var(--color-border)'}`,
                      borderRadius: 4,
                      cursor: 'pointer',
                      fontSize: 12,
                      background: selectedRoleIds.includes(role.id)
                        ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                        : 'transparent',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={selectedRoleIds.includes(role.id)}
                      onChange={() => toggleRole(role.id)}
                    />
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
          <button
            className="btn-primary"
            disabled={!canSubmit}
            onClick={handleSubmit}
          >
            {pending ? 'Adding…' : 'Add user'}
          </button>
        </div>
      </div>
    </div>
  )
}

const fieldLabelStyle: React.CSSProperties = {
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
