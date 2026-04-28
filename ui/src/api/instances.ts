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
}

export const fetchInstances = (org_id: string) =>
  apiFetch<ProcessInstance[]>(`/api/v1/process-instances?org_id=${org_id}`)

export const startInstance = (body: {
  org_id: string
  definition_id: string
  variables?: Array<{ name: string; value_type: string; value: unknown }>
}) =>
  apiFetch<ProcessInstance>('/api/v1/process-instances', {
    method: 'POST',
    body: JSON.stringify(body),
  })
