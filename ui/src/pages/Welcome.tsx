import { ReactNode } from 'react'

interface ConceptCard {
  title: string
  blurb: string
  illustration: ReactNode
}

const STROKE = 'var(--color-text-muted)'
const ACCENT = 'var(--color-primary)'

const ORG_SVG = (
  <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
    <rect x="46" y="6" width="28" height="18" rx="3" stroke={ACCENT} strokeWidth="1.5" />
    <rect x="14" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
    <rect x="46" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
    <rect x="78" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
    <path d="M60 24 L60 38 M28 38 L92 38 M28 38 L28 50 M60 38 L60 50 M92 38 L92 50" stroke={STROKE} strokeWidth="1.2" />
  </svg>
)

const FOLDER_SVG = (
  <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
    <path d="M14 22 L14 64 Q14 68 18 68 L102 68 Q106 68 106 64 L106 30 Q106 26 102 26 L56 26 L48 18 L18 18 Q14 18 14 22 Z"
      stroke={ACCENT} strokeWidth="1.5" />
    <line x1="28" y1="42" x2="92" y2="42" stroke={STROKE} strokeWidth="1" opacity="0.5" />
    <line x1="28" y1="50" x2="80" y2="50" stroke={STROKE} strokeWidth="1" opacity="0.5" />
    <line x1="28" y1="58" x2="70" y2="58" stroke={STROKE} strokeWidth="1" opacity="0.5" />
  </svg>
)

const DEFINITION_SVG = (
  <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
    <circle cx="16" cy="40" r="6" stroke={ACCENT} strokeWidth="1.5" />
    <rect x="34" y="32" width="22" height="16" rx="2" stroke={STROKE} strokeWidth="1.5" />
    <path d="M68 32 L80 40 L68 48 L56 40 Z" stroke={STROKE} strokeWidth="1.5" />
    <rect x="86" y="32" width="22" height="16" rx="2" stroke={STROKE} strokeWidth="1.5" />
    <line x1="22" y1="40" x2="34" y2="40" stroke={STROKE} strokeWidth="1.2" />
    <line x1="56" y1="40" x2="56" y2="40" stroke={STROKE} strokeWidth="1.2" />
    <line x1="80" y1="40" x2="86" y2="40" stroke={STROKE} strokeWidth="1.2" />
  </svg>
)

const INSTANCE_SVG = (
  <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
    <rect x="14" y="14" width="92" height="20" rx="3" stroke={STROKE} strokeWidth="1.2" opacity="0.45" />
    <rect x="14" y="38" width="92" height="20" rx="3" stroke={ACCENT} strokeWidth="1.5" />
    <circle cx="26" cy="48" r="3" fill={ACCENT} />
    <line x1="34" y1="48" x2="94" y2="48" stroke={ACCENT} strokeWidth="1.2" strokeDasharray="2 3" />
    <rect x="14" y="62" width="92" height="10" rx="3" stroke={STROKE} strokeWidth="1.2" opacity="0.3" />
  </svg>
)

const TASK_SVG = (
  <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
    <rect x="22" y="16" width="76" height="48" rx="4" stroke={ACCENT} strokeWidth="1.5" />
    <line x1="32" y1="30" x2="56" y2="30" stroke={STROKE} strokeWidth="1.2" />
    <line x1="32" y1="40" x2="78" y2="40" stroke={STROKE} strokeWidth="1.2" opacity="0.6" />
    <line x1="32" y1="50" x2="68" y2="50" stroke={STROKE} strokeWidth="1.2" opacity="0.6" />
    <path d="M76 28 L82 34 L92 22" stroke={ACCENT} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
)

const CONCEPTS: ConceptCard[] = [
  {
    title: 'Organization',
    blurb: 'A workspace that owns its process groups, processes, and people. Everything is scoped under an org.',
    illustration: ORG_SVG,
  },
  {
    title: 'Process Group',
    blurb: 'Org units inside an organization. Group related processes by team, domain, or business unit.',
    illustration: FOLDER_SVG,
  },
  {
    title: 'Process Definition',
    blurb: 'A BPMN blueprint — the steps, gateways, and events that describe how work flows.',
    illustration: DEFINITION_SVG,
  },
  {
    title: 'Process Instance',
    blurb: 'A live execution of a definition. Each instance carries its own variables and history.',
    illustration: INSTANCE_SVG,
  },
  {
    title: 'Task',
    blurb: 'A unit of work waiting on a person or external worker. Completing it advances the instance.',
    illustration: TASK_SVG,
  },
]

export default function Welcome() {
  return (
    <div style={{ maxWidth: 960, margin: '0 auto', padding: '24px 8px' }}>
      <div style={{ marginBottom: 32 }}>
        <h1 style={{ fontSize: 22, fontWeight: 600, marginBottom: 8 }}>Welcome to Conduit</h1>
        <p style={{ fontSize: 14, color: 'var(--color-text-muted)', maxWidth: 640, lineHeight: 1.5 }}>
          A lightweight process orchestration engine. Pick an org, process group, process, instance, or task
          from the tree on the left to get started, or read up on the core concepts below.
        </p>
      </div>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
          gap: 16,
        }}
      >
        {CONCEPTS.map(c => (
          <div
            key={c.title}
            style={{
              border: '1px solid var(--color-border)',
              borderRadius: 6,
              padding: 16,
              background: 'var(--color-surface)',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: 'var(--color-surface-2)',
                borderRadius: 4,
                marginBottom: 12,
                padding: 8,
                minHeight: 96,
              }}
            >
              {c.illustration}
            </div>
            <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>{c.title}</div>
            <div style={{ fontSize: 12, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
              {c.blurb}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
