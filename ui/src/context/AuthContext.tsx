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
