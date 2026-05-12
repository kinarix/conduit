import { apiFetch } from './client'

export interface MeOrgPgRoleEntry {
  process_group_id: string
  process_group_name: string
  role_name: string
}

export interface MeOrgEntry {
  id: string
  name: string
  slug: string
  setup_completed: boolean
  /** Org-scope role grants the user holds in this org. */
  roles: string[]
  /** Permissions granted via org-scope role assignments in this org.
   *  Server resolves role → perms once. Does NOT include global grants —
   *  the AuthContext `hasAny` helper unions with `global_permissions`. */
  permissions: string[]
  /** Process-group-scope role grants inside this org. Empty for users
   *  whose access cascades from an org-level or global grant. */
  pg_roles: MeOrgPgRoleEntry[]
}

export interface MeResponse {
  user_id: string
  email: string
  /// Display name (free text). Null until set. UI falls back to email.
  name: string | null
  /// Phone number (free text). Null until set.
  phone: string | null
  auth_kind: 'jwt' | 'api_key'
  /// Identity backend — `internal` users can change their password
  /// in-app; `external` users must rotate at their IdP.
  auth_provider: 'internal' | 'external'
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

/**
 * Self-service password change. Both fields are required. Backend
 * verifies `current_password` before writing the new hash; wrong-current
 * returns U011, same shape as a failed /auth/login.
 */
export const changeOwnPassword = (currentPassword: string, newPassword: string) =>
  apiFetch<void>('/api/v1/auth/change-password', {
    method: 'POST',
    body: JSON.stringify({
      current_password: currentPassword,
      new_password: newPassword,
    }),
  })

export interface UpdateMeResponse {
  id: string
  email: string
  name: string | null
  phone: string | null
}

/**
 * Self-service profile edit. Omit a key to leave it unchanged; pass an
 * empty string to clear it (stored as NULL).
 */
export const updateMe = (body: { name?: string; phone?: string }) =>
  apiFetch<UpdateMeResponse>('/api/v1/me', {
    method: 'PATCH',
    body: JSON.stringify(body),
  })
