import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchOrgs, deleteOrg } from '../../api/orgs'
import { deleteProcessGroup, type ProcessGroup } from '../../api/processGroups'
import { deleteDeployment, type LogicalProcess } from '../../api/deployments'
import { type Org } from '../../App'
import WorkspaceHeader from './WorkspaceHeader'
import OrgRow from './OrgRow'
import FooterNav from './FooterNav'
import { useExpansion } from './useExpansion'
import styles from './Sidebar.module.css'

export default function Sidebar() {
  const qc = useQueryClient()
  const orgsExp = useExpansion('sidebar.orgs')

  const { data: orgs = [] } = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  const [confirmOrg, setConfirmOrg] = useState<Org | null>(null)
  const [confirmGroup, setConfirmGroup] = useState<ProcessGroup | null>(null)
  const [confirmProcess, setConfirmProcess] = useState<LogicalProcess | null>(null)

  const deleteOrgMut = useMutation({
    mutationFn: () => deleteOrg(confirmOrg!.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['orgs'] })
      setConfirmOrg(null)
    },
  })

  const deleteGroupMut = useMutation({
    mutationFn: () => deleteProcessGroup(confirmGroup!.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['process-groups'] })
      qc.invalidateQueries({ queryKey: ['deployments'] })
      setConfirmGroup(null)
    },
  })

  const deleteProcessMut = useMutation({
    // Deleting a logical process deletes every version. Easier to call once per
    // version than to invent a new endpoint.
    mutationFn: async () => {
      if (!confirmProcess) return
      for (const v of confirmProcess.versions) await deleteDeployment(v.id)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      qc.invalidateQueries({ queryKey: ['instances'] })
      setConfirmProcess(null)
    },
  })

  return (
    <aside className={styles.sidebar}>
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
