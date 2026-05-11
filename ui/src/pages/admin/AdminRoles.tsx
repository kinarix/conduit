import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listBuiltinRoles, listOrgRoles, createOrgRole, updateOrgRole, removeOrgRole,
  type AdminRole,
} from '../../api/admin'
import { useOrg } from '../../App'

const PERMISSION_DETAILS: { name: string; description: string }[] = [
  { name: 'org.create',              description: 'Create new organisations (global-only).' },
  { name: 'org.read',                description: 'View organisation details.' },
  { name: 'org.update',              description: 'Rename the organisation, change settings.' },
  { name: 'org.delete',              description: 'Delete the organisation and all its data.' },
  { name: 'org_member.create',       description: 'Add users as members of the organisation.' },
  { name: 'org_member.read',         description: 'View the org member list.' },
  { name: 'org_member.delete',       description: 'Remove users from the organisation.' },
  { name: 'user.create',             description: 'Create new global user identities.' },
  { name: 'user.read',               description: 'View user identities and metadata.' },
  { name: 'user.update',             description: 'Update user details (email, password).' },
  { name: 'user.delete',             description: 'Delete user identities globally.' },
  { name: 'role.create',             description: 'Create custom role definitions.' },
  { name: 'role.read',               description: 'View role definitions.' },
  { name: 'role.update',             description: 'Update custom role definitions.' },
  { name: 'role.delete',             description: 'Delete custom role definitions.' },
  { name: 'role_assignment.create',  description: 'Grant roles to users.' },
  { name: 'role_assignment.read',    description: 'View role grants (audit).' },
  { name: 'role_assignment.delete',  description: 'Revoke role grants.' },
  { name: 'auth_config.read',        description: 'View authentication settings.' },
  { name: 'auth_config.update',      description: 'Configure auth providers (OIDC, etc.).' },
  { name: 'process.create',          description: 'Create process definitions.' },
  { name: 'process.read',            description: 'View process definitions.' },
  { name: 'process.update',          description: 'Edit process definitions and drafts.' },
  { name: 'process.delete',          description: 'Delete process definition versions.' },
  { name: 'process.deploy',          description: 'Promote drafts to production.' },
  { name: 'process.disable',         description: 'Disable/enable specific versions.' },
  { name: 'process_group.create',    description: 'Create process groups.' },
  { name: 'process_group.read',      description: 'View process groups.' },
  { name: 'process_group.update',    description: 'Rename process groups.' },
  { name: 'process_group.delete',    description: 'Delete process groups.' },
  { name: 'instance.read',           description: 'View instances and their state.' },
  { name: 'instance.start',          description: 'Start new process instances.' },
  { name: 'instance.cancel',         description: 'Cancel running instances.' },
  { name: 'instance.pause',          description: 'Pause running instances.' },
  { name: 'instance.resume',         description: 'Resume suspended instances.' },
  { name: 'instance.delete',         description: 'Delete instances and their history.' },
  { name: 'task.read',               description: 'View user tasks.' },
  { name: 'task.complete',           description: 'Complete user tasks.' },
  { name: 'task.update',             description: 'Claim or reassign tasks.' },
  { name: 'external_task.execute',   description: 'Workers: fetch, complete, fail, extend.' },
  { name: 'decision.create',         description: 'Create decision (DMN) definitions.' },
  { name: 'decision.read',           description: 'View decision definitions.' },
  { name: 'decision.update',         description: 'Edit decision tables.' },
  { name: 'decision.delete',         description: 'Delete decision definitions.' },
  { name: 'decision.deploy',         description: 'Deploy DMN versions.' },
  { name: 'secret.create',           description: 'Create encrypted secrets.' },
  { name: 'secret.read_metadata',    description: 'View secret names and timestamps.' },
  { name: 'secret.read_plaintext',   description: 'Read the actual secret value.' },
  { name: 'secret.update',           description: 'Update secret values.' },
  { name: 'secret.delete',           description: 'Delete secrets.' },
  { name: 'api_key.manage',          description: 'Admin: create/list/revoke API keys.' },
  { name: 'process_layout.read',     description: 'View modeller layout data.' },
  { name: 'process_layout.update',   description: 'Save modeller layout data.' },
  { name: 'message.correlate',       description: 'Send messages to running instances.' },
  { name: 'signal.broadcast',        description: 'Broadcast signals across instances.' },
]

const PERMISSIONS = PERMISSION_DETAILS.map(p => p.name)

export default function AdminRoles() {
  const qc = useQueryClient()
  const { org } = useOrg()
  const orgId = org?.id

  const builtinQ = useQuery({
    queryKey: ['builtin-roles'],
    queryFn: listBuiltinRoles,
  })
  const customQ = useQuery({
    queryKey: ['org-roles', orgId],
    queryFn: () => listOrgRoles(orgId!),
    enabled: !!orgId,
  })
  const rolesQ = {
    data: useMemo(
      () => [...(builtinQ.data ?? []), ...(customQ.data ?? [])],
      [builtinQ.data, customQ.data],
    ),
    isLoading: builtinQ.isLoading || customQ.isLoading,
    isError: builtinQ.isError || customQ.isError,
  }

  const [showCreate, setShowCreate] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [removingId, setRemovingId] = useState<string | null>(null)
  const [selectedPerm, setSelectedPerm] = useState<string | null>(null)
  const [panelOpen, setPanelOpen] = useState(false)

  const createMut = useMutation({
    mutationFn: ({ name, permissions }: { name: string; permissions: string[] }) =>
      createOrgRole(orgId!, name, permissions),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setShowCreate(false)
    },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, name, permissions }: { id: string; name: string; permissions: string[] }) =>
      updateOrgRole(orgId!, id, name, permissions),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setEditingId(null)
    },
  })

  const removeMut = useMutation({
    mutationFn: (id: string) => removeOrgRole(orgId!, id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['org-roles', orgId] })
      setRemovingId(null)
    },
  })

  // Clicking a chip opens the panel if closed, and toggles the selection.
  const onChipClick = (perm: string) => {
    setPanelOpen(true)
    setSelectedPerm(prev => (prev === perm && panelOpen ? null : perm))
  }

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (rolesQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (rolesQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load roles.</div>

  const roles = rolesQ.data
  const builtIn = roles.filter(r => r.org_id === null)
  const custom = roles.filter(r => r.org_id !== null)

  return (
    <div>
      <div>
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 12 }}>
          <button
            type="button"
            className="btn-ghost"
            onClick={() => setPanelOpen(true)}
            style={{
              fontSize: 12,
              padding: '5px 10px',
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              visibility: panelOpen ? 'hidden' : 'visible',
            }}
            aria-hidden={panelOpen}
            tabIndex={panelOpen ? -1 : 0}
            aria-label="Open permissions help"
            title="Permissions help"
          >
            <HelpIcon />
            Permissions help
          </button>
        </div>

        <section style={{ marginBottom: 32 }}>
          <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 16px' }}>Built-in roles</h2>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 12 }}>
            {builtIn.map(role => (
              <RoleCard
                key={role.id}
                role={role}
                selectedPerm={panelOpen ? selectedPerm : null}
                onChipClick={onChipClick}
              />
            ))}
          </div>
        </section>

        <section>
          <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0 }}>Custom roles</h2>
            {!showCreate && (
              <button className="btn-primary" style={{ fontSize: 12, padding: '5px 12px' }} onClick={() => setShowCreate(true)}>
                New role
              </button>
            )}
          </div>

          {showCreate && (
            <RoleForm
              title="Create custom role"
              initialName=""
              initialPerms={[]}
              submitLabel="Create role"
              submittingLabel="Creating…"
              pending={createMut.isPending}
              error={createMut.error as Error | null}
              onSubmit={(name, permissions) => createMut.mutate({ name, permissions })}
              onCancel={() => { setShowCreate(false); createMut.reset() }}
              style={{ marginBottom: 16 }}
            />
          )}

          {custom.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', fontSize: 13, color: 'var(--color-text-muted)', border: '1px solid var(--color-border)', borderRadius: 6 }}>
              No custom roles yet.
            </div>
          ) : (
            <div style={{ border: '1px solid var(--color-border)', borderRadius: 6, overflow: 'hidden' }}>
              {custom.map((role, idx) => {
                const isEditing = editingId === role.id
                const isRemoving = removingId === role.id
                const borderBottom = idx < custom.length - 1 ? '1px solid var(--color-border)' : 'none'

                if (isEditing) {
                  return (
                    <div key={role.id} style={{ borderBottom, padding: 12 }}>
                      <RoleForm
                        title={`Edit "${role.name}"`}
                        initialName={role.name}
                        initialPerms={role.permissions}
                        submitLabel="Save"
                        submittingLabel="Saving…"
                        pending={updateMut.isPending}
                        error={updateMut.error as Error | null}
                        onSubmit={(name, permissions) =>
                          updateMut.mutate({ id: role.id, name, permissions })
                        }
                        onCancel={() => { setEditingId(null); updateMut.reset() }}
                      />
                    </div>
                  )
                }

                return (
                  <div
                    key={role.id}
                    style={{
                      padding: '12px 16px',
                      borderBottom,
                      background: isRemoving ? 'var(--status-error-soft)' : 'transparent',
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: 12,
                    }}
                  >
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>{role.name}</div>
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                        {role.permissions.map(p => (
                          <PermChip
                            key={p}
                            perm={p}
                            selected={panelOpen && selectedPerm === p}
                            onClick={() => onChipClick(p)}
                          />
                        ))}
                      </div>
                    </div>
                    <div style={{ flexShrink: 0 }}>
                      {isRemoving ? (
                        <div style={{ display: 'flex', gap: 6 }}>
                          <button
                            className="btn-ghost"
                            style={{ fontSize: 12, padding: '3px 8px', color: 'var(--status-error)' }}
                            disabled={removeMut.isPending}
                            onClick={() => removeMut.mutate(role.id)}
                          >
                            {removeMut.isPending ? 'Deleting…' : 'Confirm delete'}
                          </button>
                          <button className="btn-ghost" style={{ fontSize: 12, padding: '3px 8px' }} onClick={() => setRemovingId(null)}>
                            Cancel
                          </button>
                        </div>
                      ) : (
                        <div style={{ display: 'flex', gap: 4 }}>
                          <button
                            className="btn-ghost"
                            style={{ fontSize: 12, padding: '3px 8px' }}
                            onClick={() => { setEditingId(role.id); updateMut.reset() }}
                          >
                            Edit
                          </button>
                          <button
                            className="btn-ghost"
                            style={{ fontSize: 12, padding: '3px 8px', color: 'var(--status-error)' }}
                            onClick={() => setRemovingId(role.id)}
                          >
                            Delete
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </section>
      </div>

      {panelOpen && (
        <PermissionsPanel
          selectedPerm={selectedPerm}
          onSelect={setSelectedPerm}
          onClose={() => { setPanelOpen(false); setSelectedPerm(null) }}
        />
      )}
    </div>
  )
}

function RoleForm({
  title,
  initialName,
  initialPerms,
  submitLabel,
  submittingLabel,
  pending,
  error,
  onSubmit,
  onCancel,
  style,
}: {
  title: string
  initialName: string
  initialPerms: string[]
  submitLabel: string
  submittingLabel: string
  pending: boolean
  error: Error | null
  onSubmit: (name: string, permissions: string[]) => void
  onCancel: () => void
  style?: React.CSSProperties
}) {
  const [name, setName] = useState(initialName)
  const [perms, setPerms] = useState<string[]>(initialPerms)

  const togglePerm = (p: string) =>
    setPerms(prev => prev.includes(p) ? prev.filter(x => x !== p) : [...prev, p])

  const canSubmit = name.trim().length > 0 && perms.length > 0 && !pending

  return (
    <div style={{
      border: '1px solid var(--color-border)',
      borderRadius: 8,
      padding: 20,
      background: 'var(--bg-secondary)',
      ...style,
    }}>
      <h3 style={{ fontSize: 13, fontWeight: 600, margin: '0 0 12px' }}>{title}</h3>
      <input
        type="text"
        placeholder="Role name"
        value={name}
        onChange={e => setName(e.target.value)}
        autoFocus
        style={{
          width: '100%',
          padding: '6px 10px',
          fontSize: 13,
          border: '1px solid var(--color-border)',
          borderRadius: 5,
          background: 'var(--bg-primary)',
          color: 'var(--color-text)',
          marginBottom: 12,
          boxSizing: 'border-box',
        }}
      />
      <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--color-text-muted)', marginBottom: 8, textTransform: 'uppercase', letterSpacing: '0.04em' }}>
        Permissions
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 4, marginBottom: 16 }}>
        {PERMISSIONS.map(p => (
          <label key={p} style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, cursor: 'pointer', padding: '3px 0' }}>
            <input
              type="checkbox"
              checked={perms.includes(p)}
              onChange={() => togglePerm(p)}
            />
            {p}
          </label>
        ))}
      </div>
      {error && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 8 }}>
          {error.message}
        </div>
      )}
      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn-ghost" onClick={onCancel}>Cancel</button>
        <button
          className="btn-primary"
          disabled={!canSubmit}
          onClick={() => onSubmit(name.trim(), perms)}
        >
          {pending ? submittingLabel : submitLabel}
        </button>
      </div>
    </div>
  )
}

function RoleCard({
  role,
  selectedPerm,
  onChipClick,
}: {
  role: AdminRole
  selectedPerm: string | null
  onChipClick: (perm: string) => void
}) {
  return (
    <div style={{
      border: '1px solid var(--color-border)',
      borderRadius: 8,
      padding: '14px 16px',
      background: 'var(--bg-secondary)',
    }}>
      <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{role.name}</div>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
        {role.permissions.length === 0
          ? <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>No permissions</span>
          : role.permissions.map(p => (
              <PermChip key={p} perm={p} selected={selectedPerm === p} onClick={() => onChipClick(p)} />
            ))
        }
      </div>
    </div>
  )
}

function PermChip({
  perm,
  selected,
  onClick,
}: {
  perm: string
  selected?: boolean
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        fontSize: 10,
        padding: '2px 5px',
        borderRadius: 3,
        border: '1px solid transparent',
        background: selected
          ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 14%, transparent))'
          : 'var(--color-surface-2)',
        color: selected ? 'var(--color-primary)' : 'var(--color-text-muted)',
        borderColor: selected ? 'var(--color-primary)' : 'transparent',
        fontFamily: 'monospace',
        cursor: 'pointer',
        transition: 'background 0.12s, color 0.12s, border-color 0.12s',
      }}
    >
      {perm}
    </button>
  )
}

function PermissionsPanel({
  selectedPerm,
  onSelect,
  onClose,
}: {
  selectedPerm: string | null
  onSelect: (perm: string | null) => void
  onClose: () => void
}) {
  return (
    <aside style={{
      position: 'fixed',
      right: 16,
      top: 24,
      bottom: 16,
      width: 320,
      border: '1px solid var(--color-border)',
      borderRadius: 8,
      background: 'var(--bg-secondary)',
      boxShadow: '0 4px 16px rgba(0, 0, 0, 0.08)',
      overflowY: 'auto',
      zIndex: 30,
      display: 'flex',
      flexDirection: 'column',
    }}>
      <div style={{
        padding: '12px 14px 10px',
        borderBottom: '1px solid var(--color-border)',
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        gap: 8,
        position: 'sticky',
        top: 0,
        background: 'var(--bg-secondary)',
        zIndex: 1,
      }}>
        <div style={{ minWidth: 0 }}>
          <h3 style={{
            fontSize: 11,
            fontWeight: 600,
            textTransform: 'uppercase',
            letterSpacing: '0.06em',
            color: 'var(--color-text-muted)',
            margin: 0,
          }}>
            Permissions
          </h3>
          <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
            Click a permission chip to highlight it here.
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close permissions help"
          title="Close"
          style={{
            background: 'transparent',
            border: 'none',
            color: 'var(--color-text-muted)',
            cursor: 'pointer',
            padding: 2,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 4,
            flexShrink: 0,
          }}
          onMouseEnter={e => { e.currentTarget.style.background = 'var(--color-surface-2)' }}
          onMouseLeave={e => { e.currentTarget.style.background = 'transparent' }}
        >
          <CloseIcon />
        </button>
      </div>
      <div>
        {PERMISSION_DETAILS.map(({ name, description }) => {
          const isSelected = selectedPerm === name
          return (
            <button
              key={name}
              type="button"
              onClick={() => onSelect(isSelected ? null : name)}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                padding: '10px 16px',
                border: 'none',
                borderLeft: `3px solid ${isSelected ? 'var(--color-primary)' : 'transparent'}`,
                background: isSelected
                  ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                  : 'transparent',
                cursor: 'pointer',
                transition: 'background 0.12s, border-color 0.12s',
              }}
            >
              <div style={{
                fontSize: 11,
                fontFamily: 'monospace',
                fontWeight: 600,
                color: isSelected ? 'var(--color-primary)' : 'var(--color-text)',
                marginBottom: 3,
              }}>
                {name}
              </div>
              <div style={{ fontSize: 11, color: 'var(--color-text-muted)', lineHeight: 1.45 }}>
                {description}
              </div>
            </button>
          )
        })}
      </div>
    </aside>
  )
}

function HelpIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="8" r="6.5" />
      <path d="M6.3 6c.1-.9.9-1.5 1.8-1.5 1 0 1.8.7 1.8 1.6 0 .9-.7 1.3-1.3 1.6-.4.2-.6.5-.6.9v.4" />
      <circle cx="8" cy="11.5" r="0.4" fill="currentColor" stroke="none" />
    </svg>
  )
}

function CloseIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
      <path d="M4 4l8 8M12 4l-8 8" />
    </svg>
  )
}
