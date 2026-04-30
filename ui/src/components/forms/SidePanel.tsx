import { useEffect, type ReactNode } from 'react'
import styles from './SidePanel.module.css'

interface Props {
  title: string
  subtitle?: ReactNode
  onClose: () => void
  footer: ReactNode
  children: ReactNode
}

export default function SidePanel({ title, subtitle, onClose, footer, children }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div className={styles.overlay} onClick={onClose}>
      <aside className={styles.panel} onClick={e => e.stopPropagation()}>
        <header className={styles.header}>
          <div>
            <h3 className={styles.title}>{title}</h3>
            {subtitle && <div className={styles.subtitle}>{subtitle}</div>}
          </div>
          <button type="button" className={styles.closeBtn} onClick={onClose} title="Close (Esc)">
            ✕
          </button>
        </header>
        <div className={styles.body}>{children}</div>
        <div className={styles.footer}>{footer}</div>
      </aside>
    </div>
  )
}
