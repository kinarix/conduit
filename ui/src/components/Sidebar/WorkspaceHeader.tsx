import styles from './Sidebar.module.css'

export default function WorkspaceHeader() {
  return (
    <header className={styles.header}>
      <div className={styles.brand}>Conduit</div>
    </header>
  )
}
