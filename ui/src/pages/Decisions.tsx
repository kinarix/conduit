import { useMemo } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { ReactFlow, Background, BackgroundVariant, Controls, type Node, type Edge } from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useOrg } from '../App'
import { fetchDecisions, type DecisionSummary } from '../api/decisions'
import { TableNavIcon } from '../components/Sidebar/SidebarIcons'

function DrdGraph({ decisions, groupId }: { decisions: DecisionSummary[]; groupId?: string }) {
  const navigate = useNavigate()
  const editBase = groupId ? `/process-groups/${groupId}/decisions` : '/decisions'
  const nodes: Node[] = useMemo(
    () =>
      decisions.map((d, i) => ({
        id: d.decision_key,
        position: { x: (i % 4) * 180, y: Math.floor(i / 4) * 100 },
        data: { label: d.name ?? d.decision_key },
        style: {
          background: 'var(--color-bg-secondary, #1e1e2e)',
          color: 'var(--color-text, #cdd6f4)',
          border: '1px solid var(--color-border, #45475a)',
          borderRadius: 6,
          fontSize: 12,
          padding: '6px 12px',
          cursor: 'pointer',
        },
      })),
    [decisions],
  )

  const edges: Edge[] = []

  if (decisions.length === 0) {
    return null
  }

  return (
    <div style={{ height: 280, border: '1px solid var(--color-border, #45475a)', borderRadius: 8, overflow: 'hidden', marginBottom: 24 }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodeClick={(_, node) => navigate(`${editBase}/${node.id}/edit`)}
        fitView
        proOptions={{ hideAttribution: true }}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
      >
        <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  )
}

export default function Decisions() {
  const { org } = useOrg()
  const { groupId } = useParams<{ groupId?: string }>()

  const { data: decisions = [], isLoading } = useQuery({
    queryKey: ['decisions', org?.id, groupId],
    queryFn: () => fetchDecisions(org!.id, groupId),
    enabled: !!org,
  })

  const newHref = groupId ? `/process-groups/${groupId}/decisions/new` : '/decisions/new'
  const editBase = groupId ? `/process-groups/${groupId}/decisions` : '/decisions'

  if (!org) {
    return (
      <div className="empty-state">
        <p>Select an organisation to manage decision tables.</p>
      </div>
    )
  }

  if (isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div style={{ padding: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
        <div>
          <h1 style={{ fontSize: 18, fontWeight: 600, margin: 0, display: 'flex', alignItems: 'center', gap: 8 }}>
            <TableNavIcon size={16} />
            Decision Tables
          </h1>
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: '4px 0 0' }}>
            {groupId ? 'Scoped to this process group.' : 'DMN 1.3 decision tables evaluated by business rule tasks.'}
          </p>
        </div>
        <Link to={newHref}>
          <button className="btn-primary">+ New decision</button>
        </Link>
      </div>

      {decisions.length > 1 && <DrdGraph decisions={decisions} groupId={groupId} />}

      {decisions.length === 0 ? (
        <div className="empty-state">
          <p>No decision tables yet. Click <strong>+ New decision</strong> to create one.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Key</th>
              <th>Version</th>
              <th>Scope</th>
              <th>Deployed</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {decisions.map(d => (
              <tr key={d.id}>
                <td style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <TableNavIcon size={13} />
                  {d.name ?? d.decision_key}
                </td>
                <td>
                  <code style={{ fontSize: 11, background: 'var(--color-bg-secondary, #1e1e2e)', padding: '2px 6px', borderRadius: 4 }}>
                    {d.decision_key}
                  </code>
                </td>
                <td>v{d.version}</td>
                <td>
                  <span style={{
                    fontSize: 10,
                    padding: '2px 6px',
                    borderRadius: 4,
                    background: d.process_group_id ? 'rgba(245,158,11,0.12)' : 'rgba(100,116,139,0.15)',
                    color: d.process_group_id ? '#f59e0b' : 'var(--text-tertiary)',
                    fontWeight: 500,
                  }}>
                    {d.process_group_id ? 'Group' : 'Org-wide'}
                  </span>
                </td>
                <td style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
                  {new Date(d.deployed_at).toLocaleDateString()}
                </td>
                <td>
                  <Link to={`${editBase}/${d.decision_key}/edit`}>
                    <button style={{ fontSize: 11, padding: '3px 10px' }}>Edit</button>
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
