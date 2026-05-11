import { useState, useEffect, useRef } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchOrgs, deleteOrg } from '../../api/orgs'
import { deleteProcessGroup, type ProcessGroup } from '../../api/processGroups'
import { deleteDeployment, type LogicalProcess } from '../../api/deployments'
import { deleteDecision, type DecisionSummary } from '../../api/decisions'
import { type Org } from '../../App'
import WorkspaceHeader from './WorkspaceHeader'
import OrgRow from './OrgRow'
import FooterNav from './FooterNav'
import { useExpansion } from './useExpansion'
import styles from './Sidebar.module.css'

export default function Sidebar({ width }: { width?: number }) {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const location = useLocation()
  const orgsExp = useExpansion('sidebar.orgs')

  const { data: orgs = [] } = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  // Auto-expand the first org on initial load so the tree is immediately visible.
  const didAutoExpand = useRef(false)
  useEffect(() => {
    if (didAutoExpand.current || orgs.length === 0) return
    if (orgsExp.expanded.size === 0) {
      didAutoExpand.current = true
      orgsExp.expand(orgs[0].id)
    } else {
      didAutoExpand.current = true
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [orgs.length])

  const [confirmOrg, setConfirmOrg] = useState<Org | null>(null)
  const [confirmGroup, setConfirmGroup] = useState<ProcessGroup | null>(null)
  const [confirmProcess, setConfirmProcess] = useState<LogicalProcess | null>(null)
  const [confirmDecision, setConfirmDecision] = useState<{ orgId: string; decision: DecisionSummary } | null>(null)

  const deleteOrgMut = useMutation({
    mutationFn: () => deleteOrg(confirmOrg!.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['orgs'] })
      setConfirmOrg(null)
    },
  })

  const deleteGroupMut = useMutation({
    mutationFn: () => deleteProcessGroup(confirmGroup!.org_id, confirmGroup!.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['process-groups'] })
      qc.invalidateQueries({ queryKey: ['deployments'] })
      setConfirmGroup(null)
    },
  })

  const deleteProcessMut = useMutation({
    mutationFn: async () => {
      if (!confirmProcess) return
      for (const v of confirmProcess.versions) await deleteDeployment(v.org_id, v.id)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      qc.invalidateQueries({ queryKey: ['instances'] })
      setConfirmProcess(null)
    },
  })

  const deleteDecisionMut = useMutation({
    mutationFn: () => deleteDecision(confirmDecision!.orgId, confirmDecision!.decision.decision_key),
    onSuccess: () => {
      const { orgId, decision } = confirmDecision!
      qc.invalidateQueries({ queryKey: ['decisions', orgId] })
      const editPaths = [
        `/decisions/${decision.decision_key}/edit`,
        `/process-groups/${decision.process_group_id}/decisions/${decision.decision_key}/edit`,
      ]
      if (editPaths.some(p => location.pathname === p)) navigate('/decisions')
      setConfirmDecision(null)
    },
  })

  return (
    <aside className={styles.sidebar} style={width !== undefined ? { width } : undefined}>
      <WorkspaceHeader />

      <div className={styles.tree}>
        {orgs.map(org => (
          <OrgRow
            key={org.id}
            org={org}
            expanded={orgsExp.expanded.has(org.id)}
            onToggle={() => orgsExp.toggle(org.id)}
            onConfirmDeleteOrg={setConfirmOrg}
            onConfirmDeleteGroup={setConfirmGroup}
            onConfirmDeleteProcess={setConfirmProcess}
            onConfirmDeleteDecision={(orgId, decision) => setConfirmDecision({ orgId, decision })}
          />
        ))}
      </div>

      <FooterNav />

      {confirmOrg && (
        <ConfirmModal
          title="Delete organisation"
          body={
            <>
              Delete <strong>"{confirmOrg.name}"</strong>? This removes the org and all its
              groups permanently.
            </>
          }
          pending={deleteOrgMut.isPending}
          error={deleteOrgMut.error}
          onCancel={() => { deleteOrgMut.reset(); setConfirmOrg(null) }}
          onConfirm={() => deleteOrgMut.mutate()}
        />
      )}
      {confirmGroup && (
        <ConfirmModal
          title="Delete process group"
          body={
            <>
              Delete <strong>"{confirmGroup.name}"</strong>? The group must be empty —
              move any processes inside to another group first.
            </>
          }
          pending={deleteGroupMut.isPending}
          error={deleteGroupMut.error}
          onCancel={() => { deleteGroupMut.reset(); setConfirmGroup(null) }}
          onConfirm={() => deleteGroupMut.mutate()}
        />
      )}
      {confirmProcess && (
        <ConfirmModal
          title="Delete process"
          body={
            <>
              Delete <strong>"{confirmProcess.displayName}"</strong> ({confirmProcess.versions.length}{' '}
              {confirmProcess.versions.length === 1 ? 'version' : 'versions'})? This cannot be
              undone. All versions must have no instances.
            </>
          }
          pending={deleteProcessMut.isPending}
          error={deleteProcessMut.error}
          onCancel={() => { deleteProcessMut.reset(); setConfirmProcess(null) }}
          onConfirm={() => deleteProcessMut.mutate()}
        />
      )}
      {confirmDecision && (
        <ConfirmModal
          title="Delete decision table"
          body={
            <>
              Delete <strong>"{confirmDecision.decision.name ?? confirmDecision.decision.decision_key}"</strong>?
              This cannot be undone. The decision must not be referenced by any process or other decision table.
            </>
          }
          pending={deleteDecisionMut.isPending}
          error={deleteDecisionMut.error}
          onCancel={() => { deleteDecisionMut.reset(); setConfirmDecision(null) }}
          onConfirm={() => deleteDecisionMut.mutate()}
        />
      )}
    </aside>
  )
}

function ConfirmModal({
  title,
  body,
  pending,
  error,
  onCancel,
  onConfirm,
}: {
  title: string
  body: React.ReactNode
  pending: boolean
  error: unknown
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <h3>{title}</h3>
        <p style={{ fontSize: 13, color: 'var(--text-secondary)', margin: '8px 0 16px' }}>{body}</p>
        {error ? <div className="error-banner">{String((error as Error).message ?? error)}</div> : null}
        <div className="modal-actions">
          <button className="btn-ghost" disabled={pending} onClick={onCancel}>Cancel</button>
          <button className="btn-danger" disabled={pending} onClick={onConfirm}>
            {pending ? 'Deleting…' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  )
}
