import { useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { useOrg } from '../App'
import { fetchInstances, type ProcessInstance } from '../api/instances'
import { fetchDeployments, groupByProcessKey, type ProcessDefinition } from '../api/deployments'
import { fetchTasks, type Task } from '../api/tasks'
import { KpiCard, Panel } from './DashboardWidgets'
import styles from './Dashboard.module.css'

const ERROR_STATES = new Set(['error', 'failed'])

function fmt(d: string) {
  return new Date(d).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
}

function defName(defs: ProcessDefinition[], defId: string) {
  const d = defs.find(x => x.id === defId)
  return d ? (d.name ?? d.process_key) : defId
}

export default function OrgDashboard() {
  const { org } = useOrg()
  const navigate = useNavigate()

  const instancesQ = useQuery({
    queryKey: ['instances', org?.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })
  const deploymentsQ = useQuery({
    queryKey: ['deployments', org?.id],
    queryFn: () => fetchDeployments(org!.id),
    enabled: !!org,
  })
  const tasksQ = useQuery({
    queryKey: ['tasks', org?.id],
    queryFn: () => fetchTasks(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  if (!org) {
    return <div className={styles.placeholder}>Pick an organisation to get started.</div>
  }

  if (instancesQ.isLoading || deploymentsQ.isLoading || tasksQ.isLoading) {
    return <div className={styles.spinner}><span className="spinner" /></div>
  }

  const instances = instancesQ.data ?? []
  const defs = deploymentsQ.data ?? []
  const tasks = tasksQ.data ?? []

  const running = instances.filter(i => i.state === 'running')
  const errors = instances.filter(i => ERROR_STATES.has(i.state))
  const openTasks = tasks.filter((t: Task) => t.state === 'active')
  const deployed = defs.filter(d => d.status === 'deployed')
  const recent = [...instances].sort(
    (a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime()
  ).slice(0, 10)

  const processes = groupByProcessKey(deployed)

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1 className={styles.title}>{org.name}</h1>
        <div className={styles.subtitle}>Organisation overview</div>
      </div>

      <div className={styles.kpiRow}>
        <KpiCard label="Running instances" value={running.length} tone={running.length > 0 ? 'info' : undefined} />
        <KpiCard label="Error instances" value={errors.length} tone={errors.length > 0 ? 'error' : undefined} />
        <KpiCard label="Open tasks" value={openTasks.length} tone={openTasks.length > 0 ? 'info' : undefined} />
        <KpiCard label="Deployed processes" value={processes.length} />
      </div>

      <div className={styles.panelsGrid}>
        <ErrorInstancesPanel instances={errors.slice(0, 5)} defs={defs} onNavigate={navigate} onViewAll={() => navigate('/instances')} />
        <OpenTasksPanel tasks={openTasks.slice(0, 5)} defs={defs} instances={instances} onViewAll={() => navigate('/tasks')} />
        <RecentInstancesPanel instances={recent} defs={defs} onNavigate={navigate} onViewAll={() => navigate('/instances')} />
        <ProcessesPanel processes={processes} instances={instances} onNavigate={navigate} />
      </div>
    </div>
  )
}

function ErrorInstancesPanel({
  instances,
  defs,
  onNavigate,
  onViewAll,
}: {
  instances: ProcessInstance[]
  defs: ProcessDefinition[]
  onNavigate: (path: string) => void
  onViewAll: () => void
}) {
  return (
    <Panel title={`Error instances (${instances.length})`} onViewAll={instances.length > 0 ? onViewAll : undefined}>
      {instances.length === 0 ? (
        <div className={styles.empty}>No errors — looking good</div>
      ) : (
        <table className={styles.panelTable}>
          <thead>
            <tr><th>Process</th><th>Instance</th><th>Started</th></tr>
          </thead>
          <tbody>
            {instances.map(i => (
              <tr key={i.id} onClick={() => onNavigate(`/instances/${i.id}`)}>
                <td>{defName(defs, i.definition_id)}</td>
                <td><span className={styles.mono}>#{i.counter}</span></td>
                <td className={styles.muted}>{fmt(i.started_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}

function OpenTasksPanel({
  tasks,
  defs,
  instances,
  onViewAll,
}: {
  tasks: Task[]
  defs: ProcessDefinition[]
  instances: ProcessInstance[]
  onViewAll: () => void
}) {
  function procName(instanceId: string) {
    const inst = instances.find(i => i.id === instanceId)
    if (!inst) return '—'
    return defName(defs, inst.definition_id)
  }

  return (
    <Panel title={`Open tasks (${tasks.length})`} onViewAll={tasks.length > 0 ? onViewAll : undefined}>
      {tasks.length === 0 ? (
        <div className={styles.empty}>No open tasks</div>
      ) : (
        <table className={styles.panelTable}>
          <thead>
            <tr><th>Task</th><th>Process</th><th>Assignee</th></tr>
          </thead>
          <tbody>
            {tasks.map(t => (
              <tr key={t.id}>
                <td>{t.name ?? t.element_id}</td>
                <td className={styles.muted}>{procName(t.instance_id)}</td>
                <td className={t.assignee ? '' : styles.muted}>{t.assignee ?? 'Unassigned'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}

function RecentInstancesPanel({
  instances,
  defs,
  onNavigate,
  onViewAll,
}: {
  instances: ProcessInstance[]
  defs: ProcessDefinition[]
  onNavigate: (path: string) => void
  onViewAll: () => void
}) {
  function badgeCls(state: string) {
    if (state === 'running') return 'badge badge-running'
    if (state === 'completed') return 'badge badge-completed'
    if (ERROR_STATES.has(state)) return 'badge badge-error'
    return 'badge'
  }

  return (
    <Panel title="Recent instances" onViewAll={instances.length > 0 ? onViewAll : undefined}>
      {instances.length === 0 ? (
        <div className={styles.empty}>No instances yet</div>
      ) : (
        <table className={styles.panelTable}>
          <thead>
            <tr><th>Process</th><th>Status</th><th>Started</th></tr>
          </thead>
          <tbody>
            {instances.map(i => (
              <tr key={i.id} onClick={() => onNavigate(`/instances/${i.id}`)}>
                <td>{defName(defs, i.definition_id)}</td>
                <td><span className={badgeCls(i.state)}>{i.state}</span></td>
                <td className={styles.muted}>{fmt(i.started_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}

function ProcessesPanel({
  processes,
  instances,
  onNavigate,
}: {
  processes: ReturnType<typeof groupByProcessKey>
  instances: ProcessInstance[]
  onNavigate: (path: string) => void
}) {
  const rows = processes.slice(0, 5)

  function counts(proc: ReturnType<typeof groupByProcessKey>[number]) {
    const defIds = new Set(proc.versions.map(v => v.id))
    const inst = instances.filter(i => defIds.has(i.definition_id))
    return {
      running: inst.filter(i => i.state === 'running').length,
      errors: inst.filter(i => ERROR_STATES.has(i.state)).length,
    }
  }

  return (
    <Panel title={`Processes (${processes.length})`}>
      {rows.length === 0 ? (
        <div className={styles.empty}>No deployed processes</div>
      ) : (
        <table className={styles.panelTable}>
          <thead>
            <tr><th>Name</th><th>Running</th><th>Errors</th></tr>
          </thead>
          <tbody>
            {rows.map(proc => {
              const { running, errors } = counts(proc)
              return (
                <tr key={proc.key} onClick={() => onNavigate(`/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`)}>
                  <td>{proc.displayName}</td>
                  <td>{running > 0 ? <span style={{ color: 'var(--accent)' }}>{running}</span> : <span className={styles.muted}>0</span>}</td>
                  <td>{errors > 0 ? <span style={{ color: 'var(--status-error)' }}>{errors}</span> : <span className={styles.muted}>0</span>}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}
    </Panel>
  )
}
