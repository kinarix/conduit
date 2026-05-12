// eslint-disable-next-line @typescript-eslint/no-explicit-any
const BASE = ((import.meta as any).env?.VITE_API_URL as string | undefined) ?? ''

export const TOKEN_KEY = 'conduit.token'

interface ApiErrorBody {
  code?: string
  message?: string
  action?: string
}

export class ApiError extends Error {
  code: string
  action?: string
  status: number

  constructor(message: string, code: string, status: number, action?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.status = status
    this.action = action
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const token = localStorage.getItem(TOKEN_KEY)
  const authHeader: Record<string, string> = token
    ? { Authorization: `Bearer ${token}` }
    : {}

  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', ...authHeader, ...init?.headers },
  })

  // 401 has two meanings: an expired/invalid token on an authed call (session
  // gone — bounce to /login), or a public-endpoint auth failure such as a
  // bad email/password on /auth/login (let the caller render the error).
  // Only the first case warrants the redirect; gate it on whether we actually
  // sent a token.
  if (res.status === 401 && token) {
    console.error('[401] Unauthenticated response from:', path)
    localStorage.removeItem(TOKEN_KEY)
    window.location.href = '/login'
    return undefined as T
  }

  if (!res.ok) {
    let body: ApiErrorBody = {}
    try {
      body = await res.json()
    } catch {
      // not JSON — fall through with empty body
    }
    const code = body.code ?? `HTTP${res.status}`
    const message = body.message ?? (res.statusText || `HTTP ${res.status}`)
    throw new ApiError(`[${code}] ${message}`, code, res.status, body.action)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}
