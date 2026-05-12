import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { fetchAdminOrg, patchAdminOrg } from '../../../api/admin'
import { useOrg } from '../../../App'

export default function GeneralSection() {
  const qc = useQueryClient()
  const { org: ctxOrg } = useOrg()
  const orgId = ctxOrg?.id
  const orgQ = useQuery({
    queryKey: ['admin-org', orgId],
    queryFn: () => fetchAdminOrg(orgId!),
    enabled: !!orgId,
  })

  // Local form state — initialised from the fetched org and kept in
  // sync via the effect below. Editing a field marks it dirty; only
  // dirty fields are sent in the PATCH body so unrelated saves don't
  // accidentally clobber other operators' edits.
  const [name, setName] = useState('')
  const [adminEmail, setAdminEmail] = useState('')
  const [adminName, setAdminName] = useState('')
  const [supportEmail, setSupportEmail] = useState('')
  const [description, setDescription] = useState('')

  useEffect(() => {
    if (orgQ.data) {
      setName(orgQ.data.name)
      setAdminEmail(orgQ.data.admin_email ?? '')
      setAdminName(orgQ.data.admin_name ?? '')
      setSupportEmail(orgQ.data.support_email ?? '')
      setDescription(orgQ.data.description ?? '')
    }
  }, [orgQ.data])

  const saveMut = useMutation({
    mutationFn: (body: Parameters<typeof patchAdminOrg>[1]) => patchAdminOrg(orgId!, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin-org', orgId] }),
  })

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (orgQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (orgQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load org.</div>

  const org = orgQ.data!

  // A field is dirty if its current value differs from the persisted
  // one. For nullable fields, blank means "clear to NULL" — that's
  // dirty whenever the persisted value isn't already null.
  const dirty = {
    name: name.trim() !== org.name,
    admin_email: adminEmail.trim() !== (org.admin_email ?? ''),
    admin_name: adminName.trim() !== (org.admin_name ?? ''),
    support_email: supportEmail.trim() !== (org.support_email ?? ''),
    description: description.trim() !== (org.description ?? ''),
  }
  const isDirty = Object.values(dirty).some(Boolean)
  const nameValid = name.trim().length > 0

  const saveName = () => {
    if (!dirty.name || !nameValid) return
    saveMut.mutate({ name: name.trim() })
  }

  const saveContacts = () => {
    const body: Parameters<typeof patchAdminOrg>[1] = {}
    // `null` clears the column server-side; non-empty strings are
    // trimmed and stored. The PATCH endpoint distinguishes "field
    // absent" from "explicit null" so we only ship dirty fields.
    if (dirty.admin_email)   body.admin_email   = adminEmail.trim() === '' ? null : adminEmail.trim()
    if (dirty.admin_name)    body.admin_name    = adminName.trim() === '' ? null : adminName.trim()
    if (dirty.support_email) body.support_email = supportEmail.trim() === '' ? null : supportEmail.trim()
    if (dirty.description)   body.description   = description.trim() === '' ? null : description.trim()
    if (Object.keys(body).length === 0) return
    saveMut.mutate(body)
  }

  return (
    <div style={{ maxWidth: 520 }}>
      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 20px' }}>Organisation</h2>

        <div style={{ marginBottom: 16 }}>
          <label style={labelStyle}>Name</label>
          <div style={{ display: 'flex', gap: 8 }}>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              style={{ ...inputStyle, flex: 1 }}
            />
            <button
              className="btn-primary"
              disabled={!dirty.name || !nameValid || saveMut.isPending}
              onClick={saveName}
              style={{ flexShrink: 0 }}
            >
              {saveMut.isPending ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>

        <div>
          <label style={labelStyle}>Slug</label>
          <div style={{
            padding: '7px 10px',
            fontSize: 13,
            border: '1px solid var(--color-border)',
            borderRadius: 5,
            background: 'var(--color-surface-2)',
            color: 'var(--color-text-muted)',
            fontFamily: 'monospace',
          }}>
            {org.slug}
          </div>
          <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
            The slug is used in API requests and cannot be changed.
          </div>
        </div>
      </section>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 4px' }}>Details</h2>
        <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
          Contact points and a short description. All fields are optional —
          clear a field and save to remove it.
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div>
            <label style={labelStyle}>Description</label>
            <textarea
              value={description}
              rows={2}
              placeholder="What does this organisation do?"
              onChange={e => setDescription(e.target.value)}
              style={{ ...inputStyle, resize: 'vertical', fontFamily: 'inherit' }}
            />
          </div>
          <div>
            <label style={labelStyle}>Admin contact name</label>
            <input
              type="text"
              value={adminName}
              placeholder="e.g. Jane Doe"
              onChange={e => setAdminName(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Admin contact email</label>
            <input
              type="email"
              value={adminEmail}
              placeholder="admin@acme.example"
              onChange={e => setAdminEmail(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Support email</label>
            <input
              type="email"
              value={supportEmail}
              placeholder="support@acme.example"
              onChange={e => setSupportEmail(e.target.value)}
              style={inputStyle}
            />
            <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
              Where this org's end-users send support requests.
            </div>
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 4 }}>
            <button
              className="btn-primary"
              disabled={
                !(dirty.admin_email || dirty.admin_name || dirty.support_email || dirty.description)
                || saveMut.isPending
              }
              onClick={saveContacts}
            >
              {saveMut.isPending ? 'Saving…' : 'Save details'}
            </button>
          </div>
        </div>

        {saveMut.isSuccess && !isDirty && (
          <div style={{ fontSize: 12, color: 'var(--status-success)', marginTop: 10 }}>
            Saved.
          </div>
        )}
        {saveMut.isError && (
          <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 10 }}>
            {(saveMut.error as Error).message}
          </div>
        )}
      </section>

      <section>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 4px', color: 'var(--status-error)' }}>Danger zone</h2>
        <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 16 }}>
          Destructive actions that cannot be undone.
        </div>
        <div style={{
          border: '1px solid var(--status-error)',
          borderRadius: 8,
          padding: '16px 20px',
          opacity: 0.6,
        }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 500 }}>Delete organization</div>
              <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
                Permanently delete this org and all data. Not available in this release.
              </div>
            </div>
            <button className="btn-ghost" disabled style={{ fontSize: 12, color: 'var(--status-error)', padding: '5px 12px' }}>
              Delete org
            </button>
          </div>
        </div>
      </section>
    </div>
  )
}

const labelStyle: React.CSSProperties = {
  display: 'block',
  fontSize: 12,
  fontWeight: 500,
  marginBottom: 5,
  color: 'var(--color-text)',
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '7px 10px',
  fontSize: 13,
  border: '1px solid var(--color-border)',
  borderRadius: 5,
  background: 'var(--bg-primary)',
  color: 'var(--color-text)',
  boxSizing: 'border-box',
}
