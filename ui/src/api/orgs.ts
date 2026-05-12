import { apiFetch } from './client'

export interface Org {
  id: string
  name: string
  slug: string
  created_at: string
  admin_email: string | null
  admin_name: string | null
  support_email: string | null
  description: string | null
}

/**
 * Body for `POST /api/v1/orgs`. Contact / description fields are
 * optional and may be omitted; empty strings are accepted and treated
 * as omitted server-side.
 */
export interface CreateOrgBody {
  name: string
  slug: string
  admin_email?: string
  admin_name?: string
  support_email?: string
  description?: string
}

export const fetchOrgs = () => apiFetch<Org[]>('/api/v1/orgs')

export const createOrg = (body: CreateOrgBody) =>
  apiFetch<Org>('/api/v1/orgs', { method: 'POST', body: JSON.stringify(body) })

export const deleteOrg = (id: string) =>
  apiFetch<void>(`/api/v1/orgs/${id}`, { method: 'DELETE' })

/**
 * Per-entity counts that block an org delete. Each field comes back as
 * a `number` (server returns Postgres `count(*)` which is i64-shaped;
 * JSON encodes as a JS number — safe under any realistic org scale).
 */
export interface OrgStats {
  members: number
  processes: number
  decisions: number
  instances: number
}

export const fetchOrgStats = (id: string) =>
  apiFetch<OrgStats>(`/api/v1/orgs/${id}/stats`)
