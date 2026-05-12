/**
 * Square 28×28 icon-only button. Native `title` provides the hover
 * tooltip — no JS, no extra DOM, matches browser-native a11y.
 *
 * Global rule: row-level destructive / actionable controls in admin
 * tables use this component so a row of actions reads as a tidy strip
 * of equal-sized chips rather than a ragged mix of wrapped-text
 * buttons. Tooltip text comes from `title` and is the human-readable
 * name of the action.
 *
 * Three tones map to the existing colour tokens:
 *  - `primary`  — filled brand colour (default for non-destructive)
 *  - `danger`   — filled error red (delete/remove)
 *  - `neutral`  — surface-2 with text colour (read-only, "open detail")
 */
import type { ReactNode, MouseEvent } from 'react'

export type IconButtonTone = 'primary' | 'danger' | 'neutral'

interface Props {
  title: string
  tone?: IconButtonTone
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void
  disabled?: boolean
  children: ReactNode
  /** Override the default 28×28 size if the surrounding row is denser. */
  size?: number
}

export default function IconButton({
  title, tone = 'primary', onClick, disabled, children, size = 28,
}: Props) {
  const palette =
    tone === 'primary' ? {
      bg: 'var(--color-primary)',
      hover: 'var(--color-primary-hover)',
      color: '#fff',
    } : tone === 'danger' ? {
      bg: 'var(--color-error)',
      hover: 'var(--color-error-hover, color-mix(in srgb, var(--color-error) 85%, black))',
      color: '#fff',
    } : {
      bg: 'var(--color-surface-2)',
      hover: 'var(--color-border)',
      color: 'var(--color-text)',
    }

  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      disabled={disabled}
      style={{
        width: size,
        height: size,
        padding: 0,
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: palette.bg,
        color: palette.color,
        border: 'none',
        borderRadius: 6,
        opacity: disabled ? 0.45 : 1,
        cursor: disabled ? 'not-allowed' : 'pointer',
        transition: 'background 0.12s, transform 0.05s',
      }}
      onMouseEnter={e => { if (!disabled) (e.currentTarget as HTMLButtonElement).style.background = palette.hover }}
      onMouseLeave={e => { if (!disabled) (e.currentTarget as HTMLButtonElement).style.background = palette.bg }}
      onMouseDown={e => { if (!disabled) (e.currentTarget as HTMLButtonElement).style.transform = 'scale(0.94)' }}
      onMouseUp={e => { (e.currentTarget as HTMLButtonElement).style.transform = 'scale(1)' }}
    >
      {children}
    </button>
  )
}
