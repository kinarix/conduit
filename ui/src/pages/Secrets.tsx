import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useOrg } from '../App'
import {
  createSecret,
  deleteSecret,
  fetchSecrets,
  type SecretMetadata,
} from '../api/secrets'

export default function Secrets() {
  const { org } = useOrg()
  const qc = useQueryClient()
  const [adding, setAdding] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState<SecretMetadata | null>(null)

  const { data: secrets = [], isLoading } = useQuery({
    queryKey: ['secrets', org?.id],
    queryFn: () => fetchSecrets(org!.id),
    enabled: !!org,
  })

  const del = useMutation({
    mutationFn: (name: string) => deleteSecret(org!.id, name),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['secrets', org?.id] })
      setConfirmDelete(null)
    },
  })

  if (!org) {
    return (
      <div className="empty-state">
        <p>Select an organisation to manage secrets.</p>
      </div>
    )
  }
  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div style={{ padding: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <div>
          <h1 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Secrets</h1>
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: '4px 0 0' }}>
            Encrypted credentials referenced by name from HTTP service tasks. Values are
            never returned by the API once stored — to rotate, delete and recreate.
          </p>
        </div>
        <button className="btn-primary" onClick={() => setAdding(true)}>
          Add secret
        </button>
      </div>

      {secrets.length === 0 ? (
        <div className="empty-state">
          <p>No secrets yet.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Created</th>
              <th>Updated</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {secrets.map(s => (
              <tr key={s.id}>
                <td style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }}>{s.name}</td>
                <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                  {new Date(s.created_at).toLocaleString()}
                </td>
                <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                  {new Date(s.updated_at).toLocaleString()}
                </td>
                <td>
                  <button className="btn-ghost" onClick={() => setConfirmDelete(s)}>
                    Delete
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {adding && (
        <AddSecretModal
          orgId={org.id}
          existingNames={secrets.map(s => s.name)}
          onClose={() => setAdding(false)}
        />
      )}

      {confirmDelete && (
        <ConfirmDeleteModal
          name={confirmDelete.name}
          pending={del.isPending}
          error={del.error instanceof Error ? del.error.message : null}
          onCancel={() => setConfirmDelete(null)}
          onConfirm={() => del.mutate(confirmDelete.name)}
        />
      )}
    </div>
  )
}

function AddSecretModal({
  orgId,
  existingNames,
  onClose,
}: {
  orgId: string
  existingNames: string[]
  onClose: () => void
}) {
  const qc = useQueryClient()
  const [name, setName] = useState('')
  const [value, setValue] = useState('')

  const create = useMutation({
    mutationFn: () => createSecret(orgId, { name: name.trim(), value }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['secrets', orgId] })
      onClose()
    },
  })

  const trimmed = name.trim()
  const duplicate = !!trimmed && existingNames.includes(trimmed)
  const valid = trimmed.length > 0 && value.length > 0 && !duplicate

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <h2 style={{ fontSize: 16, fontWeight: 600, margin: 0, marginBottom: 16 }}>
          Add secret
        </h2>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <label>
            <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 4 }}>
              Name
            </div>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="e.g. stripe_key"
              style={{ width: '100%' }}
              autoFocus
            />
            {duplicate && (
              <div style={{ fontSize: 11, color: 'var(--danger)', marginTop: 4 }}>
                A secret with that name already exists in this org.
              </div>
            )}
          </label>
          <label>
            <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 4 }}>
              Value
            </div>
            <input
              type="password"
              value={value}
              onChange={e => setValue(e.target.value)}
              placeholder="(stored encrypted)"
              style={{ width: '100%', fontFamily: 'var(--font-mono)' }}
            />
            <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginTop: 4 }}>
              For Basic auth, use the form <code>username:password</code>.
            </div>
          </label>
          {create.error instanceof Error && (
            <div style={{ fontSize: 12, color: 'var(--danger)' }}>
              {create.error.message}
            </div>
          )}
        </div>
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 20 }}>
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button
            className="btn-primary"
            disabled={!valid || create.isPending}
            onClick={() => create.mutate()}
          >
            {create.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}

function ConfirmDeleteModal({
  name,
  pending,
  error,
  onCancel,
  onConfirm,
}: {
  name: string
  pending: boolean
  error: string | null
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <h2 style={{ fontSize: 16, fontWeight: 600, margin: 0, marginBottom: 12 }}>
          Delete secret?
        </h2>
        <p style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
          Any deployed BPMN that references <code>{name}</code> will fail at the next fire
          of an HTTP task using it. This cannot be undone.
        </p>
        {error && (
          <div style={{ fontSize: 12, color: 'var(--danger)', marginTop: 8 }}>{error}</div>
        )}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 20 }}>
          <button className="btn-ghost" onClick={onCancel}>Cancel</button>
          <button className="btn-danger" disabled={pending} onClick={onConfirm}>
            {pending ? 'Deleting…' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  )
}
