import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchInstanceEvents } from '../../api/events'
import { useOrg } from '../../App'
import EventRow from './EventRow'
import { ALL_CATEGORIES, formatEvent, type EventCategory } from './eventFormatters'
import styles from './InstanceTimeline.module.css'

interface Props {
  instanceId: string
}

export default function InstanceTimeline({ instanceId }: Props) {
  const [filter, setFilter] = useState<EventCategory | 'all'>('all')
  const { org } = useOrg()

  const { data: events = [], isLoading } = useQuery({
    queryKey: ['instance-events', org?.id, instanceId],
    queryFn: () => fetchInstanceEvents(org!.id, instanceId),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: events.length }
    for (const ev of events) {
      const cat = formatEvent(ev).category
      c[cat] = (c[cat] ?? 0) + 1
    }
    return c
  }, [events])

  const filtered = useMemo(() => {
    if (filter === 'all') return events
    return events.filter(ev => formatEvent(ev).category === filter)
  }, [events, filter])

  if (isLoading) {
    return <div className={styles.empty}>Loading…</div>
  }

  return (
    <div className={styles.container}>
      <div className={styles.filters}>
        {ALL_CATEGORIES.map(c => {
          const count = counts[c.id] ?? 0
          if (c.id !== 'all' && count === 0) return null
          return (
            <button
              key={c.id}
              type="button"
              className={`${styles.chip} ${filter === c.id ? styles.active : ''}`}
              onClick={() => setFilter(c.id as EventCategory | 'all')}
            >
              {c.label}
              <span className={styles.count}>{count}</span>
            </button>
          )
        })}
      </div>

      <div className={styles.list}>
        {filtered.length === 0 ? (
          <div className={styles.empty}>
            {events.length === 0 ? 'No events recorded yet.' : 'No events match this filter.'}
          </div>
        ) : (
          filtered.map(ev => <EventRow key={ev.id} event={ev} />)
        )}
      </div>
    </div>
  )
}
