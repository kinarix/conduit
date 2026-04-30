import { useNavigate, useLocation } from 'react-router-dom'
import { type LogicalProcess } from '../../api/deployments'
import { ProcessIcon, TrashIcon } from './SidebarIcons'
import styles from './Sidebar.module.css'

interface Props {
  proc: LogicalProcess
  onConfirmDelete: (proc: LogicalProcess) => void
}

const DRAG_MIME = 'application/x-conduit-process-def-id'

export default function ProcessRow({ proc, onConfirmDelete }: Props) {
  const navigate = useNavigate()
  const location = useLocation()

  const editingThis = proc.versions.some(
    v =>
      location.pathname === `/definitions/${v.id}/edit` ||
      location.pathname === `/definitions/${v.id}`,
  )
  const dashboardPath = `/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`
  const onDashboard = location.pathname === dashboardPath
  const active = editingThis || onDashboard

  const handleClick = () => {
    navigate(`/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`)
  }

  return (
    <div
      className={`${styles.row} ${styles.indent2} ${active ? styles.selected : ''}`}
      draggable
      onDragStart={e => {
        e.dataTransfer.effectAllowed = 'move'
        e.dataTransfer.setData(DRAG_MIME, proc.latest.id)
        // Also set text/plain as a fallback for browsers that ignore custom
        // mime types (older Safari).
        e.dataTransfer.setData('text/plain', proc.latest.id)
      }}
      onClick={handleClick}
      title={proc.displayName}
    >
      <span className={styles.toggle} />
      <span className={styles.icon}>
        <ProcessIcon size={11} />
      </span>
      <span className={styles.label}>{proc.displayName}</span>
      {proc.hasDraft && <span className={styles.draftDot} title="Has draft" />}
      {proc.versions.length > 1 && (
        <span className={styles.versionBadge} title={`${proc.versions.length} versions`}>
          ×{proc.versions.length}
        </span>
      )}
      <span className={styles.actions}>
        <button
          type="button"
          className={`${styles.actionBtn} ${styles.delete}`}
          title="Delete process"
          onClick={e => {
            e.stopPropagation()
            onConfirmDelete(proc)
          }}
        >
          <TrashIcon size={11} />
        </button>
      </span>
    </div>
  )
}

export const PROCESS_DRAG_MIME = DRAG_MIME
