import { apiFetch } from './client'

export type EventType =
  | 'variable_set'
  | 'variable_changed'
  | 'job_created'
  | 'job_locked'
  | 'job_completed'
  | 'job_failed'
  | 'job_cancelled'
  | 'element_entered'
  | 'element_left'
  | 'message_received'
  | 'signal_received'
  | 'error_raised'
  | 'error_caught'
  | string // forward-compatible — server may emit new types

export interface ProcessEvent {
  id: string
  instance_id: string
  execution_id: string | null
  event_type: EventType
  element_id: string | null
  occurred_at: string
  payload: Record<string, unknown>
  metadata: Record<string, unknown>
}

export const fetchInstanceEvents = (orgId: string, id: string) =>
  apiFetch<ProcessEvent[]>(`/api/v1/orgs/${orgId}/process-instances/${id}/events`)
