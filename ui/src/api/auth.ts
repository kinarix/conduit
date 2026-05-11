import { apiFetch } from './client'

export interface MeResponse {
  user_id: string
  org_id: string
  email: string
  auth_kind: 'jwt' | 'api_key'
  permissions: string[]
  roles: string[]
  setup_completed: boolean
}

export interface LoginResponse {
  access_token: string
  token_type: string
  expires_in: number
}

export interface LoginOrg {
  name: string
  slug: string
  is_system: boolean
}

/** Pass `org_slug = ''` (or omit) to sign in as platform admin. */
export const login = (org_slug: string, email: string, password: string) =>
  apiFetch<LoginResponse>('/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify(
      org_slug ? { org_slug, email, password } : { email, password }
    ),
  })

export const fetchLoginOrgs = () =>
  apiFetch<LoginOrg[]>('/api/v1/auth/orgs')

export const fetchMe = () => apiFetch<MeResponse>('/api/v1/me')
