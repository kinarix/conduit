import { useState, useEffect, useRef } from 'react'
import styles from './Sidebar.module.css'

interface Props {
  initial: string
  onSubmit: (next: string) => void
  onCancel: () => void
  onValueChange?: (value: string) => void
  placeholder?: string
  isInvalid?: boolean
}

export default function InlineNameInput({ initial, onSubmit, onCancel, onValueChange, placeholder, isInvalid }: Props) {
  const [value, setValue] = useState(initial)
  const ref = useRef<HTMLInputElement>(null)

  useEffect(() => {
    ref.current?.focus()
    ref.current?.select()
  }, [])

  const commit = () => {
    if (isInvalid) { onCancel(); return }
    const trimmed = value.trim()
    if (!trimmed || trimmed === initial) onCancel()
    else onSubmit(trimmed)
  }

  return (
    <input
      ref={ref}
      className={`${styles.inlineInput}${isInvalid ? ` ${styles.inlineInputError}` : ''}`}
      value={value}
      placeholder={placeholder}
      onChange={e => {
        setValue(e.target.value)
        onValueChange?.(e.target.value)
      }}
      onKeyDown={e => {
        if (e.key === 'Enter') { if (!isInvalid) commit() }
        else if (e.key === 'Escape') onCancel()
      }}
      onBlur={commit}
      onClick={e => e.stopPropagation()}
    />
  )
}
