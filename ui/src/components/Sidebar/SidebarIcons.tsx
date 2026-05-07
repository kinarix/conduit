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
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <rect x="3" y="9" width="18" height="13" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
      <path d="M2 10L12 3l10 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="9" y="15" width="6" height="7" rx="1" stroke="currentColor" strokeWidth="1.3" />
      <rect x="4.5" y="12" width="4" height="3" rx="0.5" stroke="currentColor" strokeWidth="1.2" />
      <rect x="15.5" y="12" width="4" height="3" rx="0.5" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  )
}

export function GroupIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <rect x="6" y="16.5" width="15" height="5" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
      <rect x="3.5" y="11.5" width="15" height="5" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
      <rect x="1" y="6.5" width="15" height="5" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  )
}

export function ProcessIcon({ size = 12, color = '#16a34a' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <circle cx="12" cy="12" r="9.5" stroke={color} strokeWidth="1.5" />
      <path d="M9.5 8.5l7 3.5-7 3.5V8.5z" stroke={color} strokeWidth="1.4" strokeLinejoin="round" strokeLinecap="round" />
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

export function DownloadIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M6 8V2M3 5l3-3 3 3M2 10h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

export function UploadIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M6 2v6M3 5l3 3 3-3M2 10h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

export function TableNavIcon({ size = 13, color = '#f59e0b' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" style={{ flexShrink: 0 }}>
      <path d="M12 2.5l9.5 9.5-9.5 9.5L2.5 12z" stroke={color} strokeWidth="1.5" strokeLinejoin="round" />
      <line x1="12" y1="2.5" x2="12" y2="21.5" stroke={color} strokeWidth="1" opacity="0.4" />
      <line x1="2.5" y1="12" x2="21.5" y2="12" stroke={color} strokeWidth="1" opacity="0.4" />
    </svg>
  )
}
