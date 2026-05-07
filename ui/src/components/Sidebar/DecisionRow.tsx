import { useNavigate, useLocation } from 'react-router-dom'
import { type DecisionSummary } from '../../api/decisions'
import { TableNavIcon, TrashIcon } from './SidebarIcons'
import styles from './Sidebar.module.css'

interface Props {
  decision: DecisionSummary
  editBase: string
  indentClass?: string
  onSelect?: () => void
  onConfirmDelete: () => void
}

export default function DecisionRow({ decision, editBase, indentClass = styles.indent2, onSelect, onConfirmDelete }: Props) {
  const navigate = useNavigate()
  const location = useLocation()

  const editPath = `${editBase}/${decision.decision_key}/edit`
  const active = location.pathname === editPath

  return (
    <div
      className={`${styles.row} ${indentClass} ${active ? styles.selected : ''}`}
      onClick={() => { onSelect?.(); navigate(editPath) }}
      title={decision.name ?? decision.decision_key}
    >
      <span className={styles.toggle} />
      <span className={styles.icon}>
        <TableNavIcon size={11} />
      </span>
      <span className={styles.label}>{decision.name ?? decision.decision_key}</span>
      <span className={styles.actions}>
        <button
          type="button"
          className={`${styles.actionBtn} ${styles.delete}`}
          title="Delete decision"
          onClick={e => { e.stopPropagation(); onConfirmDelete() }}
        >
          <TrashIcon size={11} />
        </button>
      </span>
    </div>
  )
}
