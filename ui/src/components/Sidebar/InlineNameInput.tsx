import { useState, useEffect, useRef } from 'react'
import styles from './Sidebar.module.css'

interface Props {
  initial: string
  onSubmit: (next: string) => void
  onCancel: () => void
  placeholder?: string
}

/**
 * Inline rename input. Submits on Enter / blur, cancels on Escape.
 * Re-uses the sidebar's --accent border for focus.
 */
export default function InlineNameInput({ initial, onSubmit, onCancel, placeholder }: Props) {
  const [value, setValue] = useState(initial)
  const ref = useRef<HTMLInputElement>(null)

  useEffect(() => {
    ref.current?.focus()
    ref.current?.select()
  }, [])

  const commit = () => {
    const trimmed = value.trim()
    if (!trimmed || trimmed === initial) onCancel()
    else onSubmit(trimmed)
  }

  return (
    <input
      ref={ref}
      className={styles.inlineInput}
      value={value}
      placeholder={placeholder}
      onChange={e => setValue(e.target.value)}
      onKeyDown={e => {
        if (e.key === 'Enter') commit()
        else if (e.key === 'Escape') onCancel()
      }}
      onBlur={commit}
      onClick={e => e.stopPropagation()}
    />
  )
}
