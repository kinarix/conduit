import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchDeployments } from '../api/deployments'
import { startInstance, toVariableInputs } from '../api/instances'
import { useOrg } from '../App'

// Re-export helper from instances for use in this module
// (kept here to avoid a circular dep through App)

export default function DefinitionsList() {
  const { org } = useOrg()
  const navigate = useNavigate()
  const qc = useQueryClient()
  const [startingId, setStartingId] = useState<string | null>(null)
  const [variables, setVariables] = useState('')
  const [varError, setVarError] = useState('')

  const { data: defs, isLoading } = useQuery({
    queryKey: ['deployments', org!.id],
    queryFn: () => fetchDeployments(org!.id),
    enabled: !!org,
  })

  const startMut = useMutation({
    mutationFn: (defId: string) => {
      let vars
      if (variables.trim()) {
        try {
          vars = toVariableInputs(JSON.parse(variables) as Record<string, unknown>)
        } catch {
          throw new Error('Invalid JSON')
        }
      }
      return startInstance({ org_id: org!.id, definition_id: defId, variables: vars })
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['instances'] })
      setStartingId(null)
      setVariables('')
      setVarError('')
      navigate('/instances')
    },
    onError: (e: Error) => setVarError(e.message),
  })

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
        <h1 style={{ fontSize: 18, fontWeight: 600 }}>Process Definitions</h1>
        <Link to="/definitions/new">
          <button className="btn-primary">+ New</button>
        </Link>
      </div>

      {!defs || defs.length === 0 ? (
        <div className="empty-state">
          <p>No definitions yet. Click <strong>+ New</strong> to design and deploy a BPMN process.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Key</th>
              <th>Name</th>
              <th>Version</th>
              <th>Deployed</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {defs.map(d => (
              <tr key={d.id}>
                <td style={{ fontFamily: 'monospace', fontSize: 12 }}>{d.key}</td>
                <td>{d.name}</td>
                <td style={{ color: 'var(--color-text-muted)' }}>v{d.version}</td>
                <td style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>
                  {new Date(d.deployed_at).toLocaleString()}
                </td>
                <td>
                  <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                    <button
                      className="btn-ghost"
                      onClick={() => { setStartingId(d.id); setVarError('') }}
                    >
                      Start
                    </button>
                    <Link to={`/definitions/${d.id}`}>
                      <button className="btn-ghost">Edit</button>
                    </Link>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {startingId && (
        <div className="modal-overlay" onClick={() => setStartingId(null)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Start instance</h3>
            <div className="field">
              <label>Initial variables — JSON object (optional)</label>
              <textarea
                rows={4}
                placeholder={'{ "amount": 100, "approved": false }'}
                value={variables}
                onChange={e => setVariables(e.target.value)}
              />
            </div>
            {varError && <div className="error-banner">{varError}</div>}
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setStartingId(null)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={startMut.isPending}
                onClick={() => startMut.mutate(startingId)}
              >
                {startMut.isPending ? 'Starting…' : 'Start'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
