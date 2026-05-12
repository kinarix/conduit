import { createContext, useContext, useEffect, useState, ReactNode } from 'react'
import { fetchMe, MeResponse } from '../api/auth'
import { TOKEN_KEY } from '../api/client'

interface AuthContextValue {
  token: string | null
  user: MeResponse | null
  isAuthenticated: boolean
  isLoading: boolean
  setToken: (token: string) => void
  logout: () => void
  refreshUser: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue>({
  token: null,
  user: null,
  isAuthenticated: false,
  isLoading: true,
  setToken: () => {},
  logout: () => {},
  refreshUser: async () => {},
})

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => localStorage.getItem(TOKEN_KEY))
  const [user, setUser] = useState<MeResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    if (!token) {
      setIsLoading(false)
      return
    }
    fetchMe()
      .then(me => {
        setUser(me)
        setIsLoading(false)
      })
      .catch(() => {
        // token invalid — client.ts will have already redirected on 401
        setIsLoading(false)
      })
  }, [token])

  const setToken = (newToken: string) => {
    localStorage.setItem(TOKEN_KEY, newToken)
    setTokenState(newToken)
    setIsLoading(true)
    fetchMe()
      .then(me => {
        setUser(me)
        setIsLoading(false)
      })
      .catch(() => setIsLoading(false))
  }

  const logout = () => {
    localStorage.removeItem(TOKEN_KEY)
    setTokenState(null)
    setUser(null)
    window.location.href = '/login'
  }

  const refreshUser = async () => {
    if (!token) return
    try {
      const me = await fetchMe()
      if (me) setUser(me)
    } catch { /* 401 handled by client.ts redirect */ }
  }

  return (
    <AuthContext.Provider value={{ token, user, isAuthenticated: !!token && !!user, isLoading, setToken, logout, refreshUser }}>
      {children}
    </AuthContext.Provider>
  )
}

export const useAuth = () => useContext(AuthContext)

/**
 * Resolve the caller's effective permission set in the context of an org.
 *
 * Global admins satisfy every check (server-side authorisation still
 * applies on each request — this only drives UI visibility).
 *
 * For non-admins, `hasAny` returns true when ANY of the supplied
 * permission strings is present in either:
 *   - `user.global_permissions` (cross-cutting grants), or
 *   - `user.orgs.find(o => o.id === orgId)?.permissions` when `orgId` is
 *     passed (the per-org bundle resolved server-side from role grants).
 *
 * Pass `orgId` whenever the gate guards an org-scoped surface (admin tabs,
 * the Admin nav link in the current org). Pass `undefined` only for
 * surfaces that genuinely have no org context.
 */
export function useCurrentPerms(orgId?: string | null) {
  const { user } = useAuth()
  const isGlobalAdmin = user?.is_global_admin ?? false
  // When the caller knows the org, only look at that bundle. When it's
  // still resolving (sidebar hasn't picked an org yet, e.g. direct
  // navigation to `/admin/...`), fall back to the union of all orgs the
  // user belongs to so the gate doesn't flicker-redirect. Per-page
  // queries are still org-scoped and will reject in the wrong org.
  const orgPerms = orgId
    ? user?.orgs.find(o => o.id === orgId)?.permissions ?? []
    : (user?.orgs.flatMap(o => o.permissions) ?? [])
  const merged = new Set<string>([
    ...(user?.global_permissions ?? []),
    ...orgPerms,
  ])
  const hasAny = (perms: string[]) =>
    isGlobalAdmin || perms.some(p => merged.has(p))
  return { isGlobalAdmin, hasAny }
}
