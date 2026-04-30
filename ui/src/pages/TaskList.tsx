import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchTasks, type Task } from '../api/tasks'
import CompleteTaskPanel from './Tasks/CompleteTaskPanel'

export default function TaskList() {
  const [completing, setCompleting] = useState<Task | null>(null)

  const { data: tasks, isLoading } = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 5_000,
  })

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div style={{ padding: 24 }}>
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
                <td>{t.name ?? <span style={{ color: 'var(--text-tertiary)' }}>(unnamed)</span>}</td>
                <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{t.element_id}</td>
                <td style={{ color: 'var(--text-tertiary)' }}>{t.assignee ?? '—'}</td>
                <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                  {new Date(t.created_at).toLocaleString()}
                </td>
                <td>
                  <button className="btn-ghost" onClick={() => setCompleting(t)}>
                    Complete
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {completing && <CompleteTaskPanel task={completing} onClose={() => setCompleting(null)} />}
    </div>
  )
}
