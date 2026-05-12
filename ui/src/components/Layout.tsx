import { useEffect, useRef, useState } from 'react'
import { Outlet, useLocation } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchOrgs } from '../api/orgs'
import Sidebar from './Sidebar/Sidebar'
import TopTabs from './TopTabs'
import Welcome from '../pages/Welcome'
import PlatformShell from '../pages/platform/PlatformShell'
import AccountMenu from './AccountMenu'
import { useAuth, useCurrentPerms } from '../context/AuthContext'
import { useOrg } from '../App'

const SIDEBAR_MIN = 160
const SIDEBAR_MAX = 520
const SIDEBAR_DEFAULT = 260
const SIDEBAR_STORAGE_KEY = 'sidebar.width'

function readSavedWidth() {
  const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY)
  if (!raw) return SIDEBAR_DEFAULT
  const n = parseInt(raw, 10)
  return isNaN(n) ? SIDEBAR_DEFAULT : Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, n))
}

export default function Layout() {
  const { user } = useAuth()
  const isPlatformAdmin = user?.is_global_admin ?? false
  // Sidebar visibility rules:
  //   - Admin routes: always hide. The AdminShell has its own side
  //     panel; the org tree is irrelevant there.
  //   - Dashboard `/`: hide only when the user has zero operational
  //     access. A pure admin (OrgAdmin alone) would see an empty tree
  //     and clutter; a Developer / Operator / mixed-role user needs
  //     the tree to navigate to processes, decisions, tasks, etc.
  //     from the landing page.
  //   - Everywhere else: show. These routes ARE the tree's domain.
  const location = useLocation()
  const path = location.pathname
  const { org } = useOrg()
  const { hasAny } = useCurrentPerms(org?.id)
  const isAdminRoute = path === '/admin' || path.startsWith('/admin/')
  const isDashboardRoute = path === '/'
  // Any of these is enough to make the org-tree useful: pg/process
  // browsing, instance/task work, decisions, secrets, layouts.
  const hasOperationalAccess = isPlatformAdmin || hasAny([
    'process_group.read', 'process_group.create',
    'process.read', 'process.create', 'process.deploy',
    'instance.read', 'instance.start',
    'task.read', 'task.complete',
    'decision.read', 'decision.create',
    'secret.read_metadata', 'secret.create',
    'process_layout.read',
  ])
  const hideSidebar = isAdminRoute || (isDashboardRoute && !hasOperationalAccess)

  const { isLoading, isFetching, isError, refetch } = useQuery({
    queryKey: ['orgs'],
    queryFn: fetchOrgs,
    retry: 1,
    enabled: !isPlatformAdmin,
  })

  if (isPlatformAdmin) return <PlatformShell />

  const [sidebarWidth, setSidebarWidth] = useState(readSavedWidth)
  const [dragging, setDragging] = useState(false)
  const dragStart = useRef<{ x: number; width: number } | null>(null)

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId)
    dragStart.current = { x: e.clientX, width: sidebarWidth }
    setDragging(true)
  }

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    const next = Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, dragStart.current.width + e.clientX - dragStart.current.x))
    setSidebarWidth(next)
  }

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    const next = Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, dragStart.current.width + e.clientX - dragStart.current.x))
    localStorage.setItem(SIDEBAR_STORAGE_KEY, String(next))
    dragStart.current = null
    setDragging(false)
  }

  if (isLoading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh' }}>
        <div className="spinner" />
      </div>
    )
  }

  if (isError) {
    return <BackendDown onRetry={refetch} isFetching={isFetching} />
  }

  return (
    <div style={{ display: 'flex', height: '100vh', overflow: 'hidden', cursor: dragging ? 'col-resize' : undefined }}>
      <AccountMenu />
      {!hideSidebar && <Sidebar width={sidebarWidth} />}

      {!hideSidebar && (
        <div
          style={{
            width: 4,
            flexShrink: 0,
            cursor: 'col-resize',
            background: dragging ? 'var(--accent)' : 'transparent',
            transition: dragging ? 'none' : 'background 0.15s',
            userSelect: 'none',
          }}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={handlePointerUp}
          onMouseEnter={e => { if (!dragging) (e.currentTarget as HTMLDivElement).style.background = 'var(--border-primary)' }}
          onMouseLeave={e => { if (!dragging) (e.currentTarget as HTMLDivElement).style.background = 'transparent' }}
        />
      )}

      <main style={{ flex: 1, overflow: 'auto', minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        {/* setup_completed is per-org under phase-23.1; show Welcome if the
            user's first/only org is still pending setup. Multi-org users
            see the regular shell — the org switcher (TODO) will let them
            pick which one to operate in. Admin routes bypass Welcome
            entirely so an org admin can configure a freshly-bootstrapped
            org before it's marked setup-complete. */}
        {!isAdminRoute && user?.orgs?.length === 1 && user.orgs[0].setup_completed === false ? (
          <Welcome />
        ) : (
          <>
            {(isAdminRoute || isDashboardRoute) && <TopTabs />}
            <div style={{ flex: 1, minHeight: 0, overflow: 'auto' }}>
              <Outlet />
            </div>
          </>
        )}
      </main>
    </div>
  )
}


const BASE_DELAY = 5
const MAX_DELAY = 60

function BackendDown({ onRetry, isFetching }: { onRetry: () => void; isFetching: boolean }) {
  const attempt = useRef(0)
  const remaining = useRef(BASE_DELAY)
  const onRetryRef = useRef(onRetry)
  const isFetchingRef = useRef(isFetching)
  const [display, setDisplay] = useState(BASE_DELAY)

  useEffect(() => { onRetryRef.current = onRetry }, [onRetry])
  useEffect(() => { isFetchingRef.current = isFetching }, [isFetching])

  useEffect(() => {
    const tick = setInterval(() => {
      if (isFetchingRef.current) return
      remaining.current -= 1
      if (remaining.current <= 0) {
        attempt.current += 1
        const next = Math.min(BASE_DELAY * Math.pow(2, attempt.current), MAX_DELAY)
        remaining.current = next
        setDisplay(next)
        onRetryRef.current()
      } else {
        setDisplay(remaining.current)
      }
    }, 1000)
    return () => clearInterval(tick)
  }, [])

  const handleManualRetry = () => {
    attempt.current = 0
    remaining.current = BASE_DELAY
    setDisplay(BASE_DELAY)
    onRetryRef.current()
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh' }}>
      <div style={{ textAlign: 'center', maxWidth: 400, padding: 24 }}>
        <div style={{ fontSize: 40, marginBottom: 16 }}>⚡</div>
        <h2 style={{ fontSize: 18, fontWeight: 700, marginBottom: 8 }}>Oops — can't reach the server</h2>
        <p style={{ fontSize: 14, color: 'var(--color-text-muted)', lineHeight: 1.6, marginBottom: 24 }}>
          Conduit couldn't connect to the backend. Make sure the server is running and try again.
        </p>
        <div style={{ display: 'flex', gap: 8, justifyContent: 'center', alignItems: 'center' }}>
          <button className="btn-primary" onClick={handleManualRetry} disabled={isFetching}
            style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            {isFetching && <div className="spinner" style={{ width: 12, height: 12 }} />}
            {isFetching ? 'Retrying…' : 'Retry now'}
          </button>
          {!isFetching && (
            <span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>
              Retrying in {display}s…
            </span>
          )}
        </div>
      </div>
    </div>
  )
}
