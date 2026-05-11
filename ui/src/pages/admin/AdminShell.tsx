import { NavLink, Outlet } from 'react-router-dom'
import { useAuth } from '../../context/AuthContext'

const TABS: { to: string; label: string; perms: string[] }[] = [
  { to: 'users',    label: 'Users',    perms: ['user.manage', 'role.manage'] },
  { to: 'roles',    label: 'Roles',    perms: ['role.manage'] },
  { to: 'auth',     label: 'Auth',     perms: ['org.manage'] },
  { to: 'settings', label: 'Settings', perms: ['org.manage'] },
]

export default function AdminShell() {
  const { user } = useAuth()
  const perms = new Set(user?.permissions ?? [])
  const visibleTabs = TABS.filter(t => t.perms.some(p => perms.has(p)))

  return (
    <div style={{ padding: '24px 32px', maxWidth: 960, margin: '0 auto' }}>
      <div style={{ marginBottom: 24 }}>
        <h1 style={{ fontSize: 20, fontWeight: 700, margin: 0 }}>Admin</h1>
      </div>

      <nav style={{
        display: 'flex',
        gap: 0,
        borderBottom: '1px solid var(--color-border)',
        marginBottom: 28,
      }}>
        {visibleTabs.map(tab => (
          <NavLink
            key={tab.to}
            to={tab.to}
            style={({ isActive }) => ({
              padding: '8px 16px',
              fontSize: 13,
              fontWeight: isActive ? 600 : 400,
              color: isActive ? 'var(--color-primary)' : 'var(--color-text-muted)',
              borderBottom: `2px solid ${isActive ? 'var(--color-primary)' : 'transparent'}`,
              textDecoration: 'none',
              marginBottom: -1,
              transition: 'color 0.15s, border-color 0.15s',
            })}
          >
            {tab.label}
          </NavLink>
        ))}
      </nav>

      <Outlet />
    </div>
  )
}
