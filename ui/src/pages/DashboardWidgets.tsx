import styles from './Dashboard.module.css'

type Tone = 'ok' | 'error' | 'info' | undefined

export function KpiCard({ label, value, tone }: { label: string; value: number; tone?: Tone }) {
  const cls = [styles.kpiValue, tone ? styles[tone] : ''].filter(Boolean).join(' ')
  return (
    <div className={styles.kpiCard}>
      <div className={styles.kpiLabel}>{label}</div>
      <div className={cls}>{value}</div>
    </div>
  )
}

export function Panel({
  title,
  children,
  onViewAll,
}: {
  title: string
  children: React.ReactNode
  onViewAll?: () => void
}) {
  return (
    <div className={styles.panel}>
      <div className={styles.panelHead}>{title}</div>
      <div className={styles.panelBody}>{children}</div>
      {onViewAll && (
        <div className={styles.panelFoot}>
          <button className={styles.viewAll} onClick={onViewAll}>View all →</button>
        </div>
      )}
    </div>
  )
}
