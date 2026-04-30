import type { ProcessEvent } from '../../api/events'

export type EventCategory = 'variable' | 'job' | 'element' | 'message' | 'error' | 'other'

export interface FormattedEvent {
  category: EventCategory
  icon: string
  /** One-line summary, may include simple HTML-free markup. */
  title: string
  /** Optional secondary line — short context. */
  subtitle?: string
  /** Free-form expandable detail (shown when the row is expanded). */
  detail?: Record<string, unknown>
  /** Tone affects the row colour bar. */
  tone: 'neutral' | 'ok' | 'warn' | 'error'
}

function asString(v: unknown): string {
  if (v == null) return '—'
  if (typeof v === 'string') return v
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

export function formatEvent(e: ProcessEvent): FormattedEvent {
  switch (e.event_type) {
    case 'variable_set': {
      const name = asString(e.payload.name)
      const value = asString(e.payload.new_value)
      return {
        category: 'variable',
        icon: '＋',
        title: `Set ${name} = ${truncate(value, 80)}`,
        subtitle: e.element_id ? `at ${e.element_id}` : 'instance start',
        detail: e.payload,
        tone: 'neutral',
      }
    }
    case 'variable_changed': {
      const name = asString(e.payload.name)
      const oldV = asString(e.payload.old_value)
      const newV = asString(e.payload.new_value)
      return {
        category: 'variable',
        icon: '↻',
        title: `${name}: ${truncate(oldV, 30)} → ${truncate(newV, 30)}`,
        subtitle: e.element_id ? `at ${e.element_id}` : undefined,
        detail: e.payload,
        tone: 'neutral',
      }
    }
    case 'job_created': {
      const jobType = asString(e.payload.job_type)
      return {
        category: 'job',
        icon: '⚙',
        title: `Job created — ${jobType}`,
        subtitle: e.element_id ?? undefined,
        detail: { ...e.payload, ...e.metadata },
        tone: 'neutral',
      }
    }
    case 'job_locked': {
      const worker = asString(e.metadata.worker_id ?? '')
      return {
        category: 'job',
        icon: '🔒',
        title: `Job locked${worker && worker !== '—' ? ` by ${worker}` : ''}`,
        subtitle: asString(e.payload.job_type),
        detail: { ...e.payload, ...e.metadata },
        tone: 'neutral',
      }
    }
    case 'job_completed':
      return {
        category: 'job',
        icon: '✓',
        title: `Job completed — ${asString(e.payload.job_type)}`,
        detail: { ...e.payload, ...e.metadata },
        tone: 'ok',
      }
    case 'job_failed':
      return {
        category: 'job',
        icon: '✗',
        title: `Job failed — ${asString(e.payload.job_type)}`,
        subtitle: asString(e.metadata.error_message ?? e.metadata.error_code ?? ''),
        detail: { ...e.payload, ...e.metadata },
        tone: 'error',
      }
    case 'job_cancelled':
      return {
        category: 'job',
        icon: '⊘',
        title: `Job cancelled — ${asString(e.payload.job_type)}`,
        subtitle: asString(e.metadata.reason ?? ''),
        detail: { ...e.payload, ...e.metadata },
        tone: 'warn',
      }
    case 'element_entered': {
      const elType = asString(e.payload.element_type)
      return {
        category: 'element',
        icon: '▶',
        title: `Entered ${e.element_id ?? '(unknown)'}`,
        subtitle: elType,
        detail: e.payload,
        tone: 'neutral',
      }
    }
    case 'element_left': {
      const elType = asString(e.payload.element_type)
      return {
        category: 'element',
        icon: '◀',
        title: `Left ${e.element_id ?? '(unknown)'}`,
        subtitle: elType,
        detail: e.payload,
        tone: 'neutral',
      }
    }
    case 'message_received': {
      return {
        category: 'message',
        icon: '✉',
        title: `Message received — ${asString(e.payload.name)}`,
        subtitle: e.metadata.correlation_key
          ? `key: ${asString(e.metadata.correlation_key)}`
          : undefined,
        detail: { ...e.payload, ...e.metadata },
        tone: 'neutral',
      }
    }
    case 'signal_received': {
      return {
        category: 'message',
        icon: '⚑',
        title: `Signal received — ${asString(e.payload.name)}`,
        detail: { ...e.payload, ...e.metadata },
        tone: 'neutral',
      }
    }
    case 'error_raised':
      return {
        category: 'error',
        icon: '!',
        title: `Error raised — ${asString(e.payload.error_code ?? 'unknown')}`,
        subtitle: asString(e.payload.message ?? ''),
        detail: { ...e.payload, ...e.metadata },
        tone: 'error',
      }
    case 'error_caught':
      return {
        category: 'error',
        icon: '✓!',
        title: `Error caught — ${asString(e.payload.error_code ?? 'unknown')}`,
        subtitle: e.element_id ? `by ${e.element_id}` : undefined,
        detail: { ...e.payload, ...e.metadata },
        tone: 'warn',
      }
    default:
      return {
        category: 'other',
        icon: '·',
        title: e.event_type,
        subtitle: e.element_id ?? undefined,
        detail: { ...e.payload, ...e.metadata },
        tone: 'neutral',
      }
  }
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + '…' : s
}

export const ALL_CATEGORIES: { id: EventCategory | 'all'; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'variable', label: 'Variables' },
  { id: 'job', label: 'Jobs' },
  { id: 'element', label: 'Elements' },
  { id: 'message', label: 'Messages' },
  { id: 'error', label: 'Errors' },
]
