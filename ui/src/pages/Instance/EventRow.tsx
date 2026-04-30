import { useState } from 'react'
import type { ProcessEvent } from '../../api/events'
import { formatEvent } from './eventFormatters'
import styles from './EventRow.module.css'

interface Props {
  event: ProcessEvent
}

function formatTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  const ms = String(d.getMilliseconds()).padStart(3, '0')
  return `${hh}:${mm}:${ss}.${ms}`
}

export default function EventRow({ event }: Props) {
  const f = formatEvent(event)
  const [expanded, setExpanded] = useState(false)
  const hasDetail = !!f.detail && Object.keys(f.detail).length > 0

  return (
    <div
      className={styles.row}
      onClick={() => hasDetail && setExpanded(prev => !prev)}
      role={hasDetail ? 'button' : undefined}
      title={hasDetail ? (expanded ? 'Hide details' : 'Show details') : undefined}
    >
      <div className={`${styles.toneBar} ${styles[f.tone] ?? ''}`} />
      <div className={`${styles.icon} ${styles[f.tone] ?? ''}`}>{f.icon}</div>
      <div className={styles.body}>
        <div className={styles.titleRow}>
          <div className={styles.title}>{f.title}</div>
          <div className={styles.time} title={new Date(event.occurred_at).toLocaleString()}>
            {formatTime(event.occurred_at)}
          </div>
        </div>
        {f.subtitle && <div className={styles.subtitle}>{f.subtitle}</div>}
        {expanded && f.detail && (
          <pre className={styles.detail}>{JSON.stringify(f.detail, null, 2)}</pre>
        )}
      </div>
    </div>
  )
}
