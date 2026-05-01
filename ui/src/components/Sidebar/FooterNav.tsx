import { useLocation, Link } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchTasks } from '../../api/tasks'
import { fetchInstances } from '../../api/instances'
import { useOrg } from '../../App'
import { InboxIcon, ListIcon } from './SidebarIcons'
import styles from './Sidebar.module.css'

export default function FooterNav() {
  const location = useLocation()
  const { org } = useOrg()

  const tasksQ = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 30_000,
  })

  const instancesQ = useQuery({
    queryKey: ['instances', org?.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 30_000,
  })

  const pendingTasks = (tasksQ.data ?? []).filter(t => t.state === 'active' || t.state === 'pending').length
  const runningInstances = (instancesQ.data ?? []).filter(i => i.state === 'running').length

  return (
    <nav className={styles.footer}>
      <Link
        to="/tasks"
        className={`${styles.footerRow} ${location.pathname === '/tasks' ? styles.selected : ''}`}
      >
        <span className={styles.icon}><InboxIcon size={13} /></span>
        <span>Tasks</span>
        {pendingTasks > 0 && <span className={styles.footerCount}>{pendingTasks}</span>}
      </Link>
      <Link
        to="/instances"
        className={`${styles.footerRow} ${location.pathname.startsWith('/instances') && !location.pathname.includes('/instances/') ? styles.selected : ''}`}
      >
        <span className={styles.icon}><ListIcon size={13} /></span>
        <span>All instances</span>
        {runningInstances > 0 && <span className={styles.footerCount}>{runningInstances}</span>}
      </Link>
      <Link
        to="/secrets"
        className={`${styles.footerRow} ${location.pathname === '/secrets' ? styles.selected : ''}`}
      >
        <span className={styles.icon}><KeyIcon size={13} /></span>
        <span>Secrets</span>
      </Link>
    </nav>
  )
}

function KeyIcon({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4">
      <circle cx="6" cy="10" r="3" />
      <path d="M8 8 L13.5 2.5 M11 5 L13 7 M12 4 L14 6" strokeLinecap="round" />
    </svg>
  )
}
