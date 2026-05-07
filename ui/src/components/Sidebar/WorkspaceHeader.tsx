import styles from './Sidebar.module.css'

export default function WorkspaceHeader() {
  return (
    <header className={styles.header}>
      <img src="/favicon.svg" width={20} height={20} alt="" aria-hidden style={{ borderRadius: 4, flexShrink: 0 }} />
      <div className={styles.brand}>Conduit</div>
    </header>
  )
}
