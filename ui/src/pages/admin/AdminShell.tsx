import { NavLink, Outlet } from 'react-router-dom'
import { useOrg } from '../../App'
import { useCurrentPerms } from '../../context/AuthContext'

type Entry = { to: string; label: string; perms: string[] }
type Group = { heading: string; entries: Entry[] }

/**
 * Admin entries grouped for the vertical side panel. Each entry's
 * `perms` list is the union of permissions sufficient to make the
 * destination useful — `hasAny` short-circuits on the first match.
 *
 * Settings used to be a single page; it's now a group of sibling routes
 * (general / auth / notifications) at the top level so the side panel
 * stays flat with no second-level expansion.
 */
const GROUPS: Group[] = [
  {
    heading: 'Manage',
    entries: [
      { to: 'users', label: 'Users', perms: ['user.read', 'user.create', 'role_assignment.read', 'role_assignment.create'] },
      // Role catalog management is platform-admin-only. Org admins
      // assign roles via the Users tab but don't see this catalog tab —
      // `role.read` alone is not enough to surface it, you also need
      // create/update/delete to do anything meaningful here.
      { to: 'roles', label: 'Roles', perms: ['role.create', 'role.update', 'role.delete'] },
    ],
  },
  {
    heading: 'Settings',
    entries: [
      { to: 'general',       label: 'General',        perms: ['org.read', 'org.update'] },
      { to: 'auth',          label: 'Authentication', perms: ['auth_config.read', 'auth_config.update'] },
      { to: 'notifications', label: 'Notifications',  perms: ['notification_config.read', 'notification_config.update'] },
    ],
  },
]

export default function AdminShell() {
  const { org } = useOrg()
  const { hasAny } = useCurrentPerms(org?.id)

  const visibleGroups = GROUPS
    .map(g => ({ ...g, entries: g.entries.filter(e => hasAny(e.perms)) }))
    .filter(g => g.entries.length > 0)

  return (
    <div style={{ display: 'flex', gap: 32, padding: 24 }}>
      <aside style={{ minWidth: 180, flexShrink: 0 }}>
        <h1 style={{ fontSize: 20, fontWeight: 700, margin: '0 0 20px' }}>Settings</h1>
        <nav style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          {visibleGroups.map(group => (
            <div key={group.heading}>
              <div style={{
                fontSize: 11,
                fontWeight: 600,
                textTransform: 'uppercase',
                letterSpacing: '0.05em',
                color: 'var(--color-text-muted)',
                marginBottom: 6,
                paddingLeft: 8,
              }}>
                {group.heading}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                {group.entries.map(e => (
                  <NavLink
                    key={e.to}
                    to={e.to}
                    style={({ isActive }) => ({
                      padding: '6px 8px',
                      fontSize: 13,
                      fontWeight: isActive ? 600 : 400,
                      color: isActive ? 'var(--color-primary)' : 'var(--color-text)',
                      background: isActive
                        ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 10%, transparent))'
                        : 'transparent',
                      borderRadius: 4,
                      textDecoration: 'none',
                      transition: 'background 0.1s, color 0.1s',
                    })}
                  >
                    {e.label}
                  </NavLink>
                ))}
              </div>
            </div>
          ))}
        </nav>
      </aside>

      <main style={{ flex: 1, minWidth: 0 }}>
        <Outlet />
      </main>
    </div>
  )
}
