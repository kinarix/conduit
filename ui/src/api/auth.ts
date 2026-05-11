import { apiFetch } from './client'

export interface MeOrgEntry {
  id: string
  name: string
  slug: string
  setup_completed: boolean
  roles: string[]
}

export interface MeResponse {
  user_id: string
  email: string
  auth_kind: 'jwt' | 'api_key'
  is_global_admin: boolean
  /// Permissions held globally (apply across every org).
  global_permissions: string[]
  global_roles: string[]
  orgs: MeOrgEntry[]
}

export interface LoginResponse {
  access_token: string
  token_type: string
  expires_in: number
}

/**
 * Login takes email + password only — email is globally unique since the
 * phase-23.1 RBAC redesign. Choice of org happens after login via the
 * org switcher (URL or local-storage state).
 */
export const login = (email: string, password: string) =>
  apiFetch<LoginResponse>('/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })

export const fetchMe = () => apiFetch<MeResponse>('/api/v1/me')
