import type { Node, Edge } from '@xyflow/react';
import type { BpmnNodeData, BpmnEdgeData } from './bpmnTypes';
import { ELEMENT_COLORS } from './bpmnTypes';
import BpmnSchemaBuilder from './BpmnSchemaBuilder';

interface Props {
  selected: Node | Edge | null;
  onNodeChange: (id: string, data: Partial<BpmnNodeData>) => void;
  onEdgeChange: (id: string, data: Partial<BpmnEdgeData>) => void;
  processSchema?: string;
  onProcessSchemaChange?: (schema: string | undefined) => void;
}

function isNode(el: Node | Edge): el is Node {
  return 'position' in el;
}

const panelStyle: React.CSSProperties = {
  width: '100%',
  height: '100%',
  background: '#f8fafc',
  padding: '14px 12px',
  overflowY: 'auto',
  boxSizing: 'border-box',
};

const headingStyle: React.CSSProperties = {
  fontSize: 10,
  fontWeight: 700,
  color: '#94a3b8',
  textTransform: 'uppercase',
  letterSpacing: '0.08em',
  marginBottom: 10,
};

// Single-line row: [label] [input]
const rowStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  marginBottom: 6,
};

const labelStyle: React.CSSProperties = {
  minWidth: 50,
  fontSize: 11,
  color: '#94a3b8',
  fontWeight: 500,
  flexShrink: 0,
  textAlign: 'right',
};

const inputStyle: React.CSSProperties = {
  flex: 1,
  padding: '4px 7px',
  fontSize: 12,
  border: '1px solid #e2e8f0',
  borderRadius: 4,
  background: '#ffffff',
  color: '#0f172a',
  outline: 'none',
  boxSizing: 'border-box',
  minWidth: 0,
};

const readonlyStyle: React.CSSProperties = {
  ...inputStyle,
  background: '#f1f5f9',
  color: '#94a3b8',
  fontSize: 11,
  fontFamily: 'ui-monospace, monospace',
};

const selectStyle: React.CSSProperties = {
  ...inputStyle,
  cursor: 'pointer',
  appearance: 'none',
  backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 12 12'%3E%3Cpath fill='%2394a3b8' d='M6 8L1 3h10z'/%3E%3C/svg%3E")`,
  backgroundRepeat: 'no-repeat',
  backgroundPosition: 'right 7px center',
  paddingRight: 22,
};

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div style={rowStyle}>
      <span style={labelStyle}>{label}</span>
      {children}
    </div>
  );
}

export default function BpmnProperties({
  selected,
  onNodeChange,
  onEdgeChange,
  processSchema,
  onProcessSchemaChange,
}: Props) {
  if (!selected) {
    return (
      <div style={panelStyle}>
        <div style={headingStyle}>Process</div>
        <div style={{ fontSize: 11, color: '#cbd5e1', marginBottom: 10 }}>
          Select an element to edit its properties.
        </div>
        {onProcessSchemaChange && (
          <BpmnSchemaBuilder
            value={processSchema}
            onChange={onProcessSchemaChange}
            accentColor="#16a34a"
          />
        )}
      </div>
    );
  }

  if (isNode(selected)) {
    const d = selected.data as BpmnNodeData;
    const isTask = d.bpmnType === 'userTask' || d.bpmnType === 'serviceTask';
    const accentColor = ELEMENT_COLORS[d.bpmnType]?.stroke ?? '#6366f1';

    return (
      <div style={panelStyle}>
        <div style={{ ...headingStyle, color: accentColor }}>Properties</div>

        <Field label="ID">
          <input style={readonlyStyle} value={selected.id} readOnly />
        </Field>

        {isTask && (
          <Field label="Type">
            <select
              style={selectStyle}
              value={d.bpmnType}
              onChange={e => onNodeChange(selected.id, { bpmnType: e.target.value as BpmnNodeData['bpmnType'] })}
              onFocus={e => (e.target.style.borderColor = accentColor)}
              onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
            >
              <option value="userTask">User Task</option>
              <option value="serviceTask">Service Task</option>
            </select>
          </Field>
        )}

        <Field label="Name">
          <input
            style={inputStyle}
            value={d.label ?? ''}
            onChange={e => onNodeChange(selected.id, { label: e.target.value })}
            onFocus={e => (e.target.style.borderColor = accentColor)}
            onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
          />
        </Field>

        {d.bpmnType === 'serviceTask' && (
          <>
            <Field label="Topic">
              <input
                style={inputStyle}
                value={d.topic ?? ''}
                placeholder="e.g. email-sender"
                onChange={e => onNodeChange(selected.id, { topic: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
            <Field label="URL">
              <input
                style={inputStyle}
                value={d.url ?? ''}
                placeholder="https://..."
                onChange={e => onNodeChange(selected.id, { url: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
          </>
        )}

        {(d.bpmnType === 'startEvent' || d.bpmnType === 'userTask' || d.bpmnType === 'serviceTask') && (
          <BpmnSchemaBuilder
            value={d.bpmnType === 'startEvent' ? processSchema : d.schema}
            onChange={schema =>
              d.bpmnType === 'startEvent'
                ? onProcessSchemaChange?.(schema)
                : onNodeChange(selected.id, { schema })
            }
            accentColor={accentColor}
          />
        )}
      </div>
    );
  }

  // Edge
  const d = (selected.data ?? {}) as BpmnEdgeData;
  return (
    <div style={panelStyle}>
      <div style={headingStyle}>Flow</div>

      <Field label="ID">
        <input style={readonlyStyle} value={selected.id} readOnly />
      </Field>

      <Field label="Cond.">
        <input
          style={inputStyle}
          value={d.condition ?? ''}
          placeholder="e.g. approved == true"
          onChange={e => onEdgeChange(selected.id, { condition: e.target.value || undefined })}
          onFocus={e => (e.target.style.borderColor = '#6366f1')}
          onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
        />
      </Field>
    </div>
  );
}
