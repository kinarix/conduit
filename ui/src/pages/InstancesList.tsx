import { useQuery } from '@tanstack/react-query'
import { fetchInstances } from '../api/instances'
import { useOrg } from '../App'

const STATE_CLASS: Record<string, string> = {
  running:   'badge-running',
  completed: 'badge-completed',
  error:     'badge-error',
  failed:    'badge-error',
}

export default function InstancesList() {
  const { org } = useOrg()

  const { data: instances, isLoading } = useQuery({
    queryKey: ['instances', org!.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div>
      <h1 style={{ fontSize: 18, fontWeight: 600, marginBottom: 20 }}>Process Instances</h1>

      {!instances || instances.length === 0 ? (
        <div className="empty-state">
          <p>No instances yet. Start one from the <a href="/definitions">Definitions</a> page.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>State</th>
              <th>Started</th>
              <th>Ended</th>
            </tr>
          </thead>
          <tbody>
            {instances.map(i => (
              <tr key={i.id}>
                <td style={{ fontFamily: 'monospace', fontSize: 11 }}>
                  {i.id.slice(0, 8)}…
                </td>
                <td>
                  <span className={`badge ${STATE_CLASS[i.state] ?? 'badge-active'}`}>
                    {i.state}
                  </span>
                </td>
                <td style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>
                  {new Date(i.started_at).toLocaleString()}
                </td>
                <td style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>
                  {i.ended_at ? new Date(i.ended_at).toLocaleString() : '—'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
