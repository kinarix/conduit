import { NavLink, useLocation } from 'react-router-dom'
import { useOrg } from '../App'
import { useCurrentPerms } from '../context/AuthContext'

/**
 * Top navigation bar shown on the screens where the org/project tree
 * sidebar is hidden (the dashboard and the admin console). Two tabs:
 *
 *   Dashboard  →  `/`
 *   Settings   →  `/admin` (renders the admin shell)
 *
 * The Settings tab is gated by the same permission set as the old
 * FooterNav admin link — global admins always see it; org admins see it
 * when they hold any admin-relevant permission in the current org.
 *
 * Inside a process group / process / instance the left sidebar is the
 * primary nav, so these tabs are intentionally not rendered there.
 */
export default function TopTabs() {
  const { org } = useOrg()
  const { hasAny } = useCurrentPerms(org?.id)
  const location = useLocation()
  const onAdmin =
    location.pathname === '/admin' || location.pathname.startsWith('/admin/')

  const canSeeSettings = hasAny([
    'org.read', 'org.update',
    'user.read', 'role.read',
    'role_assignment.read',
    'auth_config.read',
    'notification_config.read',
  ])

  return (
    <nav
      style={{
        display: 'flex',
        gap: 4,
        padding: '12px 24px 0',
        borderBottom: '1px solid var(--color-border)',
        background: 'var(--bg-primary)',
      }}
    >
      <TabLink to="/" active={!onAdmin}>Dashboard</TabLink>
      {canSeeSettings && (
        <TabLink to="/admin" active={onAdmin}>Settings</TabLink>
      )}
    </nav>
  )
}

function TabLink({
  to, active, children,
}: { to: string; active: boolean; children: React.ReactNode }) {
  return (
    <NavLink
      to={to}
      style={{
        padding: '8px 16px',
        fontSize: 13,
        fontWeight: active ? 600 : 400,
        color: active ? 'var(--color-primary)' : 'var(--color-text-muted)',
        borderBottom: `2px solid ${active ? 'var(--color-primary)' : 'transparent'}`,
        textDecoration: 'none',
        marginBottom: -1,
        transition: 'color 0.15s, border-color 0.15s',
      }}
      end
    >
      {children}
    </NavLink>
  )
}
