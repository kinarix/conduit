/* Shared JSON-schema helpers used by both the schema builder (modeller) and
 * the run-time variable forms (Start instance, Complete task).
 *
 * Schema shape we support:
 *
 *   {
 *     "type": "object",
 *     "properties": {
 *       "amount": { "type": "integer", "description": "...", ... },
 *       "approved": { "type": ["boolean", "null"] },
 *       "name": { "type": "string", "pattern": "^[A-Z]+$" }
 *     },
 *     "required": ["amount"],
 *     "additionalProperties": false
 *   }
 */

export type FieldType = 'string' | 'number' | 'integer' | 'boolean'

export interface SchemaField {
  name: string
  type: FieldType
  optional: boolean
  nullable: boolean
  pattern?: string
  description?: string
}

export interface ParsedSchema {
  fields: SchemaField[]
  additionalProperties: boolean
}

export const FIELD_TYPES: readonly FieldType[] = ['string', 'number', 'integer', 'boolean']

export function parseFields(schemaStr: string | undefined): ParsedSchema {
  if (!schemaStr) return { fields: [], additionalProperties: false }
  try {
    const schema = JSON.parse(schemaStr) as {
      properties?: Record<string, { type?: string | string[]; description?: string; pattern?: string }>
      required?: string[]
      additionalProperties?: boolean
    }
    const props = schema.properties ?? {}
    const required = schema.required ?? []
    const fields: SchemaField[] = Object.entries(props).map(([name, def]) => {
      const rawType = def.type
      const typeArr = Array.isArray(rawType) ? rawType : [rawType ?? 'string']
      const nullable = typeArr.includes('null')
      const baseType = typeArr.find(t => t !== 'null') ?? 'string'
      return {
        name,
        type: (FIELD_TYPES as readonly string[]).includes(baseType)
          ? (baseType as FieldType)
          : 'string',
        optional: !required.includes(name),
        nullable,
        pattern: def.pattern,
        description: def.description,
      }
    })
    return { fields, additionalProperties: schema.additionalProperties === true }
  } catch {
    return { fields: [], additionalProperties: false }
  }
}

export function buildSchemaStr(fields: SchemaField[], additionalProperties: boolean): string | undefined {
  const valid = fields.filter(f => f.name.trim())
  if (valid.length === 0 && !additionalProperties) return undefined
  const properties: Record<string, unknown> = {}
  const required: string[] = []
  for (const f of valid) {
    const baseType = f.nullable ? [f.type, 'null'] : f.type
    const def: Record<string, unknown> = { type: baseType }
    if (f.description) def.description = f.description
    if (f.type === 'string' && f.pattern) def.pattern = f.pattern
    properties[f.name] = def
    if (!f.optional) required.push(f.name)
  }
  const schema: Record<string, unknown> = { type: 'object', properties }
  if (required.length) schema.required = required
  if (additionalProperties) schema.additionalProperties = true
  return JSON.stringify(schema, null, 2)
}

/* ── Coercion: string → typed value ────────────────────────────────────── */

export function coerce(field: SchemaField, raw: string | boolean): unknown {
  if (field.type === 'boolean') return Boolean(raw)
  if (raw === '' || raw === null || raw === undefined) return field.nullable ? null : undefined
  if (field.type === 'integer') {
    const n = Number(raw)
    if (!Number.isFinite(n) || !Number.isInteger(n)) {
      throw new Error(`${field.name}: integer expected`)
    }
    return n
  }
  if (field.type === 'number') {
    const n = Number(raw)
    if (!Number.isFinite(n)) throw new Error(`${field.name}: number expected`)
    return n
  }
  return String(raw)
}

export function valueTypeFor(field: SchemaField): string {
  if (field.type === 'boolean') return 'boolean'
  if (field.type === 'integer' || field.type === 'number') return 'integer' // backend lumps both
  return 'string'
}

export function validateField(field: SchemaField, raw: string | boolean): string | null {
  if (!field.optional && (raw === '' || raw === undefined || raw === null)) {
    return `${field.name} is required`
  }
  if (field.type === 'string' && typeof raw === 'string' && raw && field.pattern) {
    try {
      const re = new RegExp(field.pattern)
      if (!re.test(raw)) return `${field.name} doesn't match pattern`
    } catch {
      // ignore invalid pattern
    }
  }
  return null
}
