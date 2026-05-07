import { useState, useEffect, useCallback, useRef, KeyboardEvent } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useOrg } from '../App'
import {
  fetchDecision,
  fetchDecisions,
  deployDecision,
  testDecision,
  type HitPolicy,
  type CollectAggregator,
  type TestResult,
} from '../api/decisions'

// ─── Types ────────────────────────────────────────────────────────────────────

interface InputCol {
  id: string
  expression: string
}

interface OutputCol {
  id: string
  name: string
  outputValues: string  // comma-separated priority list for PRIORITY / OUTPUT ORDER
}

interface TableRule {
  id: string
  inputEntries: string[]
  outputEntries: string[]
}

interface EditorState {
  decisionKey: string
  name: string
  hitPolicy: HitPolicy
  collectAggregator: CollectAggregator | ''
  inputs: InputCol[]
  outputs: OutputCol[]
  rules: TableRule[]
  requiredDecisions: string[]
}

const HIT_POLICIES: HitPolicy[] = ['UNIQUE', 'FIRST', 'ANY', 'COLLECT', 'RULE_ORDER', 'PRIORITY', 'OUTPUT_ORDER']
const COLLECT_AGGREGATORS: CollectAggregator[] = ['SUM', 'MIN', 'MAX', 'COUNT']

let _id = 0
const uid = () => `_${++_id}`

function emptyState(): EditorState {
  const i1 = uid()
  const o1 = uid()
  const r1 = uid()
  return {
    decisionKey: '',
    name: '',
    hitPolicy: 'UNIQUE',
    collectAggregator: '',
    inputs: [{ id: i1, expression: '' }],
    outputs: [{ id: o1, name: '', outputValues: '' }],
    rules: [{ id: r1, inputEntries: [''], outputEntries: [''] }],
    requiredDecisions: [],
  }
}

// ─── DMN XML serializer ───────────────────────────────────────────────────────

function escapeXml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function toXml(state: EditorState): string {
  const hp = state.hitPolicy
  const agg = state.collectAggregator ? ` aggregation="${state.collectAggregator}"` : ''
  const hitPolicyAttr = hp === 'UNIQUE' ? '' : ` hitPolicy="${hp}"${hp === 'COLLECT' ? agg : ''}`

  const inputsXml = state.inputs
    .map(
      (inp, i) =>
        `    <input id="input_${i}" label="${escapeXml(inp.expression)}">
      <inputExpression id="inputExpr_${i}" typeRef="string">
        <text>${escapeXml(inp.expression)}</text>
      </inputExpression>
    </input>`,
    )
    .join('\n')

  const outputsXml = state.outputs
    .map((out, i) => {
      const valList = out.outputValues.trim()
      const outputValuesEl = valList
        ? `\n      <outputValues><text>${escapeXml(valList)}</text></outputValues>`
        : ''
      return `    <output id="output_${i}" label="${escapeXml(out.name)}" name="${escapeXml(out.name)}" typeRef="string">${outputValuesEl}\n    </output>`
    })
    .join('\n')

  const rulesXml = state.rules
    .map((rule, ri) => {
      const inputEntries = rule.inputEntries
        .map(
          (e, ci) =>
            `      <inputEntry id="rule${ri}_in${ci}"><text>${escapeXml(e)}</text></inputEntry>`,
        )
        .join('\n')
      const outputEntries = rule.outputEntries
        .map(
          (e, ci) =>
            `      <outputEntry id="rule${ri}_out${ci}"><text>${escapeXml(e)}</text></outputEntry>`,
        )
        .join('\n')
      return `    <rule id="rule_${ri}">\n${inputEntries}\n${outputEntries}\n    </rule>`
    })
    .join('\n')

  const reqDecisions = state.requiredDecisions
    .map(
      (key, i) =>
        `  <informationRequirement id="req_${i}">
    <requiredDecision href="#${escapeXml(key)}" />
  </informationRequirement>`,
    )
    .join('\n')

  return `<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" xmlns:dmndi="https://www.omg.org/spec/DMN/20191111/DMNDI/" id="definitions_${escapeXml(state.decisionKey)}" name="${escapeXml(state.name || state.decisionKey)}" namespace="http://camunda.org/schema/1.0/dmn">
  <decision id="${escapeXml(state.decisionKey)}" name="${escapeXml(state.name || state.decisionKey)}">
${reqDecisions ? reqDecisions + '\n' : ''}    <decisionTable id="decisionTable_${escapeXml(state.decisionKey)}"${hitPolicyAttr}>
${inputsXml}
${outputsXml}
${rulesXml}
    </decisionTable>
  </decision>
</definitions>`
}

// ─── Load decision into editor state ─────────────────────────────────────────

function decisionToState(detail: ReturnType<typeof Object.create>): EditorState {
  const t = detail.table
  return {
    decisionKey: detail.decision_key,
    name: detail.name ?? '',
    hitPolicy: t.hit_policy as HitPolicy,
    collectAggregator: (t.collect_aggregator ?? '') as CollectAggregator | '',
    inputs: t.inputs.map((inp: { expression: string }) => ({ id: uid(), expression: inp.expression })),
    outputs: t.outputs.map((out: { name: string; output_values?: string[] }) => ({
      id: uid(),
      name: out.name,
      outputValues: (out.output_values ?? []).join(', '),
    })),
    rules: t.rules.map((r: { input_entries: string[]; output_entries: string[] }) => ({
      id: uid(),
      inputEntries: [...r.input_entries],
      outputEntries: [...r.output_entries],
    })),
    requiredDecisions: t.required_decisions ?? [],
  }
}

// ─── FEEL code assist ─────────────────────────────────────────────────────────

interface FEELSuggestion {
  label: string
  insert: string
  description: string
}

const FEEL_SUGGESTIONS: FEELSuggestion[] = [
  { label: '-',            insert: '-',           description: 'Match any value' },
  { label: '>= n',         insert: '>= ',         description: 'Greater than or equal' },
  { label: '<= n',         insert: '<= ',         description: 'Less than or equal' },
  { label: '> n',          insert: '> ',          description: 'Greater than' },
  { label: '< n',          insert: '< ',          description: 'Less than' },
  { label: '!= n',         insert: '!= ',         description: 'Not equal' },
  { label: '[a..b]',       insert: '[0..100]',    description: 'Inclusive range' },
  { label: '(a..b)',       insert: '(0..100)',    description: 'Exclusive range' },
  { label: '"value"',      insert: '""',          description: 'String literal' },
  { label: '"a","b"',      insert: '"",""',       description: 'OR list' },
  { label: 'not("v")',     insert: 'not("")',     description: 'Negate string match' },
  { label: 'true',         insert: 'true',        description: 'Boolean true' },
  { label: 'false',        insert: 'false',       description: 'Boolean false' },
  { label: 'null',         insert: 'null',        description: 'Null check' },
  { label: 'date("…")',    insert: 'date("")',    description: 'Date literal' },
]

const OUTPUT_SUGGESTIONS: FEELSuggestion[] = [
  { label: '"string"',           insert: '""',                  description: 'String literal' },
  { label: '42',                 insert: '42',                  description: 'Number literal' },
  { label: 'true',               insert: 'true',                description: 'Boolean true' },
  { label: 'false',              insert: 'false',               description: 'Boolean false' },
  { label: 'null',               insert: 'null',                description: 'Null value' },
  { label: 'if…then…else…',      insert: 'if  then "" else ""', description: 'Conditional' },
  { label: 'upper case(s)',      insert: 'upper case()',        description: 'Uppercase string' },
  { label: 'lower case(s)',      insert: 'lower case()',        description: 'Lowercase string' },
  { label: 'floor(n)',           insert: 'floor()',             description: 'Round down' },
  { label: 'ceiling(n)',         insert: 'ceiling()',           description: 'Round up' },
  { label: 'abs(n)',             insert: 'abs()',               description: 'Absolute value' },
  { label: 'decimal(n, scale)',  insert: 'decimal(, 2)',        description: 'Round to decimal places' },
  { label: 'string length(s)',   insert: 'string length()',     description: 'String length' },
  { label: 'contains(s, sub)',   insert: 'contains(, "")',      description: 'String contains' },
  { label: 'starts with(s, p)',  insert: 'starts with(, "")',   description: 'Starts with prefix' },
  { label: 'ends with(s, s)',    insert: 'ends with(, "")',     description: 'Ends with suffix' },
  { label: 'now()',              insert: 'now()',               description: 'Current date+time' },
  { label: 'today()',            insert: 'today()',             description: 'Current date' },
  { label: 'date("…")',          insert: 'date("")',            description: 'Date literal' },
  { label: 'duration("P…")',     insert: 'duration("P1D")',     description: 'Duration literal' },
]

// Lightweight JS mirror of the Rust mini-FEEL validator (input entry cells only)
function validateInputEntry(cell: string): boolean {
  const v = cell.trim()
  if (!v || v === '-') return true
  if (v === 'true' || v === 'false' || v === 'null') return true
  if (/^(>=|<=|>|<|!=)\s*.+$/.test(v)) return true
  // Range: [a..b] / (a..b) / mixed
  if (/^[\[(]\s*-?\d+(\.\d+)?\s*\.\.\s*-?\d+(\.\d+)?\s*[\])]$/.test(v)) return true
  // String literal or OR list: "a" or "a","b",...
  if (/^"[^"]*"(,\s*"[^"]*")*$/.test(v)) return true
  // not(...)
  if (/^not\(.+\)$/.test(v)) return true
  // date("...")
  if (/^date\("[^"]*"\)$/.test(v)) return true
  // Plain number
  if (/^-?\d+(\.\d+)?$/.test(v)) return true
  return false
}

function validateEditorState(s: EditorState): Record<string, string> {
  const errors: Record<string, string> = {}
  if (!s.decisionKey.trim()) {
    errors['key'] = 'Required'
  } else if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(s.decisionKey)) {
    errors['key'] = 'Letters, digits, underscores only; must start with a letter'
  }
  s.inputs.forEach((inp, i) => { if (!inp.expression.trim()) errors[`ih_${i}`] = 'Required' })
  s.outputs.forEach((out, i) => { if (!out.name.trim()) errors[`oh_${i}`] = 'Required' })
  s.rules.forEach((rule, ri) => {
    rule.inputEntries.forEach((entry, ci) => {
      if (!validateInputEntry(entry)) errors[`r${ri}_in${ci}`] = 'Invalid FEEL'
    })
  })
  return errors
}

// Parse a test input string to a JSON-compatible value
function parseTestValue(raw: string): unknown {
  const v = raw.trim()
  if (!v || v === 'null') return null
  if (v === 'true') return true
  if (v === 'false') return false
  const n = Number(v)
  if (!isNaN(n)) return n
  if (v.startsWith('"') && v.endsWith('"')) return v.slice(1, -1)
  return v
}

function filterSuggestions(value: string, pool: FEELSuggestion[]): FEELSuggestion[] {
  if (!value || value === '') return pool
  const v = value.toLowerCase()
  return pool.filter(
    s => s.insert.toLowerCase().startsWith(v) || s.label.toLowerCase().includes(v),
  )
}

interface FEELInputProps {
  value: string
  onChange: (v: string) => void
  placeholder?: string
  suggestions?: FEELSuggestion[]
  hasError?: boolean
}

function FEELInput({ value, onChange, placeholder, suggestions: pool = FEEL_SUGGESTIONS, hasError }: FEELInputProps) {
  const [open, setOpen] = useState(false)
  const [highlight, setHighlight] = useState(0)
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number } | null>(null)
  const blurTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const suggestions = filterSuggestions(value, pool)

  const select = (s: FEELSuggestion) => {
    onChange(s.insert)
    setOpen(false)
    setHighlight(0)
    inputRef.current?.focus()
  }

  const handleFocus = () => {
    if (blurTimer.current) clearTimeout(blurTimer.current)
    if (inputRef.current) {
      const rect = inputRef.current.getBoundingClientRect()
      setDropdownPos({ top: rect.bottom + 2, left: rect.left })
    }
    setOpen(true)
    setHighlight(0)
  }

  const handleBlur = () => {
    blurTimer.current = setTimeout(() => setOpen(false), 120)
  }

  const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!open) return
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setHighlight(h => Math.min(h + 1, suggestions.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setHighlight(h => Math.max(h - 1, 0))
    } else if (e.key === 'Enter' && suggestions[highlight]) {
      e.preventDefault()
      select(suggestions[highlight])
    } else if (e.key === 'Escape') {
      setOpen(false)
    }
  }

  return (
    <>
      <input
        ref={inputRef}
        style={{
          width: '100%',
          background: hasError ? 'rgba(243,139,168,0.08)' : 'transparent',
          border: 'none',
          padding: '5px 8px',
          fontSize: 12,
          color: 'inherit',
          outline: hasError ? '1px solid var(--color-error, #f38ba8)' : 'none',
          fontFamily: 'var(--font-mono, monospace)',
        }}
        value={value}
        onChange={e => { onChange(e.target.value); setHighlight(0) }}
        onFocus={handleFocus}
        onBlur={handleBlur}
        onKeyDown={handleKey}
        placeholder={placeholder ?? '-'}
      />
      {open && suggestions.length > 0 && dropdownPos && (
        <div
          style={{
            position: 'fixed',
            top: dropdownPos.top,
            left: dropdownPos.left,
            zIndex: 9999,
            minWidth: 240,
            background: 'var(--color-bg, #1e1e2e)',
            border: '1px solid var(--color-border, #45475a)',
            borderRadius: 6,
            boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
            overflow: 'hidden',
          }}
        >
          {suggestions.map((s, i) => (
            <div
              key={s.label}
              onMouseDown={() => select(s)}
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '5px 10px',
                cursor: 'pointer',
                background: i === highlight ? 'var(--color-accent-muted, #cba6f720)' : 'transparent',
                borderBottom: i < suggestions.length - 1 ? '1px solid var(--color-border, #45475a)' : 'none',
              }}
              onMouseEnter={() => setHighlight(i)}
            >
              <span style={{ fontFamily: 'var(--font-mono, monospace)', fontSize: 12 }}>{s.label}</span>
              <span style={{ fontSize: 11, color: 'var(--text-tertiary, #6c7086)', marginLeft: 12 }}>{s.description}</span>
            </div>
          ))}
        </div>
      )}
    </>
  )
}

// ─── Help panel ───────────────────────────────────────────────────────────────

function HelpSection({ title, children }: { title: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(true)
  return (
    <div style={{ marginBottom: 16 }}>
      <button
        onClick={() => setOpen(o => !o)}
        style={{
          background: 'none', border: 'none', cursor: 'pointer', padding: '4px 0',
          display: 'flex', alignItems: 'center', gap: 6, width: '100%',
          color: 'inherit', fontSize: 12, fontWeight: 600,
        }}
      >
        <span style={{ fontSize: 10, opacity: 0.6 }}>{open ? '▼' : '▶'}</span>
        {title}
      </button>
      {open && <div style={{ marginTop: 6 }}>{children}</div>}
    </div>
  )
}

const hRow: React.CSSProperties = {
  display: 'flex', gap: 8, padding: '3px 0',
  borderBottom: '1px solid var(--color-border, #45475a)',
  fontSize: 11,
}
const hCode: React.CSSProperties = {
  fontFamily: 'var(--font-mono, monospace)', fontSize: 11,
  color: 'var(--color-accent, #cba6f4)', whiteSpace: 'nowrap', minWidth: 100,
}
const hDesc: React.CSSProperties = { color: 'var(--text-tertiary, #6c7086)', fontSize: 11 }
const hFn: React.CSSProperties = {
  fontFamily: 'var(--font-mono, monospace)', fontSize: 10,
  color: 'var(--color-accent, #cba6f4)', padding: '2px 4px',
  background: 'var(--color-bg-secondary, #181825)', borderRadius: 3,
  display: 'inline-block', margin: '2px 2px 2px 0',
}
const hGroup: React.CSSProperties = { fontWeight: 600, fontSize: 10, color: 'var(--text-tertiary)', marginTop: 8, marginBottom: 4 }

function HelpPanel() {
  return (
    <div
      style={{
        width: 280, minWidth: 280,
        borderLeft: '1px solid var(--color-border, #45475a)',
        padding: '16px 14px',
        overflowY: 'auto',
        height: '100vh',
        position: 'sticky',
        top: 0,
        background: 'var(--color-bg, #1e1e2e)',
        flexShrink: 0,
      }}
    >
      <div style={{ fontWeight: 700, fontSize: 13, marginBottom: 16 }}>FEEL Reference</div>

      <HelpSection title="Input Entry Syntax">
        {[
          ['-',                'Match any value'],
          ['>= n',             'Greater than or equal'],
          ['<= n',             'Less than or equal'],
          ['> n / < n',        'Greater / less than'],
          ['!= n',             'Not equal'],
          ['[a..b]',           'Inclusive range'],
          ['(a..b)',           'Exclusive range'],
          ['[a..b)',           'Mixed range'],
          ['"string"',         'Exact string match'],
          ['"x","y"',          'In list (OR)'],
          ['not("x","y")',     'Not any of'],
          ['true / false',     'Boolean'],
          ['null',             'Null check'],
          ['date("2024-01-01")','Date literal'],
        ].map(([code, desc]) => (
          <div key={code} style={hRow}>
            <span style={hCode}>{code}</span>
            <span style={hDesc}>{desc}</span>
          </div>
        ))}
      </HelpSection>

      <HelpSection title="Hit Policies">
        {[
          ['UNIQUE',      'Exactly one rule matches'],
          ['FIRST',       'First match wins'],
          ['ANY',         'All matches must agree'],
          ['COLLECT',     'All matches collected; optional SUM/MIN/MAX/COUNT'],
          ['RULE ORDER',  'All matches in declaration order'],
          ['PRIORITY',    'Highest-priority output wins'],
          ['OUTPUT ORDER','Matches sorted by output priority list'],
        ].map(([pol, desc]) => (
          <div key={pol} style={hRow}>
            <span style={{ ...hCode, minWidth: 96 }}>{pol}</span>
            <span style={hDesc}>{desc}</span>
          </div>
        ))}
      </HelpSection>

      <HelpSection title="FEEL Functions">
        <div style={hGroup}>Numeric</div>
        {['abs(n)', 'floor(n)', 'ceiling(n)', 'decimal(n, scale)', 'modulo(n, d)', 'sqrt(n)'].map(f => (
          <span key={f} style={hFn}>{f}</span>
        ))}

        <div style={hGroup}>String</div>
        {[
          'string length(s)', 'upper case(s)', 'lower case(s)',
          'substring(s, start, len?)', 'contains(s, sub)',
          'starts with(s, pre)', 'ends with(s, suf)',
          'matches(s, pattern)', 'replace(s, pattern, rep)',
        ].map(f => (
          <span key={f} style={hFn}>{f}</span>
        ))}

        <div style={hGroup}>List</div>
        {[
          'list contains(list, item)', 'count(list)', 'min(list)', 'max(list)',
          'sum(list)', 'mean(list)', 'append(list, item)',
          'flatten(list)', 'distinct values(list)',
        ].map(f => (
          <span key={f} style={hFn}>{f}</span>
        ))}

        <div style={hGroup}>Date / Time</div>
        {[
          'date("2024-01-01")', 'time("12:00:00")',
          'date and time("…")', 'duration("P1D")',
          'now()', 'today()',
        ].map(f => (
          <span key={f} style={hFn}>{f}</span>
        ))}
      </HelpSection>

      <HelpSection title="Data Wiring">
        <div style={hGroup}>Input column expression</div>
        <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 6, lineHeight: 1.6 }}>
          Each column header is a <em>process variable name</em>. The engine looks up that variable
          in the current BPMN execution context and passes it as the input value for that column.
          Example: column <code style={hCode}>age</code> reads process variable <code style={hCode}>age</code>.
        </div>

        <div style={hGroup}>Input entry cells — mini-FEEL</div>
        <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 6, lineHeight: 1.6 }}>
          Unary tests only — no stdlib functions. Each cell is tested against the column value.
          Use <code style={hCode}>-</code> to match any value. Invalid entries are highlighted in red.
        </div>

        <div style={hGroup}>Output entry cells — full FEEL</div>
        <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 6, lineHeight: 1.6 }}>
          Full FEEL expressions evaluated by dsntk. May reference process variables by name,
          call any stdlib function, or use <code style={hCode}>if … then … else</code>.
          Example: <code style={hCode}>upper case(status)</code> reads variable <code style={hCode}>status</code>.
        </div>

        <div style={hGroup}>BusinessRuleTask in BPMN</div>
        <div style={{ fontSize: 11, color: 'var(--text-tertiary)', lineHeight: 1.6 }}>
          Set <code style={hCode}>camunda:decisionRef</code> to the <em>Decision Key</em> of this table.
          All current process variables flow in as context. Each output column name is written
          back as a process variable after evaluation — available immediately in the next element.
        </div>
      </HelpSection>
    </div>
  )
}

// ─── Cell styles ─────────────────────────────────────────────────────────────

const cellStyle: React.CSSProperties = {
  padding: 0,
  border: '1px solid var(--color-border, #45475a)',
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  background: 'transparent',
  border: 'none',
  padding: '5px 8px',
  fontSize: 12,
  color: 'inherit',
  outline: 'none',
  fontFamily: 'var(--font-mono, monospace)',
}

// ─── Component ───────────────────────────────────────────────────────────────

export default function DecisionTableEditor() {
  const { key, groupId } = useParams<{ key?: string; groupId?: string }>()
  const isNew = !key || key === 'new'
  const { org } = useOrg()
  const navigate = useNavigate()
  const qc = useQueryClient()
  const decisionsBase = groupId ? `/process-groups/${groupId}/decisions` : '/decisions'

  const [state, setState] = useState<EditorState>(emptyState)
  const [error, setError] = useState<string | null>(null)
  const [cellErrors, setCellErrors] = useState<Record<string, string>>({})
  const [showHelp, setShowHelp] = useState(false)
  const [showTest, setShowTest] = useState(false)
  const [testInputs, setTestInputs] = useState<Record<string, string>>({})
  const [testResult, setTestResult] = useState<TestResult | null>(null)
  const [testPending, setTestPending] = useState(false)
  const [draftBanner, setDraftBanner] = useState<{ state: EditorState; savedAt: string } | null>(null)
  const [draftSaved, setDraftSaved] = useState(false)
  const [drdSearch, setDrdSearch] = useState('')
  const [drdOpen, setDrdOpen] = useState(false)
  const [drdRemovingKey, setDrdRemovingKey] = useState<string | null>(null)
  const drdBlurTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const draftKey = org ? `conduit_draft_${org.id}_${isNew ? 'new' : key ?? 'new'}` : null

  // Load existing decision when editing
  const { data: existing, isLoading } = useQuery({
    queryKey: ['decision', org?.id, key],
    queryFn: () => fetchDecision(org!.id, key!),
    enabled: !!org && !isNew,
  })

  // All deployed decisions — for the DRD "required decisions" panel
  const { data: allDecisions = [] } = useQuery({
    queryKey: ['decisions', org?.id],
    queryFn: () => fetchDecisions(org!.id),
    enabled: !!org,
  })

  useEffect(() => {
    if (existing) {
      setState(decisionToState(existing))
    }
  }, [existing])

  // Offer to restore a previously saved draft
  useEffect(() => {
    if (!draftKey) return
    const raw = localStorage.getItem(draftKey)
    if (!raw) return
    try {
      const saved = JSON.parse(raw) as { state: EditorState; savedAt: string }
      setDraftBanner(saved)
    } catch {
      localStorage.removeItem(draftKey)
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draftKey])

  const deploy = useMutation({
    mutationFn: (xml: string) => deployDecision(org!.id, xml, groupId),
    onSuccess: () => {
      if (draftKey) localStorage.removeItem(draftKey)
      qc.invalidateQueries({ queryKey: ['decisions', org?.id] })
      navigate(decisionsBase)
    },
    onError: (e: Error) => setError(e.message),
  })

  const handleDeploy = useCallback(() => {
    setError(null)
    const errs = validateEditorState(state)
    setCellErrors(errs)
    const count = Object.keys(errs).length
    if (count > 0) {
      setError(`Fix ${count} error${count > 1 ? 's' : ''} before deploying (highlighted in red)`)
      return
    }
    deploy.mutate(toXml(state))
  }, [state, deploy])

  const saveDraft = useCallback(() => {
    if (!draftKey) return
    localStorage.setItem(draftKey, JSON.stringify({ state, savedAt: new Date().toISOString() }))
    setDraftSaved(true)
    setDraftBanner(null)
    setTimeout(() => setDraftSaved(false), 2000)
  }, [state, draftKey])

  const restoreDraft = useCallback(() => {
    if (!draftBanner) return
    setState(draftBanner.state)
    setCellErrors({})
    setDraftBanner(null)
  }, [draftBanner])

  const discardDraft = useCallback(() => {
    if (draftKey) localStorage.removeItem(draftKey)
    setDraftBanner(null)
  }, [draftKey])

  const runTest = useCallback(async () => {
    setTestPending(true)
    setTestResult(null)
    const context: Record<string, unknown> = {}
    state.inputs.forEach(inp => {
      if (!inp.expression.trim()) return
      context[inp.expression] = parseTestValue(testInputs[inp.id] ?? '')
    })
    try {
      const result = await testDecision(org!.id, toXml(state), context)
      setTestResult(result)
    } catch (e: unknown) {
      setTestResult({ error: 'ERROR', message: (e as Error).message })
    } finally {
      setTestPending(false)
    }
  }, [state, testInputs, org])

  // ── Helpers to mutate state ───────────────────────────────────────────────

  const setField = <K extends keyof EditorState>(k: K, v: EditorState[K]) =>
    setState(s => ({ ...s, [k]: v }))

  const addInput = () =>
    setState(s => ({
      ...s,
      inputs: [...s.inputs, { id: uid(), expression: '' }],
      rules: s.rules.map(r => ({ ...r, inputEntries: [...r.inputEntries, '-'] })),
    }))

  const removeInput = (idx: number) =>
    setState(s => ({
      ...s,
      inputs: s.inputs.filter((_, i) => i !== idx),
      rules: s.rules.map(r => ({
        ...r,
        inputEntries: r.inputEntries.filter((_, i) => i !== idx),
      })),
    }))

  const setInputExpr = (idx: number, val: string) =>
    setState(s => ({
      ...s,
      inputs: s.inputs.map((inp, i) => (i === idx ? { ...inp, expression: val } : inp)),
    }))

  const addOutput = () =>
    setState(s => ({
      ...s,
      outputs: [...s.outputs, { id: uid(), name: '', outputValues: '' }],
      rules: s.rules.map(r => ({ ...r, outputEntries: [...r.outputEntries, ''] })),
    }))

  const removeOutput = (idx: number) =>
    setState(s => ({
      ...s,
      outputs: s.outputs.filter((_, i) => i !== idx),
      rules: s.rules.map(r => ({
        ...r,
        outputEntries: r.outputEntries.filter((_, i) => i !== idx),
      })),
    }))

  const setOutputField = (idx: number, field: 'name' | 'outputValues', val: string) =>
    setState(s => ({
      ...s,
      outputs: s.outputs.map((o, i) => (i === idx ? { ...o, [field]: val } : o)),
    }))

  const addRule = () =>
    setState(s => ({
      ...s,
      rules: [
        ...s.rules,
        {
          id: uid(),
          inputEntries: s.inputs.map(() => '-'),
          outputEntries: s.outputs.map(() => ''),
        },
      ],
    }))

  const removeRule = (idx: number) =>
    setState(s => ({ ...s, rules: s.rules.filter((_, i) => i !== idx) }))

  const setRuleEntry = (rIdx: number, col: 'input' | 'output', cIdx: number, val: string) =>
    setState(s => ({
      ...s,
      rules: s.rules.map((r, i) => {
        if (i !== rIdx) return r
        if (col === 'input') {
          const e = [...r.inputEntries]; e[cIdx] = val; return { ...r, inputEntries: e }
        } else {
          const e = [...r.outputEntries]; e[cIdx] = val; return { ...r, outputEntries: e }
        }
      }),
    }))

  const toggleRequired = (decisionKey: string) =>
    setState(s => ({
      ...s,
      requiredDecisions: s.requiredDecisions.includes(decisionKey)
        ? s.requiredDecisions.filter(k => k !== decisionKey)
        : [...s.requiredDecisions, decisionKey],
    }))

  // ── Guard ─────────────────────────────────────────────────────────────────

  if (!org) {
    return <div className="empty-state"><p>Select an organisation first.</p></div>
  }
  if (!isNew && isLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  const showAggregator = state.hitPolicy === 'COLLECT'
  const showOutputValues = state.hitPolicy === 'PRIORITY' || state.hitPolicy === 'OUTPUT_ORDER'

  // Other decisions that can be required (exclude this one)
  const candidateRequirements = allDecisions.filter(d => d.decision_key !== state.decisionKey)

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div style={{ display: 'flex', alignItems: 'flex-start', overflow: 'hidden', height: '100%' }}>
      {/* Main editor */}
      <div style={{ flex: 1, padding: 24, overflowY: 'auto', overflowX: 'auto', minWidth: 0 }}>
        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 20 }}>
          <div>
            <h1 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>
              {isNew ? 'New Decision Table' : `Edit: ${state.name || state.decisionKey}`}
            </h1>
            {!isNew && (
              <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: '4px 0 0' }}>
                Saving creates a new version.
              </p>
            )}
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            <button
              onClick={() => setShowHelp(h => !h)}
              style={{ fontSize: 11, opacity: showHelp ? 1 : 0.7 }}
              title="Toggle FEEL reference panel"
            >
              {showHelp ? 'Hide help' : '? Help'}
            </button>
            <button
              onClick={() => setShowTest(t => !t)}
              style={{ fontSize: 11, opacity: showTest ? 1 : 0.7 }}
              title="Test the decision table with sample inputs"
            >
              {showTest ? 'Hide test' : '▶ Test'}
            </button>
            <button
              onClick={saveDraft}
              style={{ fontSize: 11 }}
              title="Save a local draft (not deployed)"
            >
              {draftSaved ? '✓ Saved' : 'Save draft'}
            </button>
            <button onClick={() => navigate(decisionsBase)}>Cancel</button>
            <button className="btn-primary" onClick={handleDeploy} disabled={deploy.isPending}>
              {deploy.isPending ? 'Deploying…' : 'Deploy'}
            </button>
          </div>
        </div>

        {draftBanner && (
          <div style={{ background: 'var(--color-bg-secondary, #181825)', border: '1px solid var(--color-border, #45475a)', borderRadius: 6, padding: '8px 12px', marginBottom: 12, fontSize: 12, display: 'flex', alignItems: 'center', gap: 12 }}>
            <span style={{ color: 'var(--text-tertiary)' }}>
              Unsaved draft from {new Date(draftBanner.savedAt).toLocaleString()}
            </span>
            <button onClick={restoreDraft} style={{ fontSize: 11, padding: '2px 8px' }}>Restore</button>
            <button onClick={discardDraft} style={{ fontSize: 11, padding: '2px 8px' }}>Discard</button>
          </div>
        )}

        {error && (
          <div style={{ background: 'var(--color-error-bg, #f38ba820)', border: '1px solid var(--color-error, #f38ba8)', borderRadius: 6, padding: '8px 12px', marginBottom: 16, fontSize: 12, color: 'var(--color-error, #f38ba8)' }}>
            {error}
          </div>
        )}

        {/* Meta fields */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr auto auto', gap: 12, marginBottom: 20, alignItems: 'end' }}>
          <div>
            <label style={{ display: 'block', fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 4 }}>Decision Key *</label>
            <input
              style={{ width: '100%' }}
              value={state.decisionKey}
              onChange={e => setField('decisionKey', e.target.value)}
              placeholder="e.g. ageCategory"
              disabled={!isNew}
            />
          </div>
          <div>
            <label style={{ display: 'block', fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 4 }}>Name</label>
            <input
              style={{ width: '100%' }}
              value={state.name}
              onChange={e => setField('name', e.target.value)}
              placeholder="e.g. Age Category"
              autoFocus={isNew}
            />
          </div>
          <div>
            <label style={{ display: 'block', fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 4 }}>Hit Policy</label>
            <select value={state.hitPolicy} onChange={e => setField('hitPolicy', e.target.value as HitPolicy)}>
              {HIT_POLICIES.map(hp => <option key={hp} value={hp}>{hp}</option>)}
            </select>
          </div>
          {showAggregator && (
            <div>
              <label style={{ display: 'block', fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 4 }}>Aggregator</label>
              <select
                value={state.collectAggregator}
                onChange={e => setField('collectAggregator', e.target.value as CollectAggregator | '')}
              >
                <option value="">— none —</option>
                {COLLECT_AGGREGATORS.map(a => <option key={a} value={a}>{a}</option>)}
              </select>
            </div>
          )}
        </div>

        {/* DRD panel */}
        {(() => {
          const drdCandidates = candidateRequirements.filter(d => {
            if (state.requiredDecisions.includes(d.decision_key)) return false
            if (!drdSearch.trim()) return true
            const q = drdSearch.toLowerCase()
            return (d.name ?? d.decision_key).toLowerCase().includes(q)
          })
          return (
            <div style={{ marginBottom: 20 }}>
              <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>Required Decisions (DRD inputs)</h3>

              {/* Chips for selected requirements */}
              {state.requiredDecisions.length > 0 && (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 8 }}>
                  {state.requiredDecisions.map(reqKey => {
                    const dec = allDecisions.find(d => d.decision_key === reqKey)
                    const label = dec?.name ?? reqKey
                    const confirming = drdRemovingKey === reqKey
                    return (
                      <span
                        key={reqKey}
                        style={{
                          display: 'inline-flex', alignItems: 'center', gap: 6,
                          padding: '3px 8px', borderRadius: 12,
                          fontSize: 12, fontFamily: 'var(--font-mono, monospace)',
                          background: 'var(--color-accent-muted, #cba6f720)',
                          border: '1px solid var(--color-accent, #cba6f4)',
                          color: 'var(--color-accent, #cba6f4)',
                        }}
                      >
                        {label}
                        {confirming ? (
                          <>
                            <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Remove?</span>
                            <button
                              onClick={() => { toggleRequired(reqKey); setDrdRemovingKey(null) }}
                              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--color-error, #f38ba8)', fontSize: 11, padding: '0 2px', lineHeight: 1 }}
                            >Yes</button>
                            <button
                              onClick={() => setDrdRemovingKey(null)}
                              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-tertiary)', fontSize: 11, padding: '0 2px', lineHeight: 1 }}
                            >No</button>
                          </>
                        ) : (
                          <button
                            onClick={() => setDrdRemovingKey(reqKey)}
                            style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'inherit', fontSize: 14, lineHeight: 1, padding: '0 1px', opacity: 0.7 }}
                            title="Remove"
                          >×</button>
                        )}
                      </span>
                    )
                  })}
                </div>
              )}

              {/* Search dropdown */}
              {candidateRequirements.length > 0 && (
                <div style={{ position: 'relative', maxWidth: 320 }}>
                  <input
                    value={drdSearch}
                    onChange={e => { setDrdSearch(e.target.value); setDrdOpen(true) }}
                    onFocus={() => setDrdOpen(true)}
                    onBlur={() => { drdBlurTimer.current = setTimeout(() => setDrdOpen(false), 120) }}
                    placeholder="Add required decision…"
                    style={{ width: '100%', fontSize: 12 }}
                  />
                  {drdOpen && drdCandidates.length > 0 && (
                    <div
                      style={{
                        position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 200,
                        background: 'var(--color-bg, #1e1e2e)',
                        border: '1px solid var(--color-border, #45475a)',
                        borderRadius: 6, marginTop: 2,
                        boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
                        maxHeight: 200, overflowY: 'auto',
                      }}
                    >
                      {drdCandidates.map(d => (
                        <div
                          key={d.decision_key}
                          onMouseDown={() => {
                            if (drdBlurTimer.current) clearTimeout(drdBlurTimer.current)
                            toggleRequired(d.decision_key)
                            setDrdSearch('')
                            setDrdOpen(false)
                          }}
                          style={{
                            padding: '6px 10px', cursor: 'pointer', fontSize: 12,
                            borderBottom: '1px solid var(--color-border, #45475a)',
                          }}
                          onMouseEnter={e => (e.currentTarget.style.background = 'var(--color-accent-muted, #cba6f720)')}
                          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
                        >
                          <span style={{ fontFamily: 'var(--font-mono, monospace)' }}>{d.name ?? d.decision_key}</span>
                          {d.name && (
                            <span style={{ marginLeft: 8, fontSize: 11, color: 'var(--text-tertiary)' }}>{d.decision_key}</span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                  {drdOpen && drdCandidates.length === 0 && drdSearch.trim() && (
                    <div
                      style={{
                        position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 200,
                        background: 'var(--color-bg, #1e1e2e)',
                        border: '1px solid var(--color-border, #45475a)',
                        borderRadius: 6, marginTop: 2,
                        padding: '8px 10px', fontSize: 12,
                        color: 'var(--text-tertiary)',
                      }}
                    >
                      No matching decisions
                    </div>
                  )}
                </div>
              )}
            </div>
          )
        })()}

        {/* Decision table grid */}
        <div style={{ overflowX: 'auto' }}>
          <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: 12 }}>
            <thead>
              {/* Column group header */}
              <tr>
                <th style={{ ...cellStyle, background: 'var(--color-bg-secondary)', fontSize: 10, color: 'var(--text-tertiary)', padding: '4px 8px', width: 32 }}>#</th>
                <th
                  colSpan={state.inputs.length}
                  style={{ ...cellStyle, background: '#1e3a5f40', padding: '2px 6px 2px 8px' }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={{ color: 'var(--text-tertiary)', fontSize: 10 }}>INPUT</span>
                    <button
                      onClick={addInput}
                      style={{ background: 'none', border: '1px solid var(--color-border, #45475a)', borderRadius: 3, cursor: 'pointer', padding: '0 5px', color: 'var(--text-tertiary)', fontSize: 13, lineHeight: '18px' }}
                      title="Add input column"
                    >+</button>
                  </div>
                </th>
                <th
                  colSpan={state.outputs.length}
                  style={{ ...cellStyle, background: '#1a3a2040', padding: '2px 6px 2px 8px' }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={{ color: 'var(--text-tertiary)', fontSize: 10 }}>OUTPUT</span>
                    <button
                      onClick={addOutput}
                      style={{ background: 'none', border: '1px solid var(--color-border, #45475a)', borderRadius: 3, cursor: 'pointer', padding: '0 5px', color: 'var(--text-tertiary)', fontSize: 13, lineHeight: '18px' }}
                      title="Add output column"
                    >+</button>
                  </div>
                </th>
                <th style={{ ...cellStyle, background: 'var(--color-bg-secondary)', width: 32 }} />
              </tr>
              {/* Input/output headers */}
              <tr>
                <th style={{ ...cellStyle, background: 'var(--color-bg-secondary)', width: 32 }} />
                {state.inputs.map((inp, i) => (
                  <th key={inp.id} style={{ ...cellStyle, background: '#1e3a5f40', minWidth: 140, outline: cellErrors[`ih_${i}`] ? '1px solid var(--color-error, #f38ba8)' : undefined }}>
                    <div style={{ display: 'flex', alignItems: 'center' }}>
                      <input
                        style={{ ...inputStyle, flex: 1, fontWeight: 600 }}
                        value={inp.expression}
                        onChange={e => { setInputExpr(i, e.target.value); setCellErrors(e => { const n = { ...e }; delete n[`ih_${i}`]; return n }) }}
                        placeholder="expression"
                      />
                      {state.inputs.length > 1 && (
                        <button
                          onClick={() => removeInput(i)}
                          style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '4px', color: 'var(--text-tertiary)', fontSize: 14, lineHeight: 1 }}
                          title="Remove input"
                        >×</button>
                      )}
                    </div>
                  </th>
                ))}
                {state.outputs.map((out, i) => (
                  <th key={out.id} style={{ ...cellStyle, background: '#1a3a2040', minWidth: 140, outline: cellErrors[`oh_${i}`] ? '1px solid var(--color-error, #f38ba8)' : undefined }}>
                    <div style={{ display: 'flex', flexDirection: 'column' }}>
                      <div style={{ display: 'flex', alignItems: 'center' }}>
                        <input
                          style={{ ...inputStyle, flex: 1, fontWeight: 600 }}
                          value={out.name}
                          onChange={e => { setOutputField(i, 'name', e.target.value); setCellErrors(e => { const n = { ...e }; delete n[`oh_${i}`]; return n }) }}
                          placeholder="variable name"
                        />
                        {state.outputs.length > 1 && (
                          <button
                            onClick={() => removeOutput(i)}
                            style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '4px', color: 'var(--text-tertiary)', fontSize: 14, lineHeight: 1 }}
                            title="Remove output"
                          >×</button>
                        )}
                      </div>
                      {showOutputValues && (
                        <input
                          style={{ ...inputStyle, fontSize: 10, borderTop: '1px solid var(--color-border, #45475a)' }}
                          value={out.outputValues}
                          onChange={e => setOutputField(i, 'outputValues', e.target.value)}
                          placeholder='priority list, e.g. "high","medium","low"'
                        />
                      )}
                    </div>
                  </th>
                ))}
                <th style={{ ...cellStyle, background: 'var(--color-bg-secondary)', width: 32 }} />
              </tr>
            </thead>
            <tbody>
              {state.rules.map((rule, ri) => (
                <tr key={rule.id}>
                  <td style={{ ...cellStyle, textAlign: 'center', color: 'var(--text-tertiary)', fontSize: 11, padding: '0 6px', background: 'var(--color-bg-secondary)' }}>
                    {ri + 1}
                  </td>
                  {rule.inputEntries.map((entry, ci) => (
                    <td key={ci} style={{ ...cellStyle, background: '#1e3a5f18' }}>
                      <FEELInput
                        value={entry}
                        onChange={v => { setRuleEntry(ri, 'input', ci, v); setCellErrors(e => { const n = { ...e }; delete n[`r${ri}_in${ci}`]; return n }) }}
                        placeholder="-"
                        hasError={!!cellErrors[`r${ri}_in${ci}`]}
                      />
                    </td>
                  ))}
                  {rule.outputEntries.map((entry, ci) => (
                    <td key={ci} style={{ ...cellStyle, background: '#1a3a2018' }}>
                      <FEELInput
                        value={entry}
                        onChange={v => setRuleEntry(ri, 'output', ci, v)}
                        placeholder='"adult"'
                        suggestions={OUTPUT_SUGGESTIONS}
                      />
                    </td>
                  ))}
                  <td style={{ ...cellStyle, textAlign: 'center', background: 'var(--color-bg-secondary)' }}>
                    <button
                      onClick={() => removeRule(ri)}
                      style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '4px', color: 'var(--text-tertiary)', fontSize: 14, lineHeight: 1 }}
                      title="Remove rule"
                    >×</button>
                  </td>
                </tr>
              ))}
              <tr>
                <td
                  style={{ ...cellStyle, textAlign: 'center', background: 'var(--color-bg-secondary)', padding: 0 }}
                >
                  <button
                    onClick={addRule}
                    style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '3px 8px', color: 'var(--text-tertiary)', fontSize: 16, lineHeight: 1, width: '100%' }}
                    title="Add rule"
                  >+</button>
                </td>
                <td
                  colSpan={state.inputs.length + state.outputs.length + 1}
                  style={{ ...cellStyle, background: 'transparent', cursor: 'pointer' }}
                  onClick={addRule}
                />
              </tr>
            </tbody>
          </table>
        </div>

        {/* Test panel */}
        {showTest && (
          <div style={{ marginTop: 20, padding: 16, border: '1px solid var(--color-border, #45475a)', borderRadius: 8 }}>
            <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 12, margin: '0 0 12px 0' }}>Test with sample inputs</h3>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(180px, 1fr))', gap: 10, marginBottom: 12 }}>
              {state.inputs.map(inp => (
                <div key={inp.id}>
                  <label style={{ fontSize: 11, color: 'var(--text-tertiary)', display: 'block', marginBottom: 4 }}>
                    {inp.expression || '(unnamed)'}
                  </label>
                  <input
                    style={{ width: '100%', fontFamily: 'var(--font-mono, monospace)', fontSize: 12, boxSizing: 'border-box' }}
                    value={testInputs[inp.id] ?? ''}
                    onChange={e => setTestInputs(t => ({ ...t, [inp.id]: e.target.value }))}
                    placeholder='e.g. 25, "gold", true'
                  />
                </div>
              ))}
            </div>
            <button className="btn-primary" onClick={runTest} disabled={testPending} style={{ fontSize: 12 }}>
              {testPending ? 'Running…' : 'Run test'}
            </button>
            {testResult && (
              <div style={{ marginTop: 12, fontSize: 12 }}>
                {testResult.error ? (
                  <div style={{ color: 'var(--color-error, #f38ba8)' }}>
                    {testResult.error === 'NO_MATCH'
                      ? 'No rule matched the input values.'
                      : testResult.error === 'MULTIPLE_MATCHES'
                      ? 'Multiple rules matched (UNIQUE hit policy).'
                      : testResult.message ?? testResult.error}
                  </div>
                ) : (
                  <div>
                    <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 4 }}>Output:</div>
                    <pre style={{ margin: 0, fontFamily: 'var(--font-mono, monospace)', fontSize: 12, background: 'var(--color-bg-secondary)', padding: '8px 12px', borderRadius: 6 }}>
                      {JSON.stringify(testResult.output, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Help panel */}
      {showHelp && <HelpPanel />}
    </div>
  )
}
