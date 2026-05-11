import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchInstanceEvents, type ProcessEvent } from '../../api/events'
import { useOrg } from '../../App'
import styles from './InstanceVariables.module.css'

interface Props {
  instanceId: string
}

interface VarSnapshot {
  name: string
  value: unknown
  scope: string | null // execution_id of the scope
  lastTouched: string
  history: number // number of writes seen
}

/**
 * Reconstructs the latest value of every variable by replaying the event log.
 * For each (execution_id, name), the most recent variable_set / variable_changed wins.
 */
function reconstruct(events: ProcessEvent[]): VarSnapshot[] {
  const map = new Map<string, VarSnapshot>()
  for (const ev of events) {
    if (ev.event_type !== 'variable_set' && ev.event_type !== 'variable_changed') continue
    const name = ev.payload.name as string | undefined
    if (!name) continue
    const key = `${ev.execution_id ?? '~'}::${name}`
    const prev = map.get(key)
    map.set(key, {
      name,
      value: ev.payload.new_value,
      scope: ev.execution_id,
      lastTouched: ev.occurred_at,
      history: (prev?.history ?? 0) + 1,
    })
  }
  return [...map.values()].sort((a, b) => a.name.localeCompare(b.name))
}

function display(v: unknown): string {
  if (v == null) return 'null'
  if (typeof v === 'string') return v
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

export default function InstanceVariables({ instanceId }: Props) {
  const { org } = useOrg()
  const { data: events = [], isLoading } = useQuery({
    queryKey: ['instance-events', org?.id, instanceId],
    queryFn: () => fetchInstanceEvents(org!.id, instanceId),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  const snapshot = useMemo(() => reconstruct(events), [events])

  if (isLoading) return <div className={styles.empty}>Loading…</div>
  if (snapshot.length === 0) {
    return <div className={styles.empty}>No variables recorded for this instance yet.</div>
  }

  return (
    <div className={styles.wrapper}>
      <div className={styles.note}>
        Latest value of each variable, derived from the event log. Changes are kept in
        the Timeline tab.
      </div>
      <table className={styles.table}>
        <thead>
          <tr>
            <th>Name</th>
            <th>Value</th>
            <th>Writes</th>
            <th>Last touched</th>
            <th>Scope</th>
          </tr>
        </thead>
        <tbody>
          {snapshot.map(s => (
            <tr key={`${s.scope ?? ''}::${s.name}`}>
              <td className={styles.nameCell}>{s.name}</td>
              <td className={styles.valueCell}>
                <code>{display(s.value)}</code>
              </td>
              <td className={styles.numCell}>{s.history}</td>
              <td className={styles.dateCell}>{new Date(s.lastTouched).toLocaleString()}</td>
              <td className={styles.scopeCell}>{s.scope ? s.scope.slice(0, 8) + '…' : '—'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
