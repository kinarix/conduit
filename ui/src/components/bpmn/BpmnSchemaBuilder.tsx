import { useState, useEffect, useRef } from 'react';

interface SchemaField {
  name: string;
  type: 'string' | 'number' | 'integer' | 'boolean';
  optional: boolean;
  nullable: boolean;
  pattern?: string;
  description?: string; // preserved from parsed schema, not shown in UI
}

interface Props {
  value?: string;
  onChange: (schema: string | undefined) => void;
  accentColor?: string;
}

const TYPES = ['string', 'number', 'integer', 'boolean'] as const;

function parseFields(schemaStr?: string): { fields: SchemaField[]; additionalProperties: boolean } {
  if (!schemaStr) return { fields: [], additionalProperties: false };
  try {
    const schema = JSON.parse(schemaStr);
    const props = (schema.properties ?? {}) as Record<string, {
      type?: string | string[];
      description?: string;
      pattern?: string;
    }>;
    const required: string[] = schema.required ?? [];
    const fields = Object.entries(props).map(([name, def]) => {
      const rawType = def.type;
      const typeArr = Array.isArray(rawType) ? rawType : [rawType ?? 'string'];
      const nullable = typeArr.includes('null');
      const baseType = typeArr.find(t => t !== 'null') ?? 'string';
      return {
        name,
        type: (TYPES.includes(baseType as typeof TYPES[number]) ? baseType : 'string') as SchemaField['type'],
        optional: !required.includes(name),
        nullable,
        pattern: def.pattern,
        description: def.description,
      };
    });
    return { fields, additionalProperties: schema.additionalProperties === true };
  } catch {
    return { fields: [], additionalProperties: false };
  }
}

function buildSchemaStr(fields: SchemaField[], additionalProperties: boolean): string | undefined {
  const valid = fields.filter(f => f.name.trim());
  if (valid.length === 0 && !additionalProperties) return undefined;
  const properties: Record<string, unknown> = {};
  const required: string[] = [];
  for (const f of valid) {
    const baseType = f.nullable ? [f.type, 'null'] : f.type;
    const def: Record<string, unknown> = { type: baseType };
    if (f.description) def.description = f.description;
    if (f.type === 'string' && f.pattern) def.pattern = f.pattern;
    properties[f.name] = def;
    if (!f.optional) required.push(f.name);
  }
  const schema: Record<string, unknown> = { type: 'object', properties };
  if (required.length) schema.required = required;
  if (additionalProperties) schema.additionalProperties = true;
  return JSON.stringify(schema, null, 2);
}

const inputStyle: React.CSSProperties = {
  padding: '3px 6px',
  fontSize: 11,
  border: '1px solid #e2e8f0',
  borderRadius: 3,
  background: '#fff',
  color: '#0f172a',
  outline: 'none',
  boxSizing: 'border-box',
  minWidth: 0,
};


const fieldLabel: React.CSSProperties = {
  minWidth: 52,
  fontSize: 11,
  color: '#94a3b8',
  fontWeight: 500,
  flexShrink: 0,
  textAlign: 'left',
};

const fieldRow: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 6,
  padding: '4px 0',
  borderBottom: '1px solid #e2e8f0',
};

const iconBtn: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 22,
  height: 22,
  padding: 0,
  background: 'none',
  border: '1px solid transparent',
  borderRadius: 4,
  cursor: 'pointer',
  color: '#94a3b8',
  fontSize: 11,
  fontWeight: 600,
  flexShrink: 0,
};

export default function BpmnSchemaBuilder({ value, onChange, accentColor = '#6366f1' }: Props) {
  const [open, setOpen] = useState(false);
  const [showJson, setShowJson] = useState(false);
  const [copied, setCopied] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const init = parseFields(value);
  const [fields, setFields] = useState<SchemaField[]>(() => init.fields);
  const [additionalProperties, setAdditionalProperties] = useState(() => init.additionalProperties);

  useEffect(() => {
    const p = parseFields(value);
    setFields(p.fields);
    setAdditionalProperties(p.additionalProperties);
  }, [value]);

  function commit(nextFields: SchemaField[], nextAdditional = additionalProperties) {
    setFields(nextFields);
    setAdditionalProperties(nextAdditional);
    onChange(buildSchemaStr(nextFields, nextAdditional));
  }

  function addField() {
    commit([...fields, { name: '', type: 'string', optional: false, nullable: false }]);
  }

  function removeField(i: number) {
    commit(fields.filter((_, idx) => idx !== i));
  }

  function patchField(i: number, patch: Partial<SchemaField>) {
    const next = fields.map((f, idx) => idx === i ? { ...f, ...patch } : f);
    commit(next);
  }

  function handleUpload(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = ev => {
      const text = ev.target?.result as string;
      try {
        JSON.parse(text);
        const p = parseFields(text);
        setFields(p.fields);
        setAdditionalProperties(p.additionalProperties);
        onChange(text);
      } catch { /* invalid JSON — ignore */ }
    };
    reader.readAsText(file);
    e.target.value = '';
  }

  function handleCopy() {
    if (!value) return;
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  const count = fields.filter(f => f.name.trim()).length;

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid #e2e8f0', paddingTop: 10 }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <button
          onClick={() => setOpen(o => !o)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 5,
            fontSize: 11,
            fontWeight: 600,
            color: accentColor,
            textTransform: 'uppercase',
            letterSpacing: '0.05em',
            cursor: 'pointer',
            background: 'none',
            border: 'none',
            padding: 0,
            flex: 1,
            textAlign: 'left',
          }}
        >
          <span style={{ fontSize: 9 }}>{open ? '▼' : '▶'}</span>
          Input Schema
        </button>
        {count > 0 && (
          <span style={{
            background: accentColor,
            color: '#fff',
            borderRadius: 10,
            padding: '1px 6px',
            fontSize: 10,
            fontWeight: 700,
          }}>
            {count}
          </span>
        )}
        {/* Upload */}
        <button
          style={iconBtn}
          title="Upload JSON Schema"
          onClick={() => fileRef.current?.click()}
          onMouseEnter={e => { e.currentTarget.style.borderColor = '#e2e8f0'; e.currentTarget.style.color = '#475569'; }}
          onMouseLeave={e => { e.currentTarget.style.borderColor = 'transparent'; e.currentTarget.style.color = '#94a3b8'; }}
        >
          <svg width={12} height={12} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round">
            <path d="M8 11V3M4 6l4-4 4 4"/><path d="M2 13h12"/>
          </svg>
        </button>
        {/* View JSON */}
        <button
          style={{ ...iconBtn, color: showJson ? accentColor : '#94a3b8', borderColor: showJson ? accentColor + '44' : 'transparent' }}
          title="View JSON Schema"
          onClick={() => setShowJson(v => !v)}
          onMouseEnter={e => { if (!showJson) { e.currentTarget.style.borderColor = '#e2e8f0'; e.currentTarget.style.color = '#475569'; } }}
          onMouseLeave={e => { if (!showJson) { e.currentTarget.style.borderColor = 'transparent'; e.currentTarget.style.color = '#94a3b8'; } }}
        >
          <svg width={12} height={12} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
            <path d="M5 4L1 8l4 4M11 4l4 4-4 4"/>
          </svg>
        </button>
        <input ref={fileRef} type="file" accept=".json,application/json" onChange={handleUpload} style={{ display: 'none' }} />
      </div>

      {/* JSON view */}
      {showJson && (
        <div style={{ position: 'relative', marginTop: 8 }}>
          <pre style={{
            margin: 0,
            fontSize: 10,
            lineHeight: 1.5,
            background: '#0f172a',
            color: '#94a3b8',
            padding: '8px 10px',
            borderRadius: 4,
            overflow: 'auto',
            maxHeight: 180,
            fontFamily: 'ui-monospace, monospace',
          }}>
            {value ? value : '(no schema)'}
          </pre>
          {value && (
            <button
              onClick={handleCopy}
              style={{
                position: 'absolute',
                top: 5,
                right: 5,
                fontSize: 10,
                padding: '2px 6px',
                background: copied ? '#16a34a' : '#1e293b',
                color: copied ? '#fff' : '#94a3b8',
                border: '1px solid #334155',
                borderRadius: 3,
                cursor: 'pointer',
              }}
            >
              {copied ? 'Copied!' : 'Copy'}
            </button>
          )}
        </div>
      )}

      {/* Fields */}
      {open && (
        <div style={{ marginTop: 8 }}>
          {/* Additional Variables — pinned top */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 0', borderBottom: '1px solid #e2e8f0', marginBottom: 8 }}>
            <span style={fieldLabel}>Additional Variables</span>
            <input
              type="checkbox"
              checked={additionalProperties}
              onChange={e => commit(fields, e.target.checked)}
              style={{ cursor: 'pointer', margin: 0 }}
            />
          </div>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(210px, 1fr))',
            gap: 4,
            marginBottom: 6,
          }}>
            {fields.map((f, i) => (
              <div key={i} style={{
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                borderRadius: 4,
                padding: '0 8px',
              }}>
                {/* Name */}
                <div style={fieldRow}>
                  <span style={fieldLabel}>Name</span>
                  <input
                    style={{ ...inputStyle, flex: 1 }}
                    placeholder="field name"
                    value={f.name}
                    onChange={e => patchField(i, { name: e.target.value })}
                  />
                  <button
                    onClick={() => removeField(i)}
                    title="Remove field"
                    style={{
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      color: '#cbd5e1',
                      fontSize: 15,
                      padding: '0 1px',
                      lineHeight: 1,
                      flexShrink: 0,
                    }}
                    onMouseEnter={e => (e.currentTarget.style.color = '#ef4444')}
                    onMouseLeave={e => (e.currentTarget.style.color = '#cbd5e1')}
                  >
                    ×
                  </button>
                </div>
                {/* Type */}
                <div style={fieldRow}>
                  <span style={fieldLabel}>Type</span>
                  <select
                    style={{ ...inputStyle, flex: 1, cursor: 'pointer' }}
                    value={f.type}
                    onChange={e => patchField(i, { type: e.target.value as SchemaField['type'], pattern: undefined })}
                  >
                    {TYPES.map(t => <option key={t} value={t}>{t}</option>)}
                  </select>
                </div>
                {/* Optional */}
                <div style={fieldRow}>
                  <span style={fieldLabel}>Optional</span>
                  <input
                    type="checkbox"
                    checked={f.optional}
                    onChange={e => patchField(i, { optional: e.target.checked })}
                    style={{ cursor: 'pointer', margin: 0 }}
                  />
                </div>
                {/* Nullable */}
                <div style={f.type === 'string' ? fieldRow : { ...fieldRow, borderBottom: 'none' }}>
                  <span style={fieldLabel}>Nullable</span>
                  <input
                    type="checkbox"
                    checked={f.nullable}
                    onChange={e => patchField(i, { nullable: e.target.checked })}
                    style={{ cursor: 'pointer', margin: 0 }}
                  />
                </div>
                {/* Pattern — string only */}
                {f.type === 'string' && (
                  <div style={{ ...fieldRow, borderBottom: 'none' }}>
                    <span style={fieldLabel}>Pattern</span>
                    <input
                      style={{ ...inputStyle, flex: 1 }}
                      placeholder="e.g. ^[A-Z]+$"
                      value={f.pattern ?? ''}
                      onChange={e => patchField(i, { pattern: e.target.value || undefined })}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* Add variable button */}
          <button
            onClick={addField}
            style={{
              width: '100%',
              padding: '4px 0',
              fontSize: 11,
              fontWeight: 500,
              color: accentColor,
              background: 'none',
              border: `1px dashed ${accentColor}`,
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            + Add variable
          </button>
        </div>
      )}
    </div>
  );
}
