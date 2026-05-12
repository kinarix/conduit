import { useMemo, useState } from 'react'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listOrgUsers,
  createOrgUser,
  removeOrgUser,
  listBuiltinRoles,
  listOrgRoles,
  listOrgRoleAssignments,
  grantOrgRole,
  revokeOrgRole,
  listPgRoleAssignments,
  grantPgRole,
  revokePgRole,
  resetOrgUserPassword,
  type OrgUser,
  type AdminRole,
  type PgRoleAssignment,
} from '../../api/admin'
import { fetchProcessGroups, type ProcessGroup } from '../../api/processGroups'
import { useOrg } from '../../App'
import { useAuth } from '../../context/AuthContext'
import ResetPasswordModal from '../../components/ResetPasswordModal'

/**
 * An individual role grant — either at org scope (every pg in the org)
 * or pinned to a specific process group.
 */
type Scope = { kind: 'org' } | { kind: 'pg'; pg_id: string }

interface Assignment {
  /** Server-side assignment row id (different rows live in different tables). */
  id: string
  role_id: string
  scope: Scope
}

const PERMISSION_IS_ORG_ONLY = (p: string): boolean => {
  // Mirrors `Permission::is_pg_scopable` on the server. A role containing
  // ANY of these cannot be granted at pg scope — hide that scope option.
  return (
    p.startsWith('org.') ||
    p.startsWith('org_member.') ||
    p.startsWith('user.') ||
    p.startsWith('role.') ||
    p.startsWith('role_assignment.') ||
    p.startsWith('auth_config.') ||
    p.startsWith('notification_config.') ||
    p.startsWith('secret.') ||
    p === 'api_key.manage' ||
    p === 'process_group.create' ||
    p === 'message.correlate' ||
    p === 'signal.broadcast'
  )
}

const roleIsPgScopable = (role: AdminRole): boolean =>
  role.permissions.length > 0 && !role.permissions.some(PERMISSION_IS_ORG_ONLY)

export default function AdminUsers() {
  const qc = useQueryClient()
  const { org } = useOrg()
  const orgId = org?.id
  const { user: me } = useAuth()
  const myId = me?.user_id
  // Roles the *caller* holds in the current org — used to hide actions
  // they aren't allowed to perform (e.g. an OrgAdmin cannot remove an
  // OrgOwner). Global admins bypass.
  const callerOrgRoles = new Set(
    me?.orgs.find(o => o.id === orgId)?.roles ?? [],
  )
  const callerIsOrgOwner = me?.is_global_admin || callerOrgRoles.has('OrgOwner')

  const usersQ = useQuery({
    queryKey: ['org-users', orgId],
    queryFn: () => listOrgUsers(orgId!),
    enabled: !!orgId,
  })
  const builtinRolesQ = useQuery({
    queryKey: ['builtin-roles'],
    queryFn: listBuiltinRoles,
  })
  const customRolesQ = useQuery({
    queryKey: ['org-roles', orgId],
    queryFn: () => listOrgRoles(orgId!),
    enabled: !!orgId,
  })
  const assignmentsQ = useQuery({
    queryKey: ['org-role-assignments', orgId],
    queryFn: () => listOrgRoleAssignments(orgId!),
    enabled: !!orgId,
  })
  const pgsQ = useQuery({
    queryKey: ['process-groups', orgId],
    queryFn: () => fetchProcessGroups(orgId!),
    enabled: !!orgId,
  })
  // One query per pg — N+1 today, fine until there's an "all pg
  // assignments in an org" endpoint.
  const pgAssignmentsQs = useQueries({
    queries: (pgsQ.data ?? []).map(pg => ({
      queryKey: ['pg-role-assignments', orgId, pg.id],
      queryFn: () => listPgRoleAssignments(orgId!, pg.id),
      enabled: !!orgId,
    })),
  })

  const [editingUser, setEditingUser] = useState<OrgUser | null>(null)
  /** Working copy of the editing user's full assignment list. Persisted
   *  via a diff against the server-side current state on save. */
  const [editingAssignments, setEditingAssignments] = useState<Assignment[]>([])
  const [removingId, setRemovingId] = useState<string | null>(null)
  const [showAdd, setShowAdd] = useState(false)
  const [resettingUser, setResettingUser] = useState<OrgUser | null>(null)

  const allRoles: AdminRole[] = useMemo(
    () => [...(builtinRolesQ.data ?? []), ...(customRolesQ.data ?? [])],
    [builtinRolesQ.data, customRolesQ.data],
  )

  const pgs: ProcessGroup[] = pgsQ.data ?? []
  const pgNameById = useMemo(() => {
    const m = new Map<string, string>()
    for (const pg of pgs) m.set(pg.id, pg.name)
    return m
  }, [pgs])

  // Stable fingerprint for the per-pg query results so useMemo can
  // depend on a primitive rather than an array of (possibly new) refs.
  const pgAssignmentsFingerprint = pgAssignmentsQs
    .map(q => (q.data as PgRoleAssignment[] | undefined)?.length ?? -1)
    .join(',')

  /** Combined view of every grant in the org, keyed by user_id. Each
   *  entry carries the server-side assignment id (different tables) and
   *  the scope so the editor can render and diff them uniformly. */
  const assignmentsByUser = useMemo(() => {
    const map = new Map<string, Assignment[]>()
    const push = (uid: string, a: Assignment) => {
      const arr = map.get(uid) ?? []
      arr.push(a)
      map.set(uid, arr)
    }
    for (const a of assignmentsQ.data ?? []) {
      push(a.user_id, { id: a.id, role_id: a.role_id, scope: { kind: 'org' } })
    }
    for (const q of pgAssignmentsQs) {
      const data = q.data as PgRoleAssignment[] | undefined
      if (!data) continue
      for (const a of data) {
        push(a.user_id, {
          id: a.id,
          role_id: a.role_id,
          scope: { kind: 'pg', pg_id: a.process_group_id },
        })
      }
    }
    return map
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [assignmentsQ.data, pgAssignmentsFingerprint])

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['org-users', orgId] })
    qc.invalidateQueries({ queryKey: ['org-role-assignments', orgId] })
    qc.invalidateQueries({ queryKey: ['pg-role-assignments', orgId] })
  }

  const removeMut = useMutation({
    mutationFn: (userId: string) => removeOrgUser(orgId!, userId),
    onSuccess: () => { invalidate(); setRemovingId(null) },
    // No onError — we surface `removeMut.error` inline in the row so the
    // user sees the server's reason (e.g. "cannot remove yourself",
    // "only an OrgOwner can remove another OrgOwner") instead of a
    // silent no-op.
  })

  const createMut = useMutation({
    mutationFn: async (body: {
      email: string
      auth_provider: 'internal' | 'external'
      password?: string
      external_id?: string
      assignments: Array<{ role_id: string; scope: Scope }>
    }) => {
      const user = await createOrgUser(orgId!, {
        email: body.email,
        auth_provider: body.auth_provider,
        password: body.password,
        external_id: body.external_id,
      })
      for (const a of body.assignments) {
        if (a.scope.kind === 'org') {
          await grantOrgRole(orgId!, user.id, a.role_id)
        } else {
          await grantPgRole(orgId!, a.scope.pg_id, user.id, a.role_id)
        }
      }
      return user
    },
    onSuccess: () => { invalidate(); setShowAdd(false) },
  })

  const resetPwMut = useMutation({
    mutationFn: ({ userId, newPassword }: { userId: string; newPassword: string }) =>
      resetOrgUserPassword(orgId!, userId, newPassword),
    onSuccess: () => { setResettingUser(null) },
  })

  /** Save the editor's working set by diffing role-id + scope tuples
   *  against the current server state. Each delta is one API call. */
  const setRolesMut = useMutation({
    mutationFn: async ({ userId, desired }: { userId: string; desired: Assignment[] }) => {
      const current = assignmentsByUser.get(userId) ?? []
      const keyOf = (a: { role_id: string; scope: Scope }) =>
        a.scope.kind === 'org' ? `o:${a.role_id}` : `p:${a.scope.pg_id}:${a.role_id}`
      const currentByKey = new Map(current.map(a => [keyOf(a), a]))
      const desiredKeys = new Set(desired.map(keyOf))

      for (const a of current) {
        if (!desiredKeys.has(keyOf(a))) {
          if (a.scope.kind === 'org') {
            await revokeOrgRole(orgId!, a.id)
          } else {
            await revokePgRole(orgId!, a.scope.pg_id, a.id)
          }
        }
      }
      for (const a of desired) {
        if (!currentByKey.has(keyOf(a))) {
          if (a.scope.kind === 'org') {
            await grantOrgRole(orgId!, userId, a.role_id)
          } else {
            await grantPgRole(orgId!, a.scope.pg_id, userId, a.role_id)
          }
        }
      }
    },
    onSuccess: () => { invalidate(); setEditingUser(null) },
  })

  const openEdit = (user: OrgUser) => {
    setEditingAssignments(assignmentsByUser.get(user.id) ?? [])
    setEditingUser(user)
  }

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (usersQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (usersQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load users.</div>

  const users = usersQ.data ?? []

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
              {users.map((user, idx) => {
                const userAssignments = assignmentsByUser.get(user.id) ?? []
                // Hide the destructive Remove action when the server would
                // reject it anyway:
                //   - self-removal is blocked unconditionally
                //   - removing an OrgOwner requires the caller to also be
                //     OrgOwner (or global admin)
                const isSelf = !!myId && user.id === myId
                const targetIsOrgOwner = userAssignments.some(a => {
                  const r = allRoles.find(r => r.id === a.role_id)
                  return a.scope.kind === 'org' && r?.name === 'OrgOwner'
                })
                const canRemove = !isSelf && (callerIsOrgOwner || !targetIsOrgOwner)
                return (
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
                        {userAssignments.length === 0
                          ? <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>No roles</span>
                          : userAssignments.map(a => {
                            const role = allRoles.find(r => r.id === a.role_id)
                            const roleName = role?.name ?? a.role_id.slice(0, 8)
                            if (a.scope.kind === 'org') {
                              return <span key={a.id} style={roleChipStyle}>{roleName}</span>
                            }
                            const pgName = pgNameById.get(a.scope.pg_id) ?? a.scope.pg_id.slice(0, 8)
                            return (
                              <span key={a.id} style={rolePgChipStyle}>
                                {roleName} <span style={chipScopeStyle}>in {pgName}</span>
                              </span>
                            )
                          })
                        }
                      </div>
                    </td>
                    <td style={{ ...tdStyle, textAlign: 'right' }}>
                      {removingId === user.id ? (
                        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4 }}>
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
                              onClick={() => { removeMut.reset(); setRemovingId(null) }}
                            >
                              Cancel
                            </button>
                          </div>
                          {removeMut.isError && (
                            <div style={{ fontSize: 11, color: 'var(--status-error)', maxWidth: 320, textAlign: 'right' }}>
                              {(removeMut.error as Error).message}
                            </div>
                          )}
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
                          {user.auth_provider === 'internal' && (
                            <button
                              className="btn-ghost"
                              style={{ fontSize: 12, padding: '3px 8px' }}
                              onClick={() => { resetPwMut.reset(); setResettingUser(user) }}
                            >
                              Reset password
                            </button>
                          )}
                          {canRemove && (
                          <button
                            className="btn-ghost"
                            style={{ fontSize: 12, padding: '3px 8px', color: 'var(--status-error)' }}
                            onClick={() => { removeMut.reset(); setRemovingId(user.id) }}
                          >
                            Remove
                          </button>
                          )}
                        </div>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>

      {resettingUser && (
        <ResetPasswordModal
          email={resettingUser.email}
          pending={resetPwMut.isPending}
          error={resetPwMut.error as Error | null}
          onCancel={() => setResettingUser(null)}
          onSubmit={pw => resetPwMut.mutate({ userId: resettingUser.id, newPassword: pw })}
        />
      )}

      {showAdd && (
        <AddUserModal
          roles={allRoles}
          pgs={pgs}
          pending={createMut.isPending}
          error={createMut.error as Error | null}
          onCancel={() => setShowAdd(false)}
          onSubmit={body => createMut.mutate(body)}
        />
      )}

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
            width: 440,
            maxHeight: '90vh',
            overflowY: 'auto',
            boxShadow: 'var(--shadow-md)',
          }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Edit roles</h3>
            <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
              {editingUser.email}
            </p>

            <RoleAssignmentEditor
              roles={allRoles}
              pgs={pgs}
              value={editingAssignments}
              onChange={setEditingAssignments}
            />

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
                onClick={() => setRolesMut.mutate({ userId: editingUser.id, desired: editingAssignments })}
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

const rolePgChipStyle: React.CSSProperties = {
  ...roleChipStyle,
  background: 'var(--bg-warn-soft, rgba(180, 100, 0, 0.10))',
  color: 'var(--color-warn, #b46400)',
}

const chipScopeStyle: React.CSSProperties = {
  fontSize: 10,
  fontStyle: 'italic',
  opacity: 0.75,
  fontWeight: 400,
}

/**
 * Edits a list of (role, scope) grants. Scope is either "Org-wide"
 * (only when the role is org-only or pg-scopable) or a specific process
 * group (only when the role contains no org-only permission).
 *
 * Reusable across AdminUsers' Edit modal and AddUserModal.
 */
function RoleAssignmentEditor({
  roles,
  pgs,
  value,
  onChange,
}: {
  roles: AdminRole[]
  pgs: ProcessGroup[]
  value: Assignment[]
  onChange: (next: Assignment[]) => void
}) {
  const [addRoleId, setAddRoleId] = useState<string>('')
  const [addScope, setAddScope] = useState<'org' | string>('org') // 'org' | pg_id

  const pgNameById = useMemo(() => {
    const m = new Map<string, string>()
    for (const pg of pgs) m.set(pg.id, pg.name)
    return m
  }, [pgs])

  const remove = (idx: number) =>
    onChange(value.filter((_, i) => i !== idx))

  const addRole = roles.find(r => r.id === addRoleId)
  const addRolePgScopable = addRole ? roleIsPgScopable(addRole) : false
  // Synthetic temp id for unsaved assignments; real id arrives after save.
  const newTempId = () => `new-${Math.random().toString(36).slice(2)}`

  const keyOf = (a: { role_id: string; scope: Scope }) =>
    a.scope.kind === 'org' ? `o:${a.role_id}` : `p:${a.scope.pg_id}:${a.role_id}`

  const tryAdd = () => {
    if (!addRoleId) return
    const scope: Scope = addScope === 'org' ? { kind: 'org' } : { kind: 'pg', pg_id: addScope }
    if (scope.kind === 'pg' && !addRolePgScopable) return
    const candidate = { role_id: addRoleId, scope }
    if (value.some(a => keyOf(a) === keyOf(candidate))) return
    onChange([...value, { id: newTempId(), ...candidate }])
    setAddRoleId('')
    setAddScope('org')
  }

  // When the role changes, snap scope back to 'org' if the new role
  // isn't pg-scopable.
  const onRoleChange = (id: string) => {
    setAddRoleId(id)
    const r = roles.find(x => x.id === id)
    if (r && !roleIsPgScopable(r)) setAddScope('org')
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {value.length === 0 && (
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', fontStyle: 'italic' }}>
            No role grants. Add one below.
          </div>
        )}
        {value.map((a, idx) => {
          const role = roles.find(r => r.id === a.role_id)
          const roleName = role?.name ?? a.role_id.slice(0, 8)
          const isPg = a.scope.kind === 'pg'
          const scopeLabel = isPg
            ? `in ${pgNameById.get((a.scope as { pg_id: string }).pg_id) ?? 'unknown'}`
            : 'Org-wide'
          return (
            <div
              key={a.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                padding: '6px 10px',
                border: '1px solid var(--color-border)',
                borderRadius: 5,
                background: isPg
                  ? 'var(--bg-warn-soft, rgba(180,100,0,0.06))'
                  : 'var(--bg-primary)',
              }}
            >
              <div style={{ flex: 1, fontSize: 13 }}>
                <span style={{ fontWeight: 500 }}>{roleName}</span>{' '}
                <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>{scopeLabel}</span>
              </div>
              <button
                className="btn-ghost"
                style={{ fontSize: 11, padding: '2px 6px', color: 'var(--status-error)' }}
                onClick={() => remove(idx)}
              >
                Remove
              </button>
            </div>
          )
        })}
      </div>

      <div
        style={{
          padding: 10,
          border: '1px dashed var(--color-border)',
          borderRadius: 5,
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
        }}
      >
        <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
          Add assignment
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <select
            value={addRoleId}
            onChange={e => onRoleChange(e.target.value)}
            style={{ flex: 1, fontSize: 12, padding: '4px 6px' }}
          >
            <option value="">Role…</option>
            {roles.map(r => (
              <option key={r.id} value={r.id}>
                {r.name}{r.org_id === null ? ' (built-in)' : ''}
              </option>
            ))}
          </select>
          <select
            value={addScope}
            onChange={e => setAddScope(e.target.value)}
            style={{ flex: 1, fontSize: 12, padding: '4px 6px' }}
            disabled={!addRoleId}
          >
            <option value="org">Org-wide</option>
            {addRolePgScopable && pgs.map(pg => (
              <option key={pg.id} value={pg.id}>{pg.name}</option>
            ))}
          </select>
          <button
            className="btn-primary"
            style={{ fontSize: 12, padding: '4px 10px' }}
            disabled={!addRoleId}
            onClick={tryAdd}
          >
            Add
          </button>
        </div>
        {addRole && !addRolePgScopable && (
          <div style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>
            This role contains org-only permissions and can only be granted org-wide.
          </div>
        )}
      </div>
    </div>
  )
}

function AddUserModal({
  roles,
  pgs,
  pending,
  error,
  onCancel,
  onSubmit,
}: {
  roles: AdminRole[]
  pgs: ProcessGroup[]
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (body: {
    email: string
    auth_provider: 'internal' | 'external'
    password?: string
    external_id?: string
    assignments: Array<{ role_id: string; scope: Scope }>
  }) => void
}) {
  const [email, setEmail] = useState('')
  const [provider, setProvider] = useState<'internal' | 'external'>('internal')
  const [password, setPassword] = useState('')
  const [externalId, setExternalId] = useState('')
  const [assignments, setAssignments] = useState<Assignment[]>([])

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
      assignments: assignments.map(a => ({ role_id: a.role_id, scope: a.scope })),
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
              <RoleAssignmentEditor
                roles={roles}
                pgs={pgs}
                value={assignments}
                onChange={setAssignments}
              />
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
