import { useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import {
  fetchDeployments,
  groupByProcessKey,
  type ProcessDefinition,
  type LogicalProcess,
} from '../../api/deployments'
import { fetchProcessGroups } from '../../api/processGroups'
import { fetchInstances, type ProcessInstance } from '../../api/instances'
import { fetchTasks } from '../../api/tasks'
import { useOrg } from '../../App'
import StartInstancePanel from './StartInstancePanel'
import {
  bucketThroughput,
  bucketElapsed,
  chooseBucket,
  formatDurationSec,
  type Bucket,
} from './charts/chartUtils'
import ThroughputChart from './charts/ThroughputChart'
import ElapsedTimeChart from './charts/ElapsedTimeChart'
import ErrorRateChart from './charts/ErrorRateChart'
import styles from './ProcessDashboard.module.css'

const ERROR_STATES = new Set(['error', 'failed'])

const WINDOW_OPTIONS: { id: string; label: string; ms: number }[] = [
  { id: '24h', label: '24 hours', ms: 24 * 60 * 60 * 1000 },
  { id: '7d', label: '7 days', ms: 7 * 24 * 60 * 60 * 1000 },
  { id: '30d', label: '30 days', ms: 30 * 24 * 60 * 60 * 1000 },
]

export default function ProcessDashboard() {
  const { groupId = '', processKey = '' } = useParams<{ groupId: string; processKey: string }>()
  const navigate = useNavigate()
  const { org } = useOrg()

  const decodedKey = decodeURIComponent(processKey)

  const groupsQ = useQuery({
    queryKey: ['process-groups', org?.id],
    queryFn: () => fetchProcessGroups(org!.id),
    enabled: !!org,
  })

  const defsQ = useQuery({
    queryKey: ['deployments', org?.id],
    queryFn: () => fetchDeployments(org!.id),
    enabled: !!org,
  })

  const instancesQ = useQuery({
    queryKey: ['instances', org?.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  const tasksQ = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 5_000,
  })

  const proc: LogicalProcess | null = useMemo(() => {
    const defs = defsQ.data ?? []
    const inGroup = defs.filter(d => d.process_group_id === groupId && d.process_key === decodedKey)
    if (inGroup.length === 0) return null
    return groupByProcessKey(inGroup)[0]
  }, [defsQ.data, groupId, decodedKey])

  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(null)
  const [windowOpt, setWindowOpt] = useState(WINDOW_OPTIONS[1])

  const selectedVersion: ProcessDefinition | null = useMemo(() => {
    if (!proc) return null
    if (selectedVersionId) {
      const v = proc.versions.find(x => x.id === selectedVersionId)
      if (v) return v
    }
    return proc.latest
  }, [proc, selectedVersionId])

  const groupName = (groupsQ.data ?? []).find(g => g.id === groupId)?.name ?? '…'

  if (!org) return <div className={styles.empty}>Pick an organisation to start.</div>
  if (defsQ.isLoading) return <div className={styles.empty}>Loading…</div>
  if (!proc) return <ProcessNotFound onBack={() => navigate(`/process-groups/${groupId}`)} />

  const allInstances = instancesQ.data ?? []
  const versionInstances = allInstances.filter(i => i.definition_id === selectedVersion!.id)
  const allVersionIds = new Set(proc.versions.map(v => v.id))
  const aggregateInstances = allInstances.filter(i => allVersionIds.has(i.definition_id))
  const versionTasks = (tasksQ.data ?? []).filter(t => {
    const inst = aggregateInstances.find(i => i.id === t.instance_id)
    return inst && inst.definition_id === selectedVersion!.id && t.state === 'active'
  })

  return (
    <div className={styles.page}>
      <Header
        proc={proc}
        groupId={groupId}
        groupName={groupName}
        onBack={() => navigate(`/process-groups/${groupId}`)}
        onNavigateGroup={() => navigate(`/process-groups/${groupId}`)}
      />

      <AggregateKpis instances={aggregateInstances} versionCount={proc.versions.length} />

      <Section title="Versions">
        <div className={styles.versions}>
          {proc.versions.map(v => (
            <VersionCard
              key={v.id}
              version={v}
              instanceCount={allInstances.filter(i => i.definition_id === v.id).length}
              selected={selectedVersion!.id === v.id}
              onSelect={() => setSelectedVersionId(v.id)}
              onEdit={() => navigate(`/definitions/${v.id}/edit`)}
            />
          ))}
        </div>
      </Section>

      <Section
        title={`Selected: v${selectedVersion!.version} (${selectedVersion!.status})`}
        right={
          <div className={styles.bucketToggle}>
            {WINDOW_OPTIONS.map(w => (
              <button
                key={w.id}
                type="button"
                className={windowOpt.id === w.id ? styles.active : ''}
                onClick={() => setWindowOpt(w)}
              >
                {w.label}
              </button>
            ))}
          </div>
        }
      >
        <VersionKpis instances={versionInstances} tasks={versionTasks.length} />

        <div className={styles.chartsGrid}>
          <ChartCard title="Throughput">
            <ThroughputChart
              data={bucketThroughput(versionInstances, chooseBucket(windowOpt.ms), windowOpt.ms)}
            />
          </ChartCard>
          <ChartCard title="Elapsed time (P50 / P95 / P99)">
            <ElapsedTimeChart
              data={bucketElapsed(versionInstances, chooseBucket(windowOpt.ms), windowOpt.ms)}
            />
          </ChartCard>
          <ChartCard title="Outcomes">
            <ErrorRateChart
              data={bucketThroughput(versionInstances, chooseBucket(windowOpt.ms), windowOpt.ms)}
            />
          </ChartCard>
        </div>
      </Section>

      <Section title={`Recent instances (v${selectedVersion!.version})`}>
        <RecentInstances instances={versionInstances.slice(0, 50)} />
      </Section>

      <StartInstanceLauncher org={org.id} version={selectedVersion!} />
    </div>
  )
}

/* ── Sub-components ─────────────────────────────────────────────────── */

function Header({
  proc,
  groupId,
  groupName,
  onBack,
  onNavigateGroup,
}: {
  proc: LogicalProcess
  groupId: string
  groupName: string
  onBack: () => void
  onNavigateGroup: () => void
}) {
  void groupId
  void onBack
  return (
    <header className={styles.header}>
      <div style={{ minWidth: 0, flex: 1 }}>
        <h1 className={styles.title}>{proc.displayName}</h1>
        <div className={styles.subtitle}>
          <span className={styles.crumbLink} onClick={onNavigateGroup}>{groupName}</span>
          <span style={{ color: 'var(--text-tertiary)' }}>›</span>
          <span className={styles.processKey}>{proc.key}</span>
          <span>·</span>
          <span>{proc.versions.length} {proc.versions.length === 1 ? 'version' : 'versions'}</span>
          {proc.hasDraft && <span style={{ color: 'var(--status-warn)' }}>has draft</span>}
        </div>
      </div>
    </header>
  )
}

function AggregateKpis({
  instances,
  versionCount,
}: {
  instances: ProcessInstance[]
  versionCount: number
}) {
  const running = instances.filter(i => i.state === 'running').length
  const completed = instances.filter(i => i.state === 'completed').length
  const errored = instances.filter(i => ERROR_STATES.has(i.state)).length
  return (
    <div className={styles.kpiRow}>
      <Kpi label="Versions" value={versionCount} tone="info" />
      <Kpi label="Total instances" value={instances.length} />
      <Kpi label="Running" value={running} tone={running > 0 ? 'info' : undefined} />
      <Kpi label="Completed" value={completed} tone="ok" />
      <Kpi label="Errored" value={errored} tone={errored > 0 ? 'error' : undefined} />
    </div>
  )
}

function VersionKpis({ instances, tasks }: { instances: ProcessInstance[]; tasks: number }) {
  const running = instances.filter(i => i.state === 'running').length
  const completed = instances.filter(i => i.state === 'completed').length
  const errored = instances.filter(i => ERROR_STATES.has(i.state)).length
  const cancelled = instances.filter(i => i.state === 'cancelled').length

  // P50 of elapsed time among completed instances.
  const elapsed = instances
    .filter(i => i.ended_at)
    .map(i => (new Date(i.ended_at!).getTime() - new Date(i.started_at).getTime()) / 1000)
    .sort((a, b) => a - b)
  const p50 = elapsed.length ? elapsed[Math.floor(elapsed.length / 2)] : null

  return (
    <div className={styles.kpiRow}>
      <Kpi label="Instances" value={instances.length} />
      <Kpi label="Running" value={running} tone={running > 0 ? 'info' : undefined} />
      <Kpi label="Completed" value={completed} tone="ok" />
      <Kpi label="Errored" value={errored} tone={errored > 0 ? 'error' : undefined} />
      <Kpi label="Cancelled" value={cancelled} />
      <Kpi label="Active tasks" value={tasks} tone={tasks > 0 ? 'warn' : undefined} />
      <Kpi label="P50 elapsed" value={formatDurationSec(p50)} tone="info" />
    </div>
  )
}

function Kpi({
  label,
  value,
  tone,
}: {
  label: string
  value: number | string
  tone?: 'ok' | 'warn' | 'error' | 'info'
}) {
  return (
    <div className={`${styles.kpi} ${tone ? styles[tone] : ''}`}>
      <div className={styles.kpiLabel}>{label}</div>
      <div className={styles.kpiValue}>{value}</div>
    </div>
  )
}

function VersionCard({
  version,
  instanceCount,
  selected,
  onSelect,
  onEdit,
}: {
  version: ProcessDefinition
  instanceCount: number
  selected: boolean
  onSelect: () => void
  onEdit: () => void
}) {
  return (
    <div
      className={`${styles.versionCard} ${selected ? styles.selected : ''} ${
        version.status === 'draft' ? styles.draft : ''
      }`}
      onClick={onSelect}
    >
      <div className={styles.versionLabel}>
        <span>v{version.version}</span>
        {version.status === 'draft' ? (
          <span className={styles.draftPill}>Draft</span>
        ) : (
          <span className={`${styles.statusPill} ${styles.deployed}`}>Deployed</span>
        )}
      </div>
      <div className={styles.versionMeta}>
        {new Date(version.deployed_at).toLocaleDateString()} · {instanceCount}{' '}
        {instanceCount === 1 ? 'instance' : 'instances'}
      </div>
      <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
        <a
          className={styles.crumbLink}
          style={{ fontSize: 11 }}
          onClick={e => { e.stopPropagation(); onEdit() }}
        >
          Open in modeller
        </a>
      </div>
    </div>
  )
}

function Section({
  title,
  right,
  children,
}: {
  title: string
  right?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className={styles.section}>
      <div className={styles.sectionHead}>
        <h2 className={styles.sectionTitle}>{title}</h2>
        {right}
      </div>
      {children}
    </section>
  )
}

function ChartCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className={styles.chartCard}>
      <div className={styles.chartCardTitle}>{title}</div>
      {children}
    </div>
  )
}

function RecentInstances({ instances }: { instances: ProcessInstance[] }) {
  const navigate = useNavigate()
  if (instances.length === 0) {
    return <div className={`${styles.tableWrap} ${styles.empty}`}>No instances yet for this version.</div>
  }
  return (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <thead>
          <tr>
            <th>ID</th>
            <th>State</th>
            <th>Started</th>
            <th>Ended</th>
            <th>Duration</th>
          </tr>
        </thead>
        <tbody>
          {instances.map(i => (
            <tr key={i.id} onClick={() => navigate(`/instances/${i.id}`)}>
              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{i.id.slice(0, 8)}…</td>
              <td>
                <StateBadge state={i.state} />
              </td>
              <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                {new Date(i.started_at).toLocaleString()}
              </td>
              <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                {i.ended_at ? new Date(i.ended_at).toLocaleString() : '—'}
              </td>
              <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
                {i.ended_at
                  ? formatDurationSec((new Date(i.ended_at).getTime() - new Date(i.started_at).getTime()) / 1000)
                  : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function StateBadge({ state }: { state: string }) {
  const cls =
    state === 'running' ? 'badge-running' :
    state === 'completed' ? 'badge-completed' :
    ERROR_STATES.has(state) ? 'badge-error' :
    'badge-active'
  return <span className={`badge ${cls}`}>{state}</span>
}

function ProcessNotFound({ onBack }: { onBack: () => void }) {
  return (
    <div className={styles.page}>
      <div className={styles.empty}>
        Process not found in this group.{' '}
        <a className={styles.crumbLink} onClick={onBack}>Back to group</a>
      </div>
    </div>
  )
}

function StartInstanceLauncher({ org, version }: { org: string; version: ProcessDefinition }) {
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)

  return (
    <>
      <div style={{ position: 'fixed', right: 24, bottom: 24, zIndex: 30 }}>
        <button
          className="btn-primary"
          disabled={version.status !== 'deployed'}
          title={version.status !== 'deployed' ? 'Deploy this version before starting' : ''}
          onClick={() => setOpen(true)}
          style={{ padding: '10px 20px', fontSize: 14, boxShadow: 'var(--shadow-md)' }}
        >
          ▶ Start instance (v{version.version})
        </button>
      </div>
      {open && (
        <StartInstancePanel
          org={org}
          version={version}
          onClose={() => setOpen(false)}
          onStarted={id => navigate(`/instances/${id}`)}
        />
      )}
    </>
  )
}
