interface IconProps {
  size?: number
  color?: string
}

export function ChevronIcon({ size = 12, expanded = false }: IconProps & { expanded?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 12 12"
      style={{
        flexShrink: 0,
        transform: expanded ? 'rotate(90deg)' : 'none',
        transition: 'transform 150ms ease',
      }}
      aria-hidden="true"
    >
      <path
        d="M4.5 3 L7.5 6 L4.5 9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export function OrgIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <rect x="2" y="3" width="5" height="11" rx="0.5" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.15" />
      <rect x="9" y="6" width="5" height="8" rx="0.5" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.15" />
      <rect x="3.5" y="5" width="1" height="1" fill="currentColor" />
      <rect x="3.5" y="7.5" width="1" height="1" fill="currentColor" />
      <rect x="3.5" y="10" width="1" height="1" fill="currentColor" />
      <rect x="5" y="5" width="1" height="1" fill="currentColor" />
      <rect x="5" y="7.5" width="1" height="1" fill="currentColor" />
      <rect x="5" y="10" width="1" height="1" fill="currentColor" />
      <rect x="10.5" y="8" width="1" height="1" fill="currentColor" />
      <rect x="10.5" y="10.5" width="1" height="1" fill="currentColor" />
      <rect x="12" y="8" width="1" height="1" fill="currentColor" />
      <rect x="12" y="10.5" width="1" height="1" fill="currentColor" />
    </svg>
  )
}

export function GroupIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <rect x="3.5" y="2.5" width="9" height="2.5" rx="0.6" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.12" />
      <rect x="2" y="5" width="12" height="2.5" rx="0.6" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.12" />
      <rect x="1" y="7.5" width="14" height="6" rx="0.7" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.18" />
    </svg>
  )
}

export function ProcessIcon({ size = 12 }: IconProps) {
  const w = Math.round(size * 1.7)
  return (
    <svg width={w} height={size} viewBox="0 0 22 10" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <circle cx="2.5" cy="5" r="1.8" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.15" />
      <line x1="4.5" y1="5" x2="6.8" y2="5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
      <rect x="7" y="2.2" width="8" height="5.6" rx="1" stroke="currentColor" strokeWidth="1.2" fill="currentColor" fillOpacity="0.15" />
      <line x1="15.2" y1="5" x2="17.5" y2="5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
      <circle cx="19.5" cy="5" r="1.8" stroke="currentColor" strokeWidth="1.4" fill="none" />
    </svg>
  )
}

export function PlusIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M6 2v8M2 6h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  )
}

export function TrashIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path
        d="M2.5 3.5h7M5 5.5v3M7 5.5v3M3.5 3.5l.5 6.5a.5.5 0 0 0 .5.5h3a.5.5 0 0 0 .5-.5l.5-6.5M4.5 3.5V2a.5.5 0 0 1 .5-.5h2a.5.5 0 0 1 .5.5v1.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export function PencilIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M2 10h2l6-6-2-2-6 6v2z" fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
    </svg>
  )
}

export function InboxIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path
        d="M2 9l1.5-5h9L14 9v4H2V9zM2 9h3.5l1 1.5h3l1-1.5H14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export function ListIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M3 4h10M3 8h10M3 12h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  )
}
