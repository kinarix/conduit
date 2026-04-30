import { useState, type ReactNode } from 'react'
import styles from './InstanceTabs.module.css'

export interface TabSpec {
  id: string
  label: string
  count?: number
  /** Render error-style badge instead of neutral. */
  errorBadge?: boolean
  render: () => ReactNode
}

interface Props {
  tabs: TabSpec[]
  defaultTabId?: string
}

export default function InstanceTabs({ tabs, defaultTabId }: Props) {
  const [active, setActive] = useState<string>(defaultTabId ?? tabs[0]?.id ?? '')
  const current = tabs.find(t => t.id === active) ?? tabs[0]

  return (
    <div style={{ display: 'flex', flexDirection: 'column' }}>
      <div className={styles.tabBar}>
        {tabs.map(t => (
          <button
            key={t.id}
            type="button"
            className={`${styles.tab} ${active === t.id ? styles.active : ''}`}
            onClick={() => setActive(t.id)}
          >
            {t.label}
            {t.count != null && t.count > 0 && (
              <span className={`${styles.badge} ${t.errorBadge ? styles.badgeError : ''}`}>
                {t.count}
              </span>
            )}
          </button>
        ))}
      </div>
      <div className={styles.panel}>{current?.render()}</div>
    </div>
  )
}
