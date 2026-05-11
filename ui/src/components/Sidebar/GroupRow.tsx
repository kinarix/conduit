import { useRef, useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useMutation, useQueryClient, useQuery } from '@tanstack/react-query'
import { renameProcessGroup, assignProcessGroup, type ProcessGroup } from '../../api/processGroups'
import { groupByProcessKey, createDraft, type ProcessDefinition, type LogicalProcess } from '../../api/deployments'
import { fetchDecisions, deployDecision, makeStubDmn, nextDecisionName, type DecisionSummary } from '../../api/decisions'
import { fromXml } from '../bpmn/bpmnXml'
import { useOrg, type Org } from '../../App'
import { ChevronIcon, GroupIcon, PencilIcon, PlusIcon, TrashIcon, UploadIcon, TableNavIcon } from './SidebarIcons'
import ProcessRow, { PROCESS_DRAG_MIME } from './ProcessRow'
import DecisionRow from './DecisionRow'
import InlineNameInput from './InlineNameInput'
import styles from './Sidebar.module.css'

interface Props {
  group: ProcessGroup
  org: Org
  defs: ProcessDefinition[]
  expanded: boolean
  onToggle: () => void
  onConfirmDeleteGroup: (group: ProcessGroup) => void
  onConfirmDeleteProcess: (proc: LogicalProcess) => void
  onConfirmDeleteDecision: (orgId: string, decision: DecisionSummary) => void
  onExpand?: () => void
  autoEdit?: boolean
  onEditDone?: () => void
}

export default function GroupRow({
  group,
  org,
  defs,
  expanded,
  onToggle,
  onConfirmDeleteGroup,
  onConfirmDeleteProcess,
  onConfirmDeleteDecision,
  onExpand,
  autoEdit = false,
  onEditDone,
}: Props) {
  const navigate = useNavigate()
  const location = useLocation()
  const qc = useQueryClient()
  const { setOrg } = useOrg()
  const [editing, setEditing] = useState(autoEdit)
  const [dropping, setDropping] = useState(false)
  const importInputRef = useRef<HTMLInputElement>(null)
  const orgId = org.id

  const renameMut = useMutation({
    mutationFn: (name: string) => renameProcessGroup(orgId, group.id, name),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['process-groups', orgId] })
      setEditing(false)
      onEditDone?.()
    },
  })

  const assignMut = useMutation({
    mutationFn: (defId: string) => assignProcessGroup(orgId, defId, group.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['deployments', orgId] }),
  })

  const importMut = useMutation({
    mutationFn: ({ key, name, bpmn_xml }: { key: string; name: string; bpmn_xml: string }) =>
      createDraft(orgId, { process_group_id: group.id, key, name, bpmn_xml }),
    onSuccess: def => {
      qc.invalidateQueries({ queryKey: ['deployments', orgId] })
      navigate(`/definitions/${def.id}/edit`)
    },
  })

  const createDecisionMut = useMutation({
    mutationFn: async () => {
      const cached = qc.getQueryData<DecisionSummary[]>(['decisions', orgId]) ?? []
      const name = nextDecisionName(cached)
      const key = `decision_${Date.now()}`
      await deployDecision(orgId, makeStubDmn(key, name), group.id)
      return key
    },
    onSuccess: key => {
      qc.invalidateQueries({ queryKey: ['decisions', orgId] })
      onExpand?.()
      navigate(`/process-groups/${group.id}/decisions/${key}/edit`)
    },
  })

  const handleImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    e.target.value = ''
    const reader = new FileReader()
    reader.onload = ev => {
      const xml = ev.target?.result as string
      try {
        const parsed = fromXml(xml)
        importMut.mutate({ key: parsed.processId, name: parsed.processName, bpmn_xml: xml })
      } catch {
        // invalid BPMN — silently ignore (user sees nothing happens)
      }
    }
    reader.readAsText(file)
  }

  const groupActive = location.pathname.startsWith(`/groups/${group.id}`) ||
                      location.pathname.startsWith(`/process-groups/${group.id}`)

  const { data: decisions = [] } = useQuery({
    queryKey: ['decisions', orgId, group.id],
    queryFn: () => fetchDecisions(orgId, group.id),
    enabled: expanded,
  })

  const processes = groupByProcessKey(defs)

  return (
    <>
      <div
        className={`${styles.row} ${styles.indent1} ${groupActive ? styles.selected : ''} ${dropping ? styles.dropTarget : ''}`}
        onClick={() => {
          if (!editing) {
            setOrg(org)
            onToggle()
            navigate(`/process-groups/${group.id}`)
          }
        }}
        onDragOver={e => {
          if (e.dataTransfer.types.includes(PROCESS_DRAG_MIME)) {
            e.preventDefault()
            e.dataTransfer.dropEffect = 'move'
            setDropping(true)
          }
        }}
        onDragLeave={() => setDropping(false)}
        onDrop={e => {
          e.preventDefault()
          setDropping(false)
          const id =
            e.dataTransfer.getData(PROCESS_DRAG_MIME) ||
            e.dataTransfer.getData('text/plain')
          if (id) assignMut.mutate(id)
        }}
        title={group.name}
      >
        <span className={styles.toggle} onClick={e => { e.stopPropagation(); onToggle() }}>
          <ChevronIcon size={10} expanded={expanded} />
        </span>
        <span className={styles.icon}>
          <GroupIcon size={13} />
        </span>
        {editing ? (
          <InlineNameInput
            initial={group.name}
            onSubmit={name => renameMut.mutate(name)}
            onCancel={() => { setEditing(false); onEditDone?.() }}
          />
        ) : (
          <span className={styles.label}>{group.name}</span>
        )}
        <span className={styles.actions}>
          <input
            ref={importInputRef}
            type="file"
            accept=".bpmn,.xml"
            style={{ display: 'none' }}
            onChange={handleImport}
          />
          <button
            type="button"
            className={styles.actionBtn}
            title="New decision table"
            onClick={e => { e.stopPropagation(); setOrg(org); createDecisionMut.mutate() }}
            disabled={createDecisionMut.isPending}
          >
            <TableNavIcon size={13} />
          </button>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.add}`}
            title="New process"
            onClick={e => {
              e.stopPropagation()
              navigate(`/process-groups/${group.id}/definitions/new`)
            }}
          >
            <PlusIcon size={13} />
          </button>
          <button
            type="button"
            className={styles.actionBtn}
            title="Import BPMN"
            onClick={e => { e.stopPropagation(); importInputRef.current?.click() }}
          >
            <UploadIcon size={13} />
          </button>
          <button
            type="button"
            className={styles.actionBtn}
            title="Rename"
            onClick={e => { e.stopPropagation(); setEditing(true) }}
          >
            <PencilIcon size={13} />
          </button>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.delete}`}
            title="Delete process group"
            onClick={e => { e.stopPropagation(); onConfirmDeleteGroup(group) }}
          >
            <TrashIcon size={13} />
          </button>
        </span>
      </div>

      {expanded && (
        <>
          {processes.length === 0 && decisions.length === 0 ? (
            <div className={styles.empty} style={{ paddingLeft: 'calc(var(--space-3) + 32px)' }}>
              No processes yet
            </div>
          ) : (
            <>
              {processes.map(proc => (
                <ProcessRow
                  key={`${proc.groupId}::${proc.key}`}
                  proc={proc}
                  org={org}
                  onConfirmDelete={onConfirmDeleteProcess}
                />
              ))}
              {decisions.map(dec => (
                <DecisionRow
                  key={dec.id}
                  decision={dec}
                  orgId={orgId}
                  editBase={`/process-groups/${group.id}/decisions`}
                  onSelect={() => setOrg(org)}
                  onConfirmDelete={() => onConfirmDeleteDecision(orgId, dec)}
                />
              ))}
            </>
          )}
        </>
      )}
    </>
  )
}
