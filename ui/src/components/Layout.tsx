import { NavLink, Outlet } from 'react-router-dom'
import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchOrgs, createOrg } from '../api/orgs'
import { useOrg } from '../App'

export default function Layout() {
  const { org, setOrg } = useOrg()
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [newName, setNewName] = useState('')
  const [newSlug, setNewSlug] = useState('')

  const { data: orgs } = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  const createMut = useMutation({
    mutationFn: createOrg,
    onSuccess: (created) => {
      qc.invalidateQueries({ queryKey: ['orgs'] })
      setOrg(created)
      setShowCreate(false)
      setNewName('')
      setNewSlug('')
    },
  })

  const navStyle = ({ isActive }: { isActive: boolean }): React.CSSProperties => ({
    display: 'block',
    padding: '8px 16px',
    color: isActive ? 'var(--color-primary)' : 'var(--color-text-muted)',
    background: isActive ? 'var(--color-surface-2)' : 'transparent',
    fontSize: 14,
    borderLeft: isActive ? '2px solid var(--color-primary)' : '2px solid transparent',
  })

  return (
    <div style={{ display: 'flex', height: '100vh', overflow: 'hidden' }}>
      <nav style={{
        width: 220,
        background: 'var(--color-surface)',
        borderRight: '1px solid var(--color-border)',
        display: 'flex',
        flexDirection: 'column',
        flexShrink: 0,
      }}>
        <div style={{ padding: '16px', borderBottom: '1px solid var(--color-border)' }}>
          <div style={{ fontSize: 15, fontWeight: 700, marginBottom: 12, color: 'var(--color-text)' }}>
            Conduit
          </div>

          {orgs && orgs.length > 0 ? (
            <select
              value={org?.id ?? ''}
              onChange={e => setOrg(orgs.find(o => o.id === e.target.value) ?? null)}
            >
              <option value="" disabled>Select org…</option>
              {orgs.map(o => <option key={o.id} value={o.id}>{o.name}</option>)}
            </select>
          ) : (
            <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 8 }}>
              No organisations yet
            </div>
          )}

          <button
            className="btn-ghost"
            style={{ marginTop: 8, width: '100%', fontSize: 12 }}
            onClick={() => setShowCreate(true)}
          >
            + New org
          </button>
        </div>

        <div style={{ flex: 1, paddingTop: 8 }}>
          {[
            { to: '/definitions', label: 'Definitions' },
            { to: '/instances',   label: 'Instances'   },
            { to: '/tasks',       label: 'Tasks'       },
          ].map(({ to, label }) => (
            <NavLink key={to} to={to} style={navStyle}>{label}</NavLink>
          ))}
        </div>
      </nav>

      <main style={{ flex: 1, overflow: 'auto', padding: 24 }}>
        {!org ? (
          <div className="empty-state">
            <p style={{ marginBottom: 8 }}>Select or create an organisation to get started.</p>
          </div>
        ) : (
          <Outlet />
        )}
      </main>

      {showCreate && (
        <div className="modal-overlay" onClick={() => setShowCreate(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Create organisation</h3>
            <div className="field">
              <label>Name</label>
              <input
                value={newName}
                autoFocus
                onChange={e => {
                  setNewName(e.target.value)
                  setNewSlug(e.target.value.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, ''))
                }}
              />
            </div>
            <div className="field">
              <label>Slug</label>
              <input value={newSlug} onChange={e => setNewSlug(e.target.value)} />
            </div>
            {createMut.error && (
              <div className="error-banner">{String(createMut.error)}</div>
            )}
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setShowCreate(false)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={!newName || !newSlug || createMut.isPending}
                onClick={() => createMut.mutate({ name: newName, slug: newSlug })}
              >
                {createMut.isPending ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
