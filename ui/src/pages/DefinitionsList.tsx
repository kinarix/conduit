import { Link, useNavigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchDeployments, groupByProcessKey, type LogicalProcess } from '../api/deployments'
import { fetchProcessGroups } from '../api/processGroups'
import { fetchInstances, type ProcessInstance } from '../api/instances'
import { useOrg } from '../App'

const STATE_CLASS: Record<string, string> = {
  running: 'badge-running',
  completed: 'badge-completed',
  error: 'badge-error',
  failed: 'badge-error',
}

export default function DefinitionsList() {
  const { org } = useOrg()
  const { groupId } = useParams<{ groupId?: string }>()
  const navigate = useNavigate()

  const { data: allDefs = [], isLoading } = useQuery({
    queryKey: ['deployments', org?.id],
    queryFn: () => fetchDeployments(org!.id),
    enabled: !!org,
  })

  const { data: groups = [] } = useQuery({
    queryKey: ['process-groups', org?.id],
    queryFn: () => fetchProcessGroups(org!.id),
    enabled: !!org,
  })

  const { data: allInstances = [] } = useQuery({
    queryKey: ['instances', org?.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  const groupDefs = groupId ? allDefs.filter(d => d.process_group_id === groupId) : allDefs
  const processes = groupByProcessKey(groupDefs)

  const groupName = groupId ? groups.find(g => g.id === groupId)?.name ?? '…' : 'Processes'

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div style={{ padding: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
        <h1 style={{ fontSize: 18, fontWeight: 600 }}>{groupName}</h1>
        {groupId ? (
          <Link to={`/process-groups/${groupId}/definitions/new`}>
            <button className="btn-primary">+ New process</button>
          </Link>
        ) : (
          <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
            Open a process group to add processes
          </span>
        )}
      </div>

      {processes.length === 0 ? (
        <div className="empty-state">
          <p>No processes yet.{groupId && <> Click <strong>+ New process</strong> to design and deploy.</>}</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Key</th>
              <th>Versions</th>
              <th>Latest</th>
              <th>Instances</th>
              <th>Updated</th>
            </tr>
          </thead>
          <tbody>
            {processes.map(proc => (
              <ProcessRow
                key={`${proc.groupId}::${proc.key}`}
                proc={proc}
                instances={allInstances.filter(i =>
                  proc.versions.some(v => v.id === i.definition_id),
                )}
                onClick={() =>
                  navigate(`/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`)
                }
              />
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}

function ProcessRow({
  proc,
  instances,
  onClick,
}: {
  proc: LogicalProcess
  instances: ProcessInstance[]
  onClick: () => void
}) {
  const running = instances.filter(i => i.state === 'running').length
  const errored = instances.filter(i => i.state === 'error' || (i.state as string) === 'failed').length
  return (
    <tr style={{ cursor: 'pointer' }} onClick={onClick}>
      <td>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ fontWeight: 500 }}>{proc.displayName}</span>
          {proc.hasDraft && (
            <span
              style={{
                fontSize: 10,
                padding: '1px 6px',
                borderRadius: 'var(--radius-pill)',
                background: 'var(--status-warn-soft)',
                color: 'var(--status-warn)',
                fontWeight: 600,
              }}
            >
              has draft
            </span>
          )}
        </div>
      </td>
      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-secondary)' }}>
        {proc.key}
      </td>
      <td>
        <span
          style={{
            fontSize: 11,
            padding: '1px 7px',
            borderRadius: 'var(--radius-pill)',
            background: 'var(--bg-tertiary)',
            color: 'var(--text-secondary)',
            fontWeight: 600,
          }}
        >
          {proc.versions.length}
        </span>
      </td>
      <td>
        {proc.latestDeployed ? (
          <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
            v{proc.latestDeployed.version}
          </span>
        ) : (
          <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>draft only</span>
        )}
      </td>
      <td>
        <div style={{ display: 'flex', gap: 6 }}>
          <span className={`badge ${STATE_CLASS.running}`}>{running} running</span>
          {errored > 0 && <span className={`badge ${STATE_CLASS.error}`}>{errored} errored</span>}
          <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>· {instances.length} total</span>
        </div>
      </td>
      <td style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>
        {new Date(proc.latest.deployed_at).toLocaleString()}
      </td>
    </tr>
  )
}
