import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { fetchAdminOrg, patchAdminOrg } from '../../api/admin'
import { useOrg } from '../../App'

export default function AdminSettings() {
  const qc = useQueryClient()
  const { org: ctxOrg } = useOrg()
  const orgId = ctxOrg?.id
  const orgQ = useQuery({
    queryKey: ['admin-org', orgId],
    queryFn: () => fetchAdminOrg(orgId!),
    enabled: !!orgId,
  })

  const [name, setName] = useState('')

  useEffect(() => {
    if (orgQ.data) setName(orgQ.data.name)
  }, [orgQ.data])

  const renameMut = useMutation({
    mutationFn: () => patchAdminOrg(orgId!, { name: name.trim() }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin-org', orgId] }),
  })

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (orgQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (orgQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load org.</div>

  const org = orgQ.data!
  const nameChanged = name.trim() !== org.name && name.trim().length > 0

  return (
    <div style={{ maxWidth: 480 }}>
      <section style={{ marginBottom: 40 }}>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 20px' }}>Organization</h2>

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
              disabled={!nameChanged || renameMut.isPending}
              onClick={() => renameMut.mutate()}
              style={{ flexShrink: 0 }}
            >
              {renameMut.isPending ? 'Saving…' : 'Save'}
            </button>
          </div>
          {renameMut.isSuccess && (
            <div style={{ fontSize: 12, color: 'var(--status-success)', marginTop: 6 }}>Name updated.</div>
          )}
          {renameMut.isError && (
            <div style={{ fontSize: 12, color: 'var(--status-error)', marginTop: 6 }}>
              {(renameMut.error as Error).message}
            </div>
          )}
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
