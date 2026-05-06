import { apiFetch } from './client'

export interface ProcessDefinition {
  id: string
  org_id: string
  process_key: string
  name: string | null
  version: number
  bpmn_xml: string
  deployed_at: string
  status: 'draft' | 'deployed'
  process_group_id: string
}

export const fetchDeployments = (org_id: string) =>
  apiFetch<ProcessDefinition[]>(`/api/v1/deployments?org_id=${org_id}`)

export const fetchDeployment = (id: string) =>
  apiFetch<ProcessDefinition>(`/api/v1/deployments/${id}`)

export const deployProcess = (body: {
  org_id: string
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>('/api/v1/deployments', { method: 'POST', body: JSON.stringify(body) })

export const saveDraft = (body: {
  org_id: string
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>('/api/v1/deployments/draft', { method: 'POST', body: JSON.stringify(body) })

export const createDraft = (body: {
  org_id: string
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>('/api/v1/deployments/draft/new', { method: 'POST', body: JSON.stringify(body) })

export const promoteDraft = (id: string) =>
  apiFetch<ProcessDefinition>(`/api/v1/deployments/${id}/promote`, { method: 'POST' })

export const deleteDeployment = (id: string) =>
  apiFetch<void>(`/api/v1/deployments/${id}`, { method: 'DELETE' })

/**
 * Logical "process" — a `process_key` within a group, with all of its
 * deployed versions and drafts collected. The latest deployed version (or
 * latest draft if none deployed) is exposed as `latest`.
 */
export interface LogicalProcess {
  key: string
  groupId: string
  /** Display name from the latest version. Falls back to key. */
  displayName: string
  /** Latest deployed version, if any. */
  latestDeployed: ProcessDefinition | null
  /** All versions (drafts + deployed), newest first. */
  versions: ProcessDefinition[]
  /** Convenience: pick `latestDeployed ?? versions[0]`. */
  latest: ProcessDefinition
  hasDraft: boolean
}

export interface LayoutData {
  nodes: Record<string, { x: number; y: number }>;
  edges: Record<string, { sourceHandle?: string; targetHandle?: string }>;
}

export const fetchLayout = (org_id: string, process_key: string) =>
  apiFetch<LayoutData>(`/api/v1/orgs/${org_id}/processes/${encodeURIComponent(process_key)}/layout`)

export const saveLayout = (org_id: string, process_key: string, layout: LayoutData) =>
  apiFetch<LayoutData>(
    `/api/v1/orgs/${org_id}/processes/${encodeURIComponent(process_key)}/layout`,
    { method: 'PUT', body: JSON.stringify(layout) },
  )

export function groupByProcessKey(defs: ProcessDefinition[]): LogicalProcess[] {
  const buckets = new Map<string, ProcessDefinition[]>()
  for (const d of defs) {
    const k = `${d.process_group_id}::${d.process_key}`
    const arr = buckets.get(k) ?? []
    arr.push(d)
    buckets.set(k, arr)
  }
  const result: LogicalProcess[] = []
  for (const [, arr] of buckets) {
    arr.sort((a, b) => {
      // Drafts first, then deployed by version desc.
      if (a.status !== b.status) return a.status === 'draft' ? -1 : 1
      return b.version - a.version
    })
    const deployed = arr.filter(d => d.status === 'deployed')
    const latestDeployed = deployed[0] ?? null
    const display = arr.find(d => d.name)?.name ?? arr[0].process_key
    result.push({
      key: arr[0].process_key,
      groupId: arr[0].process_group_id,
      displayName: display,
      latestDeployed,
      versions: arr,
      latest: latestDeployed ?? arr[0],
      hasDraft: arr.some(d => d.status === 'draft'),
    })
  }
  return result.sort((a, b) => a.displayName.localeCompare(b.displayName))
}
