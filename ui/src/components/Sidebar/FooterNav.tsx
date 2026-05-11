import { useLocation, Link } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchTasks } from '../../api/tasks'
import { fetchInstances } from '../../api/instances'
import { useOrg } from '../../App'
import { useAuth } from '../../context/AuthContext'
import { InboxIcon, ListIcon } from './SidebarIcons'
import styles from './Sidebar.module.css'

export default function FooterNav() {
  const location = useLocation()
  const { org } = useOrg()
  const { user } = useAuth()
  const canAdmin = user?.permissions?.some(p =>
    p === 'org.manage' || p === 'user.manage' || p === 'role.manage'
  ) ?? false
  const canManageSecrets = user?.permissions?.includes('secret.manage') ?? false

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
      {canManageSecrets && (
        <Link
          to="/secrets"
          className={`${styles.footerRow} ${location.pathname === '/secrets' ? styles.selected : ''}`}
        >
          <span className={styles.icon}><KeyIcon size={13} /></span>
          <span>Secrets</span>
        </Link>
      )}
      {canAdmin && (
        <Link
          to="/admin"
          className={`${styles.footerRow} ${location.pathname.startsWith('/admin') ? styles.selected : ''}`}
        >
          <span className={styles.icon}><ShieldIcon size={13} /></span>
          <span>Admin</span>
        </Link>
      )}
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

function ShieldIcon({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M8 1.5 L13.5 4 V8.5 C13.5 11.5 8 14.5 8 14.5 C8 14.5 2.5 11.5 2.5 8.5 V4 Z" strokeLinejoin="round" />
    </svg>
  )
}
