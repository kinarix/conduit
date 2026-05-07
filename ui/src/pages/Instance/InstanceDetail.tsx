import { useNavigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import {
  fetchInstance,
  fetchInstanceHistory,
  fetchInstanceJobs,
  type InstanceJob,
  type ExecutionHistoryEntry,
  type ProcessInstance,
} from '../../api/instances'
import { fetchDeployment, type ProcessDefinition } from '../../api/deployments'
import { fetchTasks, type Task } from '../../api/tasks'
import { fetchInstanceEvents, type ProcessEvent } from '../../api/events'
import { InstanceActions } from '../../components/InstanceActions'
import InstanceTabs, { type TabSpec } from './InstanceTabs'
import InstanceTimeline from './InstanceTimeline'
import InstanceVariables from './InstanceVariables'
import InstanceDiagram from './InstanceDiagram'
import styles from './InstanceDetail.module.css'

export default function InstanceDetail() {
  const { instanceId = '' } = useParams<{ instanceId: string }>()
  const navigate = useNavigate()

  const instanceQ = useQuery({
    queryKey: ['instance', instanceId],
    queryFn: () => fetchInstance(instanceId),
    enabled: !!instanceId,
    refetchInterval: 5_000,
  })

  const defQ = useQuery({
    queryKey: ['deployment', instanceQ.data?.definition_id],
    queryFn: () => fetchDeployment(instanceQ.data!.definition_id),
    enabled: !!instanceQ.data?.definition_id,
  })

  const tasksQ = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 5_000,
  })

  const historyQ = useQuery({
    queryKey: ['instance-history', instanceId],
    queryFn: () => fetchInstanceHistory(instanceId),
    enabled: !!instanceId,
    refetchInterval: 5_000,
  })

  const jobsQ = useQuery({
    queryKey: ['instance-jobs', instanceId],
    queryFn: () => fetchInstanceJobs(instanceId),
    enabled: !!instanceId,
    refetchInterval: 5_000,
  })

  const eventsQ = useQuery({
    queryKey: ['instance-events', instanceId],
    queryFn: () => fetchInstanceEvents(instanceId),
    enabled: !!instanceId,
    refetchInterval: 5_000,
  })

  if (instanceQ.isLoading) return <div className={styles.page}>Loading…</div>
  if (instanceQ.error)
    return (
      <div className={styles.page}>
        <div className="error-banner">{String(instanceQ.error)}</div>
      </div>
    )
  const instance = instanceQ.data
  if (!instance) return null

  const def = defQ.data
  const activeTasks = (tasksQ.data ?? []).filter(
    t => t.instance_id === instance.id && t.state === 'active',
  )
  const history = historyQ.data ?? []
  const jobs = jobsQ.data ?? []
  const errored = jobs.filter(j => j.error_message || j.state === 'failed')
  const errorEvents = (eventsQ.data ?? []).filter(e => e.event_type === 'error_raised')

  const tabs: TabSpec[] = [
    {
      id: 'diagram',
      label: 'Diagram',
      render: () => <InstanceDiagram instanceId={instance.id} />,
    },
    {
      id: 'timeline',
      label: 'Timeline',
      render: () => <InstanceTimeline instanceId={instance.id} />,
    },
    {
      id: 'variables',
      label: 'Variables',
      render: () => <InstanceVariables instanceId={instance.id} />,
    },
    {
      id: 'tasks',
      label: 'Tasks',
      count: activeTasks.length,
      render: () => <TasksTab tasks={activeTasks} />,
    },
    {
      id: 'errors',
      label: 'Errors',
      count: errored.length + errorEvents.length,
      errorBadge: errored.length + errorEvents.length > 0,
      render: () => <ErrorsTab jobs={errored} errorEvents={errorEvents} />,
    },
    {
      id: 'history',
      label: 'History',
      count: history.length,
      render: () => <HistoryTab history={history} />,
    },
  ]

  return (
    <div className={styles.page}>
      <Header instance={instance} def={def} onBack={() => navigate(-1)} />
      <InstanceTabs tabs={tabs} defaultTabId="diagram" />
    </div>
  )
}

function Header({
  instance,
  def,
  onBack,
}: {
  instance: ProcessInstance
  def: ProcessDefinition | undefined
  onBack: () => void
}) {
  return (
    <header className={styles.header}>
      <div style={{ minWidth: 0, flex: 1 }}>
        <h1 className={styles.title}>
          {def?.name || def?.process_key || 'Instance'}
          <span style={{ fontWeight: 400, color: 'var(--text-tertiary)', marginLeft: 8 }}>#{instance.counter}</span>
        </h1>
        <div className={styles.metaRow}>
          {def && (
            <span style={{ fontFamily: 'var(--font-mono)' }}>
              {def.process_key} v{def.version}
            </span>
          )}
          <StateBadge state={instance.state} />
          <span title={new Date(instance.started_at).toLocaleString()}>
            started {new Date(instance.started_at).toLocaleString()}
          </span>
          {instance.ended_at && (
            <span title={new Date(instance.ended_at).toLocaleString()}>
              ended {new Date(instance.ended_at).toLocaleString()}
            </span>
          )}
        </div>
        <div className={styles.idCell}>{instance.id}</div>
        {Object.keys(instance.labels || {}).length > 0 && (
          <div className={styles.section}>
            <div className={styles.labels}>
              {Object.entries(instance.labels).map(([k, v]) => (
                <span key={k} className={styles.labelChip}>
                  {k}: {v}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
      <InstanceActions instance={instance} variant="buttons" onDeleted={onBack} />
    </header>
  )
}

function TasksTab({ tasks }: { tasks: Task[] }) {
  if (tasks.length === 0) {
    return <div className={styles.empty}>No active tasks.</div>
  }
  return (
    <table className={styles.tasksTable}>
      <thead>
        <tr>
          <th>Name</th>
          <th>Type</th>
          <th>Assignee</th>
          <th>Created</th>
        </tr>
      </thead>
      <tbody>
        {tasks.map(t => (
          <tr key={t.id}>
            <td>{t.name || t.element_id}</td>
            <td>
              <code>{t.task_type}</code>
            </td>
            <td>{t.assignee ?? '—'}</td>
            <td>{new Date(t.created_at).toLocaleString()}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function ErrorsTab({ jobs, errorEvents }: { jobs: InstanceJob[]; errorEvents: ProcessEvent[] }) {
  if (jobs.length === 0 && errorEvents.length === 0) {
    return <div className={styles.empty}>No errors.</div>
  }
  return (
    <div>
      {errorEvents.map(e => (
        <div key={e.id} className={styles.errorCard}>
          <div className={styles.errorHead}>
            <code>{e.element_id ?? 'engine'}</code>
            {e.payload.error_code != null && (
              <code style={{ color: 'var(--text-tertiary)' }}>
                {String(e.payload.error_code)}
              </code>
            )}
            <span style={{ marginLeft: 'auto', color: 'var(--text-tertiary)' }}>
              {new Date(e.occurred_at).toLocaleString()}
            </span>
          </div>
          {e.payload.message != null && (
            <pre className={styles.errorPre}>{String(e.payload.message)}</pre>
          )}
        </div>
      ))}
      {jobs.map(j => (
        <div key={j.id} className={styles.errorCard}>
          <div className={styles.errorHead}>
            <code>{j.job_type}</code>
            {j.topic && (
              <code style={{ color: 'var(--text-tertiary)' }}>{j.topic}</code>
            )}
            <span style={{ marginLeft: 'auto', color: 'var(--text-tertiary)' }}>
              {j.retry_count}/{j.retries} retries · {new Date(j.created_at).toLocaleString()}
            </span>
          </div>
          {j.error_message && <pre className={styles.errorPre}>{j.error_message}</pre>}
        </div>
      ))}
    </div>
  )
}

function HistoryTab({ history }: { history: ExecutionHistoryEntry[] }) {
  if (history.length === 0) {
    return <div className={styles.empty}>No history yet.</div>
  }
  return (
    <table className={styles.historyTable}>
      <thead>
        <tr>
          <th>Element</th>
          <th>Type</th>
          <th>Entered</th>
          <th>Left</th>
          <th>Duration</th>
          <th>Worker</th>
        </tr>
      </thead>
      <tbody>
        {history.map(h => (
          <tr key={h.id}>
            <td>
              <code>{h.element_id}</code>
            </td>
            <td>
              <code style={{ color: 'var(--text-tertiary)' }}>{h.element_type}</code>
            </td>
            <td>{new Date(h.entered_at).toLocaleString()}</td>
            <td>{h.left_at ? new Date(h.left_at).toLocaleString() : '—'}</td>
            <td style={{ color: 'var(--text-tertiary)' }}>
              {formatDuration(h.entered_at, h.left_at)}
            </td>
            <td style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-tertiary)' }}>
              {h.worker_id ?? '—'}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function formatDuration(start: string, end: string | null): string {
  const s = new Date(start).getTime()
  const e = end ? new Date(end).getTime() : Date.now()
  const ms = e - s
  if (ms < 1000) return `${ms}ms`
  const secs = Math.round(ms / 1000)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  const r = secs - mins * 60
  return r ? `${mins}m ${r}s` : `${mins}m`
}

function StateBadge({ state }: { state: string }) {
  const palette: Record<string, { bg: string; fg: string }> = {
    running: { bg: 'rgba(34,197,94,0.15)', fg: '#16a34a' },
    suspended: { bg: 'rgba(245,158,11,0.15)', fg: '#b45309' },
    completed: { bg: 'rgba(148,163,184,0.20)', fg: '#475569' },
    cancelled: { bg: 'rgba(220,38,38,0.12)', fg: '#b91c1c' },
    error: { bg: 'rgba(220,38,38,0.18)', fg: '#991b1b' },
  }
  const c = palette[state] || palette.completed
  return (
    <span
      style={{
        fontSize: 10,
        fontWeight: 700,
        letterSpacing: '0.05em',
        textTransform: 'uppercase',
        background: c.bg,
        color: c.fg,
        padding: '2px 8px',
        borderRadius: 'var(--radius-sm)',
      }}
    >
      {state}
    </span>
  )
}
