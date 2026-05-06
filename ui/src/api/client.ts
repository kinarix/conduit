// eslint-disable-next-line @typescript-eslint/no-explicit-any
const BASE = ((import.meta as any).env?.VITE_API_URL as string | undefined) ?? ''

interface ApiErrorBody {
  code?: string
  message?: string
  action?: string
}

export class ApiError extends Error {
  code: string
  action?: string

  constructor(message: string, code: string, action?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.action = action
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json', ...init?.headers },
    ...init,
  })
  if (!res.ok) {
    let body: ApiErrorBody = {}
    try {
      body = await res.json()
    } catch {
      // not JSON — fall through with empty body
    }
    const code = body.code ?? `HTTP${res.status}`
    const message = body.message ?? (res.statusText || `HTTP ${res.status}`)
    throw new ApiError(`[${code}] ${message}`, code, body.action)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}
