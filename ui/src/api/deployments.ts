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
  disabled_at: string | null
}

export const setDeploymentDisabled = (orgId: string, id: string, disabled: boolean) =>
  apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments/${id}/disabled`, {
    method: 'PATCH',
    body: JSON.stringify({ disabled }),
  })

export const fetchDeployments = (orgId: string) =>
  apiFetch<ProcessDefinition[]>(`/api/v1/orgs/${orgId}/deployments`)

export const fetchDeployment = (orgId: string, id: string) =>
  apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments/${id}`)

export const deployProcess = (orgId: string, body: {
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments`, { method: 'POST', body: JSON.stringify(body) })

export const saveDraft = (orgId: string, body: {
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments/draft`, { method: 'POST', body: JSON.stringify(body) })

export const createDraft = (orgId: string, body: {
  process_group_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments/draft/new`, { method: 'POST', body: JSON.stringify(body) })

export const promoteDraft = (orgId: string, id: string) =>
  apiFetch<ProcessDefinition>(`/api/v1/orgs/${orgId}/deployments/${id}/promote`, { method: 'POST' })

export const deleteDeployment = (orgId: string, id: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/deployments/${id}`, { method: 'DELETE' })

export const renameProcess = (orgId: string, body: {
  process_group_id: string
  process_key: string
  name: string
}) => apiFetch<void>(`/api/v1/orgs/${orgId}/deployments/by-key`, { method: 'PATCH', body: JSON.stringify(body) })

export interface LogicalProcess {
  key: string
  groupId: string
  displayName: string
  latestDeployed: ProcessDefinition | null
  versions: ProcessDefinition[]
  latest: ProcessDefinition
  hasDraft: boolean
}

export interface LayoutData {
  nodes: Record<string, { x: number; y: number }>;
  edges: Record<string, { sourceHandle?: string; targetHandle?: string }>;
}

export const fetchLayout = (orgId: string, process_key: string) =>
  apiFetch<LayoutData>(`/api/v1/orgs/${orgId}/processes/${encodeURIComponent(process_key)}/layout`)

export const saveLayout = (orgId: string, process_key: string, layout: LayoutData) =>
  apiFetch<LayoutData>(
    `/api/v1/orgs/${orgId}/processes/${encodeURIComponent(process_key)}/layout`,
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
