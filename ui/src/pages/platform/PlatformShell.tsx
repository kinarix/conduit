import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createOrg, deleteOrg, fetchOrgStats, fetchOrgs,
  type Org, type OrgStats,
} from '../../api/orgs'
import {
  createOrgUser,
  grantOrgRole,
  listOrgUsers,
  listBuiltinRoles,
  listOrgRoles,
  createOrgRole,
  updateOrgRole,
  removeOrgRole,
  listPlatformAdmins,
  createPlatformAdmin,
  patchPlatformAdmin,
  revokePlatformAdmin,
  resetGlobalUserPassword,
  resetOrgUserPassword,
  type OrgUser,
  type AdminRole,
  type PlatformAdmin,
} from '../../api/admin'
import ResetPasswordModal from '../../components/ResetPasswordModal'
import AccountMenu from '../../components/AccountMenu'
import IconButton from '../../components/IconButton'
import { KeyIcon, PencilIcon, TrashIcon, UsersIcon } from '../../components/Sidebar/SidebarIcons'
import { PERMISSION_DETAILS } from '../../api/permissionCatalog'

type AdminUser = OrgUser

const listUsersInOrg = (orgId: string) => listOrgUsers(orgId)

type View =
  | { kind: 'list' }
  | { kind: 'create-org' }
  | { kind: 'org-users'; org: Org }
  | { kind: 'roles' }
  | { kind: 'settings' }

type Section = 'orgs' | 'roles' | 'settings'

/** Map the active view to its corresponding tab section. The create-org
 *  modal and the per-org users drill-down both belong to the "orgs"
 *  section — the tabs stay visible (and "orgs" stays highlighted)
 *  across those screens so the admin always knows where they are. */
function viewSection(v: View): Section {
  switch (v.kind) {
    case 'list':
    case 'create-org':
    case 'org-users':
      return 'orgs'
    case 'roles':
      return 'roles'
    case 'settings':
      return 'settings'
  }
}

export default function PlatformShell() {
  const [view, setView] = useState<View>({ kind: 'list' })
  // Per-row delete affordance lives at the shell level (not inside
  // OrgList) so the confirm modal can sit above the table without
  // bleeding state into the list component.
  const [deletingOrg, setDeletingOrg] = useState<Org | null>(null)

  const orgsQ = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  const handleSectionSelect = (s: Section) => {
    if (s === 'orgs') setView({ kind: 'list' })
    else if (s === 'roles') setView({ kind: 'roles' })
    else setView({ kind: 'settings' })
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh', background: 'var(--bg-primary)' }}>
      <AccountMenu />
      <header style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '12px 24px',
        paddingRight: 64, // leave room for the floating AccountMenu (32px button + margin)
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
      </header>

      {/* Tabs persist across every view so the user never loses their
          place. The buttons sit within the same centered container as
          the content (max-width 880, 24px gutter) so headers and
          tables stay vertically aligned. */}
      <PlatformTabs current={viewSection(view)} onSelect={handleSectionSelect} />

      <main style={{ flex: 1, padding: '24px 0' }}>
        {view.kind === 'list' && (
          <OrgList
            orgs={orgsQ.data ?? []}
            isLoading={orgsQ.isLoading}
            isError={orgsQ.isError}
            onNewOrg={() => setView({ kind: 'create-org' })}
            onManageUsers={org => setView({ kind: 'org-users', org })}
            onDelete={setDeletingOrg}
          />
        )}

        {view.kind === 'create-org' && (
          <CreateOrgPage
            onCancel={() => setView({ kind: 'list' })}
            onCreated={org => {
              orgsQ.refetch()
              setView({ kind: 'org-users', org })
            }}
          />
        )}

        {view.kind === 'org-users' && (
          <OrgUsers org={view.org} onBack={() => setView({ kind: 'list' })} />
        )}

        {view.kind === 'roles' && <PlatformRoles />}

        {view.kind === 'settings' && <PlatformSettings />}
      </main>

      {deletingOrg && (
        <DeleteOrgModal
          org={deletingOrg}
          onCancel={() => setDeletingOrg(null)}
          onDeleted={() => {
            setDeletingOrg(null)
            orgsQ.refetch()
          }}
        />
      )}
    </div>
  )
}

// ─── Org list ────────────────────────────────────────────────────────────────

function OrgList({
  orgs, isLoading, isError, onNewOrg, onManageUsers, onDelete,
}: {
  orgs: Org[]
  isLoading: boolean
  isError: boolean
  onNewOrg: () => void
  onManageUsers: (org: Org) => void
  onDelete: (org: Org) => void
}) {
  if (isLoading) {
    return <div style={{ textAlign: 'center', padding: 40 }}><div className="spinner" /></div>
  }
  if (isError) {
    return <div style={{ color: 'var(--status-error)', padding: 24 }}>Failed to load organisations.</div>
  }

  if (orgs.length === 0) {
    // Fresh install: minimal hint. No paragraph, no card — just a
    // single inline call to action so the admin knows where to start.
    return (
      <div style={{ maxWidth: 880, margin: '0 auto', padding: '40px 24px', textAlign: 'center' }}>
        <button
          type="button"
          onClick={onNewOrg}
          style={{
            background: 'none',
            border: 'none',
            padding: 0,
            fontSize: 14,
            color: 'var(--color-primary)',
            cursor: 'pointer',
            textDecoration: 'underline',
          }}
        >
          Create a new organisation
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
              <th style={{ ...thStyle, width: 80 }}></th>
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
                  <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end', alignItems: 'center' }}>
                    <IconButton
                      title="Manage users"
                      tone="primary"
                      onClick={() => onManageUsers(org)}
                    >
                      <UsersIcon size={14} />
                    </IconButton>
                    <IconButton
                      title="Delete organisation"
                      tone="danger"
                      onClick={() => onDelete(org)}
                    >
                      <TrashIcon size={14} />
                    </IconButton>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ─── Create-org page ─────────────────────────────────────────────────────────

/**
 * Single-step replacement for the old two-step wizard. Captures the
 * org's name, slug, and optional contact metadata in one form. The
 * first user is created afterward via the Add User affordance on the
 * org-users page — keeping user creation in one place.
 */
function CreateOrgPage({
  onCancel, onCreated,
}: {
  onCancel: () => void
  onCreated: (org: Org) => void
}) {
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [slugTouched, setSlugTouched] = useState(false)
  const [adminEmail, setAdminEmail] = useState('')
  const [adminName, setAdminName] = useState('')
  const [supportEmail, setSupportEmail] = useState('')
  const [description, setDescription] = useState('')

  const mut = useMutation({
    mutationFn: () => createOrg({
      name: name.trim(),
      slug: slug.trim(),
      admin_email: adminEmail.trim() || undefined,
      admin_name: adminName.trim() || undefined,
      support_email: supportEmail.trim() || undefined,
      description: description.trim() || undefined,
    }),
    onSuccess: onCreated,
  })

  const slugify = (v: string) => v.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')
  const canSubmit = name.trim().length > 0 && slug.trim().length > 0 && !mut.isPending

  return (
    <div style={{ maxWidth: 880, margin: '0 auto', padding: '0 24px' }}>
      <div style={{ marginBottom: 24 }}>
        <button
          onClick={onCancel}
          style={{
            fontSize: 12, color: 'var(--color-text-muted)',
            background: 'none', border: 'none', cursor: 'pointer',
            padding: 0, marginBottom: 12,
          }}
        >
          ← Back to organisations
        </button>
        <h2 style={{ fontSize: 16, fontWeight: 700, margin: 0 }}>Create organisation</h2>
        <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
          You can add users to this org after it's created.
        </div>
      </div>

      <div style={{ maxWidth: 520, display: 'flex', flexDirection: 'column', gap: 14 }}>
        <Field label="Name">
          <input
            type="text"
            autoFocus
            value={name}
            placeholder="e.g. Acme Corp"
            onChange={e => {
              setName(e.target.value)
              if (!slugTouched) setSlug(slugify(e.target.value))
            }}
            style={inputStyle}
          />
        </Field>

        <Field label="Slug" hint="Used in the API. Lowercase letters, digits, and dashes only.">
          <input
            type="text"
            value={slug}
            placeholder="acme"
            onChange={e => { setSlug(slugify(e.target.value)); setSlugTouched(true) }}
            style={inputStyle}
          />
        </Field>

        <Field label="Description" hint="Optional. Shown to admins viewing this organisation.">
          <textarea
            value={description}
            placeholder="What does this organisation do?"
            onChange={e => setDescription(e.target.value)}
            rows={2}
            style={{ ...inputStyle, resize: 'vertical', fontFamily: 'inherit' }}
          />
        </Field>

        <div style={{
          marginTop: 8, paddingTop: 12,
          borderTop: '1px solid var(--color-border)',
        }}>
          <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 10 }}>
            Contacts (optional)
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <Field label="Admin contact name">
              <input
                type="text"
                value={adminName}
                placeholder="e.g. Jane Doe"
                onChange={e => setAdminName(e.target.value)}
                style={inputStyle}
              />
            </Field>
            <Field label="Admin contact email">
              <input
                type="email"
                value={adminEmail}
                placeholder="admin@acme.example"
                onChange={e => setAdminEmail(e.target.value)}
                style={inputStyle}
              />
            </Field>
            <Field label="Support email" hint="Where this org's end-users send support requests.">
              <input
                type="email"
                value={supportEmail}
                placeholder="support@acme.example"
                onChange={e => setSupportEmail(e.target.value)}
                style={inputStyle}
              />
            </Field>
          </div>
        </div>

        {mut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 4 }}>
            {(mut.error as Error).message}
          </div>
        )}

        <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
          <button className="btn-ghost" onClick={onCancel} disabled={mut.isPending}>Cancel</button>
          <button
            className="btn-primary"
            disabled={!canSubmit}
            onClick={() => mut.mutate()}
          >
            {mut.isPending ? 'Creating…' : 'Create organisation'}
          </button>
        </div>
      </div>
    </div>
  )
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div>
      <label style={fieldLabelStyle}>{label}</label>
      {children}
      {hint && (
        <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
          {hint}
        </div>
      )}
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
  // Role picker for "Add user to org" / "Grant role" — include the
  // org's custom roles, not just built-ins. `listOrgRoles` returns
  // built-ins + custom roles in one call.
  const rolesQ = useQuery({
    queryKey: ['org-roles', org.id],
    queryFn: () => listOrgRoles(org.id),
  })
  const [showAdd, setShowAdd] = useState(false)
  const [resettingUser, setResettingUser] = useState<OrgUser | null>(null)

  const resetPwMut = useMutation({
    mutationFn: ({ userId, newPassword }: { userId: string; newPassword: string }) =>
      resetOrgUserPassword(org.id, userId, newPassword),
    onSuccess: () => setResettingUser(null),
  })

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
                  <th style={{ ...thStyle, width: 80 }}></th>
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
                    <td style={{ ...tdStyle, textAlign: 'right' }}>
                      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end', alignItems: 'center' }}>
                        {u.auth_provider === 'internal' && (
                          <IconButton
                            title="Reset password"
                            tone="primary"
                            onClick={() => { resetPwMut.reset(); setResettingUser(u) }}
                          >
                            <KeyIcon size={14} />
                          </IconButton>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

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
                {roles.map(role => {
                  const selected = selectedRoleIds.includes(role.id)
                  return (
                    <label
                      key={role.id}
                      style={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 70px 18px',
                        alignItems: 'center',
                        gap: 8,
                        padding: '5px 10px',
                        border: `1px solid ${selected ? 'var(--color-primary)' : 'var(--color-border)'}`,
                        borderRadius: 4,
                        cursor: 'pointer',
                        fontSize: 12,
                        background: selected
                          ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                          : 'transparent',
                      }}
                    >
                      <span style={{ fontWeight: 500 }}>{role.name}</span>
                      <span style={{
                        fontSize: 10,
                        color: 'var(--color-text-muted)',
                        textAlign: 'left',
                      }}>
                        {role.org_id === null ? 'built-in' : 'custom'}
                      </span>
                      <input
                        type="checkbox"
                        checked={selected}
                        onChange={() => toggleRole(role.id)}
                        style={{ justifySelf: 'end', margin: 0 }}
                      />
                    </label>
                  )
                })}
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

// ─── Delete org modal ────────────────────────────────────────────────────────

/**
 * Two-state modal:
 *   1. Pre-flight: fetches `OrgStats`. If any count > 0, lists what's
 *      blocking and disables the destructive button. If all zero,
 *      offers a single Delete confirmation.
 *   2. Mutation: PATCH-style runs `deleteOrg`, surfaces any server
 *      error inline (e.g. someone added a member between pre-flight
 *      and confirm — server still enforces).
 */
function DeleteOrgModal({
  org, onCancel, onDeleted,
}: {
  org: Org
  onCancel: () => void
  onDeleted: () => void
}) {
  const statsQ = useQuery({
    queryKey: ['org-stats', org.id],
    queryFn: () => fetchOrgStats(org.id),
  })

  const deleteMut = useMutation({
    mutationFn: () => deleteOrg(org.id),
    onSuccess: onDeleted,
  })

  const stats = statsQ.data
  const blockers = stats ? blockingItems(stats) : []
  const isEmpty = stats && blockers.length === 0

  return (
    <div
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
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
          width: 460,
          maxHeight: '90vh',
          overflowY: 'auto',
          boxShadow: 'var(--shadow-md)',
        }}
      >
        <h3 style={{ fontSize: 14, fontWeight: 600, margin: '0 0 4px' }}>
          Delete organisation
        </h3>
        <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '0 0 16px' }}>
          <span style={{ fontWeight: 500, color: 'var(--color-text)' }}>{org.name}</span>{' '}
          <code style={{ fontSize: 11 }}>{org.slug}</code>
        </p>

        {statsQ.isLoading && (
          <div style={{ padding: '12px 0' }}><div className="spinner" /></div>
        )}
        {statsQ.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
            Failed to load org stats.
          </div>
        )}

        {stats && isEmpty && (
          <div style={{
            fontSize: 13, color: 'var(--color-text)',
            background: 'var(--status-error-soft, rgba(200,40,40,0.08))',
            border: '1px solid var(--color-border)',
            borderRadius: 5, padding: 12, marginBottom: 16,
          }}>
            This organisation is empty and can be deleted. The action is
            permanent and cannot be undone.
          </div>
        )}

        {stats && !isEmpty && (
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: 13, marginBottom: 8 }}>
              Cannot delete — this org still contains:
            </div>
            <ul style={{ margin: 0, paddingLeft: 18, fontSize: 13, color: 'var(--color-text)' }}>
              {blockers.map(b => (
                <li key={b.label} style={{ marginBottom: 2 }}>
                  <span style={{ fontWeight: 500 }}>{b.count}</span>{' '}
                  <span style={{ color: 'var(--color-text-muted)' }}>{b.label}</span>
                </li>
              ))}
            </ul>
            <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 10 }}>
              Remove these first, then retry the delete.
            </p>
          </div>
        )}

        {deleteMut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
            {(deleteMut.error as Error).message}
          </div>
        )}

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button className="btn-ghost" onClick={onCancel}>Cancel</button>
          <button
            className="btn-ghost"
            style={{
              color: 'var(--status-error)',
              opacity: isEmpty && !deleteMut.isPending ? 1 : 0.5,
            }}
            disabled={!isEmpty || deleteMut.isPending}
            onClick={() => deleteMut.mutate()}
          >
            {deleteMut.isPending ? 'Deleting…' : 'Delete organisation'}
          </button>
        </div>
      </div>
    </div>
  )
}

function blockingItems(s: OrgStats): { count: number; label: string }[] {
  const out: { count: number; label: string }[] = []
  if (s.members   > 0) out.push({ count: s.members,   label: `member${pl(s.members)}` })
  if (s.processes > 0) out.push({ count: s.processes, label: `process definition${pl(s.processes)}` })
  if (s.decisions > 0) out.push({ count: s.decisions, label: `decision definition${pl(s.decisions)}` })
  if (s.instances > 0) out.push({ count: s.instances, label: `process instance${pl(s.instances)}` })
  return out
}

const pl = (n: number) => (n === 1 ? '' : 's')

// ─── Platform tabs ───────────────────────────────────────────────────────────

/**
 * Primary nav for PlatformShell. Buttons sit inside the same centered
 * container as the page content (max-width 880, 24px gutter) so the
 * tab strip and table headers visually align. The underline border
 * spans the full viewport width to keep the chrome continuous.
 */
function PlatformTabs({
  current, onSelect,
}: {
  current: Section
  onSelect: (s: Section) => void
}) {
  return (
    <nav
      style={{
        borderBottom: '1px solid var(--color-border)',
        background: 'var(--bg-primary)',
      }}
    >
      <div style={{
        maxWidth: 880,
        margin: '0 auto',
        padding: '8px 24px 0',
        display: 'flex',
        gap: 4,
      }}>
        <TabButton active={current === 'orgs'}     onClick={() => onSelect('orgs')}>Organisations</TabButton>
        <TabButton active={current === 'roles'}    onClick={() => onSelect('roles')}>Roles</TabButton>
        <TabButton active={current === 'settings'} onClick={() => onSelect('settings')}>Settings</TabButton>
      </div>
    </nav>
  )
}

function TabButton({
  active, onClick, children,
}: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        padding: '8px 16px',
        fontSize: 13,
        fontWeight: active ? 600 : 400,
        color: active ? 'var(--color-primary)' : 'var(--color-text-muted)',
        background: 'transparent',
        border: 'none',
        borderBottom: `2px solid ${active ? 'var(--color-primary)' : 'transparent'}`,
        marginBottom: -1,
        cursor: 'pointer',
        transition: 'color 0.15s, border-color 0.15s',
      }}
    >
      {children}
    </button>
  )
}

// ─── Platform roles ──────────────────────────────────────────────────────────

/**
 * Built-in role catalog as a read-only reference. Built-ins are
 * seeded by `migrations/021_roles.sql`; their permissions are part
 * of the platform's trust model and aren't editable via the API.
 *
 * Custom roles are still per-org and managed inside each org's own
 * admin console (currently platform-admin-only). That capability
 * isn't surfaced here yet — the priority is making the built-in
 * grants visible so operators can reason about what each role
 * actually permits.
 */
// Selection drives the cross-highlight: at most one side is "active" at
// a time. Clicking a role or permission again clears the selection.
type RoleSelection = { kind: 'role'; id: string }
type PermSelection = { kind: 'permission'; name: string }
type CatalogSelection = RoleSelection | PermSelection | null

function PlatformRoles() {
  const qc = useQueryClient()
  const builtinsQ = useQuery({ queryKey: ['builtin-roles'], queryFn: listBuiltinRoles })
  const orgsQ = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  // Org filter for the custom-roles section. `null` means "built-ins
  // only". When set, the left pane also shows that org's custom roles
  // and exposes create / edit / delete actions for them.
  const [orgId, setOrgId] = useState<string | null>(null)
  const orgRolesQ = useQuery({
    queryKey: ['org-roles', orgId],
    queryFn: () => listOrgRoles(orgId!),
    enabled: !!orgId,
  })

  const [selected, setSelected] = useState<CatalogSelection>(null)
  const [editing, setEditing] = useState<AdminRole | null>(null)
  const [creating, setCreating] = useState(false)
  const [deleting, setDeleting] = useState<AdminRole | null>(null)

  const createMut = useMutation({
    mutationFn: ({ org, name, perms }: { org: string; name: string; perms: string[] }) =>
      createOrgRole(org, name, perms),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setCreating(false)
    },
  })
  const updateMut = useMutation({
    mutationFn: ({ org, id, name, perms }: { org: string; id: string; name: string; perms: string[] }) =>
      updateOrgRole(org, id, name, perms),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setEditing(null)
    },
  })
  const deleteMut = useMutation({
    mutationFn: ({ org, id }: { org: string; id: string }) => removeOrgRole(org, id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setDeleting(null)
      // Drop the highlight if the deleted role was selected.
      setSelected(prev => (prev?.kind === 'role' ? null : prev))
    },
  })

  if (builtinsQ.isLoading || orgsQ.isLoading) {
    return <div style={{ textAlign: 'center', padding: 40 }}><div className="spinner" /></div>
  }
  if (builtinsQ.isError) {
    return <div style={{ color: 'var(--status-error)', padding: 24 }}>Failed to load roles.</div>
  }

  const builtins = builtinsQ.data ?? []
  const customRoles = orgRolesQ.data?.filter(r => r.org_id !== null) ?? []
  const orgs = orgsQ.data ?? []
  const selectedOrgName = orgs.find(o => o.id === orgId)?.name ?? null

  // Combined list drives both selection lookup and highlighting.
  const allRoles: AdminRole[] = [...builtins, ...customRoles]

  const selectedRole = selected?.kind === 'role'
    ? allRoles.find(r => r.id === selected.id)
    : null
  const selectedPerm = selected?.kind === 'permission' ? selected.name : null

  const permsForSelectedRole = new Set<string>(selectedRole?.permissions ?? [])
  const rolesForSelectedPerm = new Set<string>(
    selectedPerm
      ? allRoles.filter(r => r.permissions.includes(selectedPerm)).map(r => r.id)
      : [],
  )

  const toggleRole = (id: string) =>
    setSelected(prev =>
      prev?.kind === 'role' && prev.id === id ? null : { kind: 'role', id }
    )
  const togglePerm = (name: string) =>
    setSelected(prev =>
      prev?.kind === 'permission' && prev.name === name
        ? null
        : { kind: 'permission', name }
    )

  return (
    <div style={{ maxWidth: 880, margin: '0 auto', padding: '0 24px' }}>
      <div style={{
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        gap: 16,
        marginBottom: 16,
      }}>
        <div>
          <h2 style={{ fontSize: 16, fontWeight: 700, margin: 0 }}>Roles</h2>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
            Built-in roles ship with every Conduit instance. Custom roles
            are per-organisation — only platform admins can create or
            edit them.
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
          <label style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>
            Custom roles for
          </label>
          <select
            value={orgId ?? ''}
            onChange={e => {
              setOrgId(e.target.value || null)
              setSelected(null)
            }}
            style={{
              fontSize: 12,
              padding: '5px 8px',
              border: '1px solid var(--color-border)',
              borderRadius: 5,
              background: 'var(--bg-primary)',
              color: 'var(--color-text)',
              minWidth: 160,
            }}
          >
            <option value="">— no org —</option>
            {orgs.map(o => (
              <option key={o.id} value={o.id}>{o.name}</option>
            ))}
          </select>
          <button
            className="btn-primary"
            style={{ fontSize: 12, padding: '5px 12px' }}
            disabled={!orgId}
            onClick={() => { createMut.reset(); setCreating(true) }}
          >
            + New role
          </button>
        </div>
      </div>

      <div style={{
        display: 'grid',
        gridTemplateColumns: '1fr 1fr',
        gap: 16,
        alignItems: 'stretch',
      }}>
        <RolesPane
          builtins={builtins}
          customRoles={customRoles}
          customRolesLoading={!!orgId && orgRolesQ.isLoading}
          customRolesError={!!orgId && orgRolesQ.isError}
          customOrgName={selectedOrgName}
          selectedRoleId={selectedRole?.id ?? null}
          activeRoleIds={rolesForSelectedPerm}
          dimNonActive={selectedPerm !== null}
          onSelect={toggleRole}
          onEdit={r => { updateMut.reset(); setEditing(r) }}
          onDelete={r => { deleteMut.reset(); setDeleting(r) }}
        />
        <PermissionsPane
          selectedPerm={selectedPerm}
          activePerms={permsForSelectedRole}
          dimNonActive={selectedRole !== null}
          onSelect={togglePerm}
        />
      </div>

      {creating && orgId && (
        <RoleEditorModal
          mode="create"
          orgName={selectedOrgName ?? ''}
          initialName=""
          initialPerms={[]}
          pending={createMut.isPending}
          error={createMut.error as Error | null}
          onCancel={() => setCreating(false)}
          onSubmit={(name, perms) => createMut.mutate({ org: orgId, name, perms })}
        />
      )}

      {editing && orgId && (
        <RoleEditorModal
          mode="edit"
          orgName={selectedOrgName ?? ''}
          initialName={editing.name}
          initialPerms={editing.permissions}
          pending={updateMut.isPending}
          error={updateMut.error as Error | null}
          onCancel={() => setEditing(null)}
          onSubmit={(name, perms) =>
            updateMut.mutate({ org: orgId, id: editing.id, name, perms })
          }
        />
      )}

      {deleting && orgId && (
        <DeleteRoleModal
          role={deleting}
          pending={deleteMut.isPending}
          error={deleteMut.error as Error | null}
          onCancel={() => setDeleting(null)}
          onConfirm={() => deleteMut.mutate({ org: orgId, id: deleting.id })}
        />
      )}
    </div>
  )
}

function RolesPane({
  builtins, customRoles, customRolesLoading, customRolesError, customOrgName,
  selectedRoleId, activeRoleIds, dimNonActive, onSelect, onEdit, onDelete,
}: {
  builtins: AdminRole[]
  customRoles: AdminRole[]
  customRolesLoading: boolean
  customRolesError: boolean
  customOrgName: string | null
  selectedRoleId: string | null
  activeRoleIds: Set<string>
  dimNonActive: boolean
  onSelect: (id: string) => void
  onEdit: (r: AdminRole) => void
  onDelete: (r: AdminRole) => void
}) {
  const showCustomSection = customOrgName !== null
  const totalCount =
    builtins.length + (showCustomSection ? customRoles.length : 0)

  return (
    <div style={paneStyle}>
      <div style={paneHeaderStyle}>Roles ({totalCount})</div>
      <div style={paneBodyStyle}>
        <RoleGroupHeader label="Built-in" />
        {builtins.map((r, idx) => (
          <RoleRow
            key={r.id}
            role={r}
            subtitle={`built-in · ${r.permissions.length} permission${r.permissions.length === 1 ? '' : 's'}`}
            isSelected={selectedRoleId === r.id}
            isActive={activeRoleIds.has(r.id)}
            dim={dimNonActive && !activeRoleIds.has(r.id)}
            showActions={false}
            isLast={idx === builtins.length - 1}
            onSelect={onSelect}
            onEdit={onEdit}
            onDelete={onDelete}
          />
        ))}

        {showCustomSection && (
          <>
            <RoleGroupHeader label={`Custom · ${customOrgName}`} />
            {customRolesLoading && (
              <div style={{ padding: '14px', textAlign: 'center' }}>
                <div className="spinner" />
              </div>
            )}
            {customRolesError && (
              <div style={{ padding: '14px', fontSize: 12, color: 'var(--status-error)' }}>
                Failed to load custom roles.
              </div>
            )}
            {!customRolesLoading && !customRolesError && customRoles.length === 0 && (
              <div style={{ padding: '14px', fontSize: 12, color: 'var(--color-text-muted)' }}>
                No custom roles in this org yet. Use <strong>+ New role</strong> to add one.
              </div>
            )}
            {customRoles.map((r, idx) => (
              <RoleRow
                key={r.id}
                role={r}
                subtitle={`custom · ${r.permissions.length} permission${r.permissions.length === 1 ? '' : 's'}`}
                isSelected={selectedRoleId === r.id}
                isActive={activeRoleIds.has(r.id)}
                dim={dimNonActive && !activeRoleIds.has(r.id)}
                showActions
                isLast={idx === customRoles.length - 1}
                onSelect={onSelect}
                onEdit={onEdit}
                onDelete={onDelete}
              />
            ))}
          </>
        )}
      </div>
    </div>
  )
}

function RoleGroupHeader({ label }: { label: string }) {
  return (
    <div style={{
      fontSize: 10,
      fontWeight: 600,
      textTransform: 'uppercase',
      letterSpacing: 0.5,
      color: 'var(--color-text-muted)',
      padding: '8px 14px 6px',
      borderBottom: '1px solid var(--color-border)',
      background: 'var(--bg-secondary)',
    }}>
      {label}
    </div>
  )
}

function RoleRow({
  role, subtitle, isSelected, isActive, dim, showActions, isLast,
  onSelect, onEdit, onDelete,
}: {
  role: AdminRole
  subtitle: string
  isSelected: boolean
  isActive: boolean
  dim: boolean
  showActions: boolean
  isLast: boolean
  onSelect: (id: string) => void
  onEdit: (r: AdminRole) => void
  onDelete: (r: AdminRole) => void
}) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '10px 14px',
        background: isSelected
          ? 'color-mix(in srgb, var(--color-primary) 14%, transparent)'
          : 'transparent',
        borderBottom: !isLast ? '1px solid var(--color-border)' : 'none',
        opacity: dim ? 0.4 : 1,
        transition: 'opacity 0.12s, background 0.12s',
      }}
    >
      <button
        type="button"
        onClick={() => onSelect(role.id)}
        style={{
          flex: 1,
          textAlign: 'left',
          background: 'transparent',
          border: 'none',
          padding: 0,
          cursor: 'pointer',
          color: 'var(--color-text)',
          minWidth: 0,
        }}
      >
        <div style={{ fontSize: 13, fontWeight: 500 }}>{role.name}</div>
        <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2 }}>
          {subtitle}
        </div>
      </button>
      {dim ? null : !isSelected && isActive ? <CheckGlyph /> : null}
      {showActions && (
        <div style={{ display: 'flex', gap: 4, flexShrink: 0 }}>
          <IconButton
            title="Edit role"
            tone="primary"
            size={24}
            onClick={() => onEdit(role)}
          >
            <PencilIcon size={12} />
          </IconButton>
          <IconButton
            title="Delete role"
            tone="danger"
            size={24}
            onClick={() => onDelete(role)}
          >
            <TrashIcon size={12} />
          </IconButton>
        </div>
      )}
    </div>
  )
}

function RoleEditorModal({
  mode, orgName, initialName, initialPerms, pending, error, onCancel, onSubmit,
}: {
  mode: 'create' | 'edit'
  orgName: string
  initialName: string
  initialPerms: string[]
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (name: string, perms: string[]) => void
}) {
  const [name, setName] = useState(initialName)
  const [perms, setPerms] = useState<Set<string>>(new Set(initialPerms))

  const trimmed = name.trim()
  const canSubmit = trimmed.length > 0 && !pending

  const togglePerm = (p: string) =>
    setPerms(prev => {
      const next = new Set(prev)
      if (next.has(p)) next.delete(p)
      else next.add(p)
      return next
    })

  return (
    <ModalShell onCancel={onCancel} width={580}>
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>
        {mode === 'create' ? 'New custom role' : 'Edit custom role'}
      </h3>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
        {mode === 'create' ? 'Add a custom role to' : 'Modify the role in'}{' '}
        <strong>{orgName}</strong>. Built-in role names are reserved.
      </p>

      <div style={{ marginBottom: 16 }}>
        <label style={fieldLabelStyle}>Name</label>
        <input
          type="text"
          autoFocus
          value={name}
          onChange={e => setName(e.target.value)}
          placeholder="e.g. SupportEngineer"
          style={inputStyle}
        />
      </div>

      <div style={{ marginBottom: 16 }}>
        <label style={fieldLabelStyle}>
          Permissions ({perms.size} selected)
        </label>
        <div style={{
          border: '1px solid var(--color-border)',
          borderRadius: 5,
          maxHeight: 320,
          overflowY: 'auto',
          padding: 4,
        }}>
          {PERMISSION_DETAILS.map(p => {
            const checked = perms.has(p.name)
            return (
              <label
                key={p.name}
                style={{
                  display: 'grid',
                  gridTemplateColumns: '180px 18px 1fr 18px',
                  alignItems: 'center',
                  gap: 8,
                  padding: '4px 8px',
                  fontSize: 11,
                  cursor: 'pointer',
                  borderRadius: 3,
                  background: checked
                    ? 'color-mix(in srgb, var(--color-primary) 10%, transparent)'
                    : 'transparent',
                }}
              >
                <span style={{
                  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}>
                  {p.name}
                </span>
                <PermissionHelp description={p.description} />
                <span style={{
                  color: 'var(--color-text-muted)',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}>
                  {p.description}
                </span>
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => togglePerm(p.name)}
                  style={{ justifySelf: 'end', margin: 0 }}
                />
              </label>
            )
          })}
        </div>
      </div>

      {error && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
          {error.message}
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn-ghost" onClick={onCancel}>Cancel</button>
        <button
          className="btn-primary"
          disabled={!canSubmit}
          onClick={() => onSubmit(trimmed, Array.from(perms))}
        >
          {pending
            ? (mode === 'create' ? 'Creating…' : 'Saving…')
            : (mode === 'create' ? 'Create role' : 'Save')}
        </button>
      </div>
    </ModalShell>
  )
}

function DeleteRoleModal({
  role, pending, error, onCancel, onConfirm,
}: {
  role: AdminRole
  pending: boolean
  error: Error | null
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <ModalShell onCancel={onCancel} width={400}>
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 8 }}>Delete custom role</h3>
      <p style={{ fontSize: 13, marginBottom: 16 }}>
        Delete <strong>{role.name}</strong>? Any users currently holding this
        role will lose the permissions it grants. This cannot be undone.
      </p>

      {error && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
          {error.message}
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn-ghost" onClick={onCancel}>Cancel</button>
        <button
          className="btn-primary"
          disabled={pending}
          onClick={onConfirm}
          style={{ background: 'var(--color-error)', borderColor: 'var(--color-error)' }}
        >
          {pending ? 'Deleting…' : 'Delete role'}
        </button>
      </div>
    </ModalShell>
  )
}

function PermissionsPane({
  selectedPerm, activePerms, dimNonActive, onSelect,
}: {
  selectedPerm: string | null
  activePerms: Set<string>
  dimNonActive: boolean
  onSelect: (name: string) => void
}) {
  return (
    <div style={paneStyle}>
      <div style={paneHeaderStyle}>
        Permissions ({PERMISSION_DETAILS.length})
      </div>
      <div style={paneBodyStyle}>
        {PERMISSION_DETAILS.map((p, idx) => {
          const isSelected = selectedPerm === p.name
          const isActive = activePerms.has(p.name)
          const dim = dimNonActive && !isActive
          return (
            <div
              key={p.name}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                padding: '8px 14px',
                background: isSelected
                  ? 'color-mix(in srgb, var(--color-primary) 14%, transparent)'
                  : 'transparent',
                borderBottom: idx < PERMISSION_DETAILS.length - 1
                  ? '1px solid var(--color-border)'
                  : 'none',
                opacity: dim ? 0.4 : 1,
                transition: 'opacity 0.12s, background 0.12s',
              }}
            >
              <button
                type="button"
                onClick={() => onSelect(p.name)}
                style={{
                  flex: 1,
                  textAlign: 'left',
                  background: 'transparent',
                  border: 'none',
                  padding: 0,
                  cursor: 'pointer',
                  fontSize: 12,
                  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                  color: 'var(--color-text)',
                }}
              >
                {p.name}
              </button>
              <PermissionHelp description={p.description} />
              {dimNonActive && isActive && <CheckGlyph />}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function PermissionHelp({ description }: { description: string }) {
  return (
    <span
      title={description}
      aria-label={description}
      style={{
        width: 16,
        height: 16,
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        borderRadius: '50%',
        background: 'var(--color-surface-2)',
        color: 'var(--color-text-muted)',
        fontSize: 10,
        fontWeight: 600,
        cursor: 'help',
        flexShrink: 0,
        userSelect: 'none',
      }}
    >
      ?
    </span>
  )
}

function CheckGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path
        d="M3 7.5L6 10.5L11 4.5"
        stroke="var(--color-primary)"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

const paneStyle: React.CSSProperties = {
  border: '1px solid var(--color-border)',
  borderRadius: 6,
  background: 'var(--bg-secondary)',
  overflow: 'hidden',
  height: 'calc(100vh - 220px)',
  display: 'flex',
  flexDirection: 'column',
}

const paneHeaderStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  textTransform: 'uppercase',
  letterSpacing: 0.4,
  color: 'var(--color-text-muted)',
  padding: '10px 14px',
  background: 'var(--color-surface-2)',
  borderBottom: '1px solid var(--color-border)',
  flexShrink: 0,
}

const paneBodyStyle: React.CSSProperties = {
  overflowY: 'auto',
  flex: 1,
}

// ─── Platform settings ───────────────────────────────────────────────────────

/**
 * Placeholder shell for platform-wide configuration. Nothing is wired
 * yet — concrete settings (e.g. a default notification provider that
 * orgs inherit, an instance display name, support contact) will land
 * here as features call for them. Each row is a stub describing the
 * intended setting so the admin can see the planned shape.
 */
function PlatformSettings() {
  return (
    <div style={{ maxWidth: 880, margin: '0 auto', padding: '0 24px' }}>
      <h2 style={{ fontSize: 16, fontWeight: 700, margin: '0 0 4px' }}>Global settings</h2>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '0 0 24px' }}>
        Configuration that applies across every organisation in this
        Conduit instance. Org-level overrides live in each org's own
        admin → Settings page.
      </p>

      <PlatformAdminsSection />
    </div>
  )
}

// ─── Platform admins ─────────────────────────────────────────────────────────

function PlatformAdminsSection() {
  const qc = useQueryClient()
  const adminsQ = useQuery({
    queryKey: ['platform-admins'],
    queryFn: listPlatformAdmins,
  })

  const [showAdd, setShowAdd] = useState(false)
  const [editing, setEditing] = useState<PlatformAdmin | null>(null)
  const [resetting, setResetting] = useState<PlatformAdmin | null>(null)
  const [revoking, setRevoking] = useState<PlatformAdmin | null>(null)

  const createMut = useMutation({
    mutationFn: createPlatformAdmin,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['platform-admins'] })
      setShowAdd(false)
    },
  })

  const patchMut = useMutation({
    mutationFn: ({ userId, body }: { userId: string; body: { email?: string; name?: string } }) =>
      patchPlatformAdmin(userId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['platform-admins'] })
      setEditing(null)
    },
  })

  const resetMut = useMutation({
    mutationFn: ({ userId, newPassword }: { userId: string; newPassword: string }) =>
      resetGlobalUserPassword(userId, newPassword),
    onSuccess: () => setResetting(null),
  })

  const revokeMut = useMutation({
    mutationFn: (userId: string) => revokePlatformAdmin(userId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['platform-admins'] })
      setRevoking(null)
    },
  })

  const admins = adminsQ.data ?? []

  return (
    <section style={{ marginBottom: 32 }}>
      <div style={{
        display: 'flex', alignItems: 'center',
        justifyContent: 'space-between', marginBottom: 12,
      }}>
        <div>
          <h3 style={{ fontSize: 14, fontWeight: 600, margin: 0 }}>Platform admins</h3>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
            Users with the global <code>PlatformAdmin</code> role.
            They can manage every organisation on this instance.
          </div>
        </div>
        <button
          className="btn-primary"
          style={{ fontSize: 12, padding: '6px 14px' }}
          onClick={() => { createMut.reset(); setShowAdd(true) }}
        >
          + Add admin
        </button>
      </div>

      {adminsQ.isLoading ? (
        <div style={{ textAlign: 'center', padding: 40 }}><div className="spinner" /></div>
      ) : adminsQ.isError ? (
        <div style={{ color: 'var(--status-error)', fontSize: 13 }}>
          Failed to load platform admins.
        </div>
      ) : admins.length === 0 ? (
        <div style={{
          border: '1px dashed var(--color-border)', borderRadius: 6,
          padding: '20px 16px', textAlign: 'center',
          color: 'var(--color-text-muted)', fontSize: 13,
        }}>
          No platform admins yet.
        </div>
      ) : (
        <div style={{ border: '1px solid var(--color-border)', borderRadius: 6, overflow: 'hidden' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ background: 'var(--color-surface-2)', borderBottom: '1px solid var(--color-border)' }}>
                <th style={thStyle}>Email</th>
                <th style={thStyle}>Name</th>
                <th style={thStyle}>Provider</th>
                <th style={{ ...thStyle, width: 130 }}></th>
              </tr>
            </thead>
            <tbody>
              {admins.map((a, idx) => (
                <tr key={a.user_id} style={{
                  borderBottom: idx < admins.length - 1 ? '1px solid var(--color-border)' : 'none',
                }}>
                  <td style={tdStyle}><span style={{ fontSize: 13 }}>{a.email}</span></td>
                  <td style={tdStyle}>
                    {a.name ? (
                      <span style={{ fontSize: 13 }}>{a.name}</span>
                    ) : (
                      <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>—</span>
                    )}
                  </td>
                  <td style={tdStyle}>
                    <span style={{
                      fontSize: 11, padding: '2px 6px', borderRadius: 4,
                      background: 'var(--color-surface-2)', color: 'var(--color-text-muted)',
                    }}>
                      {a.auth_provider}
                    </span>
                  </td>
                  <td style={{ ...tdStyle, textAlign: 'right' }}>
                    <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                      <IconButton
                        title="Edit admin"
                        tone="primary"
                        onClick={() => { patchMut.reset(); setEditing(a) }}
                      >
                        <PencilIcon size={14} />
                      </IconButton>
                      {a.auth_provider === 'internal' && (
                        <IconButton
                          title="Reset password"
                          tone="primary"
                          onClick={() => { resetMut.reset(); setResetting(a) }}
                        >
                          <KeyIcon size={14} />
                        </IconButton>
                      )}
                      <IconButton
                        title={admins.length <= 1
                          ? 'Cannot revoke the last platform admin'
                          : 'Revoke platform admin'}
                        tone="danger"
                        disabled={admins.length <= 1}
                        onClick={() => { revokeMut.reset(); setRevoking(a) }}
                      >
                        <TrashIcon size={14} />
                      </IconButton>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {showAdd && (
        <AddPlatformAdminModal
          pending={createMut.isPending}
          error={createMut.error as Error | null}
          onCancel={() => setShowAdd(false)}
          onSubmit={body => createMut.mutate(body)}
        />
      )}

      {editing && (
        <EditPlatformAdminModal
          admin={editing}
          pending={patchMut.isPending}
          error={patchMut.error as Error | null}
          onCancel={() => setEditing(null)}
          onSubmit={body => patchMut.mutate({ userId: editing.user_id, body })}
        />
      )}

      {resetting && (
        <ResetPasswordModal
          email={resetting.email}
          pending={resetMut.isPending}
          error={resetMut.error as Error | null}
          onCancel={() => setResetting(null)}
          onSubmit={pw => resetMut.mutate({ userId: resetting.user_id, newPassword: pw })}
        />
      )}

      {revoking && (
        <RevokePlatformAdminModal
          admin={revoking}
          pending={revokeMut.isPending}
          error={revokeMut.error as Error | null}
          onCancel={() => setRevoking(null)}
          onConfirm={() => revokeMut.mutate(revoking.user_id)}
        />
      )}
    </section>
  )
}

function AddPlatformAdminModal({
  pending, error, onCancel, onSubmit,
}: {
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (body: {
    email: string
    auth_provider: 'internal' | 'external'
    password?: string
    external_id?: string
    name?: string
  }) => void
}) {
  const [email, setEmail] = useState('')
  const [name, setName] = useState('')
  const [provider, setProvider] = useState<'internal' | 'external'>('internal')
  const [password, setPassword] = useState('')
  const [externalId, setExternalId] = useState('')

  const canSubmit =
    email.trim().length > 0 &&
    !pending &&
    (provider === 'internal'
      ? password.length >= 8
      : externalId.trim().length > 0)

  const handleSubmit = () => {
    const trimmedName = name.trim()
    onSubmit({
      email: email.trim(),
      auth_provider: provider,
      password: provider === 'internal' ? password : undefined,
      external_id: provider === 'external' ? externalId.trim() : undefined,
      name: trimmedName.length > 0 ? trimmedName : undefined,
    })
  }

  return (
    <ModalShell onCancel={onCancel} width={420}>
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>Add platform admin</h3>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
        Creates a new user and grants them the global <code>PlatformAdmin</code> role.
      </p>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div>
          <label style={fieldLabelStyle}>Email</label>
          <input type="email" autoFocus value={email}
            onChange={e => setEmail(e.target.value)}
            placeholder="admin@example.com" style={inputStyle} />
        </div>

        <div>
          <label style={fieldLabelStyle}>Display name</label>
          <input type="text" value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Optional" style={inputStyle} />
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
              placeholder="At least 8 characters" style={inputStyle} />
          </div>
        ) : (
          <div>
            <label style={fieldLabelStyle}>External ID</label>
            <input type="text" value={externalId}
              onChange={e => setExternalId(e.target.value)}
              placeholder="Subject identifier from your IdP" style={inputStyle} />
          </div>
        )}
      </div>

      {error && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 12 }}>
          {error.message}
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
        <button className="btn-ghost" onClick={onCancel}>Cancel</button>
        <button className="btn-primary" disabled={!canSubmit} onClick={handleSubmit}>
          {pending ? 'Adding…' : 'Add admin'}
        </button>
      </div>
    </ModalShell>
  )
}

function EditPlatformAdminModal({
  admin, pending, error, onCancel, onSubmit,
}: {
  admin: PlatformAdmin
  pending: boolean
  error: Error | null
  onCancel: () => void
  onSubmit: (body: { email?: string; name?: string }) => void
}) {
  const [email, setEmail] = useState(admin.email)
  const [name, setName] = useState(admin.name ?? '')

  const dirtyEmail = email.trim() !== admin.email
  const dirtyName = name.trim() !== (admin.name ?? '')
  const emailValid = email.trim().length > 0
  const canSubmit = (dirtyEmail || dirtyName) && emailValid && !pending

  const handleSubmit = () => {
    const body: { email?: string; name?: string } = {}
    if (dirtyEmail) body.email = email.trim()
    if (dirtyName) body.name = name.trim()
    onSubmit(body)
  }

  return (
    <ModalShell onCancel={onCancel} width={400}>
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 16 }}>Edit platform admin</h3>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div>
          <label style={fieldLabelStyle}>Email</label>
          <input type="email" autoFocus value={email}
            onChange={e => setEmail(e.target.value)} style={inputStyle} />
        </div>
        <div>
          <label style={fieldLabelStyle}>Display name</label>
          <input type="text" value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Leave blank to clear" style={inputStyle} />
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
          {pending ? 'Saving…' : 'Save'}
        </button>
      </div>
    </ModalShell>
  )
}

function RevokePlatformAdminModal({
  admin, pending, error, onCancel, onConfirm,
}: {
  admin: PlatformAdmin
  pending: boolean
  error: Error | null
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <ModalShell onCancel={onCancel} width={400}>
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 8 }}>Revoke platform admin</h3>
      <p style={{ fontSize: 13, color: 'var(--color-text)', marginBottom: 16 }}>
        Remove the platform-admin role from{' '}
        <strong>{admin.email}</strong>? The user account itself will not be deleted —
        only the global PlatformAdmin grant is revoked.
      </p>

      {error && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 12 }}>
          {error.message}
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
        <button className="btn-ghost" onClick={onCancel}>Cancel</button>
        <button
          className="btn-primary"
          disabled={pending}
          onClick={onConfirm}
          style={{ background: 'var(--color-error)', borderColor: 'var(--color-error)' }}
        >
          {pending ? 'Revoking…' : 'Revoke'}
        </button>
      </div>
    </ModalShell>
  )
}

function ModalShell({
  width = 420, onCancel, children,
}: {
  width?: number
  onCancel: () => void
  children: React.ReactNode
}) {
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
        borderRadius: 8, padding: 24, width,
        maxHeight: '90vh', overflowY: 'auto',
        boxShadow: 'var(--shadow-md)',
      }}>
        {children}
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
