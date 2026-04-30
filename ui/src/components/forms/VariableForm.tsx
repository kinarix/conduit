import { useMemo, useState, useImperativeHandle, forwardRef } from 'react'
import {
  parseFields,
  coerce,
  valueTypeFor,
  validateField,
  type SchemaField,
} from './schema'
import styles from './VariableForm.module.css'

export interface VariableSubmission {
  name: string
  value_type: string
  value: unknown
}

export interface VariableFormHandle {
  /** Validate + coerce. Returns null when invalid (errors already shown). */
  collect: () => VariableSubmission[] | null
}

interface Props {
  /** JSON schema string (process input schema). When falsy, the form falls
   *  back to a free-form JSON textarea. */
  schema?: string
  /** Optional initial values keyed by field name. Strings/numbers/booleans only. */
  initial?: Record<string, string | number | boolean>
}

type Raw = Record<string, string | boolean>

export default forwardRef<VariableFormHandle, Props>(function VariableForm(
  { schema, initial }: Props,
  ref,
) {
  const parsed = useMemo(() => parseFields(schema), [schema])
  const hasSchema = parsed.fields.length > 0

  // Schema-driven path
  const [raw, setRaw] = useState<Raw>(() => {
    const r: Raw = {}
    for (const f of parsed.fields) {
      const v = initial?.[f.name]
      if (f.type === 'boolean') r[f.name] = Boolean(v ?? false)
      else r[f.name] = v == null ? '' : String(v)
    }
    return r
  })
  const [errors, setErrors] = useState<Record<string, string>>({})

  // Fallback JSON path
  const [json, setJson] = useState(() =>
    initial ? JSON.stringify(initial, null, 2) : '',
  )
  const [jsonError, setJsonError] = useState<string | null>(null)

  useImperativeHandle(ref, () => ({
    collect: () => {
      if (hasSchema) {
        const nextErrors: Record<string, string> = {}
        const subs: VariableSubmission[] = []
        for (const f of parsed.fields) {
          const v = raw[f.name]
          const err = validateField(f, v)
          if (err) {
            nextErrors[f.name] = err
            continue
          }
          // Optional and empty → skip the field entirely.
          if (f.optional && (v === '' || v === undefined)) continue
          try {
            const value = coerce(f, v)
            if (value === undefined) continue
            subs.push({ name: f.name, value_type: valueTypeFor(f), value })
          } catch (e) {
            nextErrors[f.name] = (e as Error).message
          }
        }
        setErrors(nextErrors)
        if (Object.keys(nextErrors).length) return null
        return subs
      }

      // Fallback: parse JSON object → list of var submissions.
      if (!json.trim()) return []
      try {
        const obj = JSON.parse(json)
        if (typeof obj !== 'object' || obj === null || Array.isArray(obj)) {
          throw new Error('Expected a JSON object')
        }
        setJsonError(null)
        return Object.entries(obj).map(([name, value]) => ({
          name,
          value_type:
            typeof value === 'number'
              ? 'integer'
              : typeof value === 'boolean'
              ? 'boolean'
              : 'string',
          value,
        }))
      } catch (e) {
        setJsonError((e as Error).message)
        return null
      }
    },
  }))

  if (hasSchema) {
    return (
      <div className={styles.form}>
        {parsed.fields.map(f => (
          <FieldInput
            key={f.name}
            field={f}
            value={raw[f.name]}
            error={errors[f.name]}
            onChange={v => setRaw(prev => ({ ...prev, [f.name]: v }))}
          />
        ))}
      </div>
    )
  }

  return (
    <div className={styles.form}>
      <div className={styles.fallbackHeader}>
        No input schema defined for this process. Provide variables as a JSON object (optional).
      </div>
      <textarea
        rows={6}
        className={styles.jsonArea}
        value={json}
        placeholder={'{\n  "key": "value"\n}'}
        onChange={e => {
          setJson(e.target.value)
          setJsonError(null)
        }}
      />
      {jsonError && <div className={styles.errorText}>{jsonError}</div>}
    </div>
  )
})

function FieldInput({
  field,
  value,
  error,
  onChange,
}: {
  field: SchemaField
  value: string | boolean
  error: string | undefined
  onChange: (v: string | boolean) => void
}) {
  return (
    <div className={styles.field}>
      <label className={styles.label}>
        {field.name}
        {!field.optional && <span className={styles.required}>*</span>}
        <span className={styles.typeChip}>
          {field.type}
          {field.nullable ? '?' : ''}
        </span>
      </label>

      {field.type === 'boolean' ? (
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={Boolean(value)}
            onChange={e => onChange(e.target.checked)}
          />
          <span>{Boolean(value) ? 'true' : 'false'}</span>
        </label>
      ) : (
        <input
          type={field.type === 'integer' || field.type === 'number' ? 'number' : 'text'}
          inputMode={field.type === 'integer' ? 'numeric' : undefined}
          step={field.type === 'integer' ? 1 : 'any'}
          className={`${styles.input} ${error ? styles.error : ''}`}
          value={typeof value === 'string' ? value : ''}
          pattern={field.type === 'string' ? field.pattern : undefined}
          onChange={e => onChange(e.target.value)}
        />
      )}

      {field.description && !error && (
        <div className={styles.helpText}>{field.description}</div>
      )}
      {error && <div className={styles.errorText}>{error}</div>}
    </div>
  )
}
