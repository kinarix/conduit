import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchTasks, completeTask, toVariableInputs } from '../api/tasks'

export default function TaskList() {
  const qc = useQueryClient()
  const [completingId, setCompletingId] = useState<string | null>(null)
  const [variables, setVariables] = useState('')
  const [error, setError] = useState('')

  const { data: tasks, isLoading } = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 5_000,
  })

  const completeMut = useMutation({
    mutationFn: (id: string) => {
      let vars
      if (variables.trim()) {
        try {
          vars = toVariableInputs(JSON.parse(variables) as Record<string, unknown>)
        } catch {
          throw new Error('Invalid JSON')
        }
      }
      return completeTask(id, vars)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tasks'] })
      setCompletingId(null)
      setVariables('')
      setError('')
    },
    onError: (e: Error) => setError(e.message),
  })

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div>
      <h1 style={{ fontSize: 18, fontWeight: 600, marginBottom: 20 }}>Tasks</h1>

      {!tasks || tasks.length === 0 ? (
        <div className="empty-state">
          <p>No open tasks.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Task</th>
              <th>Element</th>
              <th>Assignee</th>
              <th>Created</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {tasks.map(t => (
              <tr key={t.id}>
                <td>{t.name ?? <span style={{ color: 'var(--color-text-muted)' }}>(unnamed)</span>}</td>
                <td style={{ fontFamily: 'monospace', fontSize: 11 }}>{t.element_id}</td>
                <td style={{ color: 'var(--color-text-muted)' }}>{t.assignee ?? '—'}</td>
                <td style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>
                  {new Date(t.created_at).toLocaleString()}
                </td>
                <td>
                  <button
                    className="btn-ghost"
                    onClick={() => { setCompletingId(t.id); setError('') }}
                  >
                    Complete
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {completingId && (
        <div className="modal-overlay" onClick={() => setCompletingId(null)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Complete task</h3>
            <div className="field">
              <label>Output variables — JSON object (optional)</label>
              <textarea
                rows={4}
                placeholder={'{ "approved": true, "amount": 500 }'}
                value={variables}
                onChange={e => setVariables(e.target.value)}
              />
            </div>
            {error && <div className="error-banner">{error}</div>}
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setCompletingId(null)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={completeMut.isPending}
                onClick={() => completeMut.mutate(completingId)}
              >
                {completeMut.isPending ? 'Completing…' : 'Complete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
