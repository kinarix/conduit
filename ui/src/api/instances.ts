import { apiFetch } from './client'

export function toVariableInputs(
  obj: Record<string, unknown>,
): Array<{ name: string; value_type: string; value: unknown }> {
  return Object.entries(obj).map(([name, value]) => ({
    name,
    value_type:
      typeof value === 'number' ? 'number' : typeof value === 'boolean' ? 'boolean' : 'string',
    value,
  }))
}

export interface ProcessInstance {
  id: string
  org_id: string
  definition_id: string
  state: string
  labels: Record<string, string>
  started_at: string
  ended_at: string | null
  /** Sequential per-process human-friendly identifier (#1, #2, ...). */
  counter: number
}

export const fetchInstances = (org_id: string) =>
  apiFetch<ProcessInstance[]>(`/api/v1/process-instances?org_id=${org_id}`)

/** Paginated, optionally filtered. Returns rows + total. */
export async function fetchInstancesPage(opts: {
  org_id: string
  definition_id?: string
  process_key?: string
  limit?: number
  offset?: number
}): Promise<{ instances: ProcessInstance[]; total: number }> {
  const params = new URLSearchParams({ org_id: opts.org_id })
  if (opts.definition_id) params.set('definition_id', opts.definition_id)
  if (opts.process_key) params.set('process_key', opts.process_key)
  if (opts.limit != null) params.set('limit', String(opts.limit))
  if (opts.offset != null) params.set('offset', String(opts.offset))
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const BASE = ((import.meta as any).env?.VITE_API_URL as string | undefined) ?? ''
  const res = await fetch(`${BASE}/api/v1/process-instances?${params}`)
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  const instances = (await res.json()) as ProcessInstance[]
  const total = Number(res.headers.get('X-Total-Count') ?? instances.length)
  return { instances, total }
}

export const startInstance = (body: {
  org_id: string
  definition_id: string
  variables?: Array<{ name: string; value_type: string; value: unknown }>
}) =>
  apiFetch<ProcessInstance>('/api/v1/process-instances', {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const fetchInstance = (id: string) =>
  apiFetch<ProcessInstance>(`/api/v1/process-instances/${id}`)

export const pauseInstance = (id: string) =>
  apiFetch<ProcessInstance>(`/api/v1/process-instances/${id}/pause`, { method: 'POST' })

export const resumeInstance = (id: string) =>
  apiFetch<ProcessInstance>(`/api/v1/process-instances/${id}/resume`, { method: 'POST' })

export const cancelInstance = (id: string) =>
  apiFetch<ProcessInstance>(`/api/v1/process-instances/${id}/cancel`, { method: 'POST' })

export const deleteInstance = (id: string) =>
  apiFetch<void>(`/api/v1/process-instances/${id}`, { method: 'DELETE' })

export interface ExecutionHistoryEntry {
  id: string
  instance_id: string
  execution_id: string
  element_id: string
  element_type: string
  entered_at: string
  left_at: string | null
  worker_id: string | null
}

export const fetchInstanceHistory = (id: string) =>
  apiFetch<ExecutionHistoryEntry[]>(`/api/v1/process-instances/${id}/history`)

export interface InstanceJob {
  id: string
  instance_id: string
  execution_id: string
  job_type: string
  topic: string | null
  due_date: string
  retries: number
  retry_count: number
  error_message: string | null
  state: string
  created_at: string
  locked_by: string | null
  locked_until: string | null
}

export const fetchInstanceJobs = (id: string) =>
  apiFetch<InstanceJob[]>(`/api/v1/process-instances/${id}/jobs`)
