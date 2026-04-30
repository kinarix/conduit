import { useState } from 'react';
import type { Node, Edge } from '@xyflow/react';
import type { BpmnNodeData, BpmnEdgeData } from './bpmnTypes';
import { ELEMENT_COLORS } from './bpmnTypes';
import BpmnSchemaBuilder from './BpmnSchemaBuilder';
import { computeNodeWarnings } from './bpmnValidation';

interface Props {
  selected: Node | Edge | null;
  nodes: Node[];
  edges: Edge[];
  onNodeChange: (id: string, data: Partial<BpmnNodeData>) => void;
  onEdgeChange: (id: string, data: Partial<BpmnEdgeData>) => void;
  processKey?: string;
  processName?: string;
  onProcessNameChange?: (name: string) => void;
  processSchema?: string;
  onProcessSchemaChange?: (schema: string | undefined) => void;
}


function ValidationWarnings({ warnings }: { warnings: string[] }) {
  if (warnings.length === 0) return null;
  return (
    <div style={{ marginBottom: 10 }}>
      {warnings.map(w => (
        <div key={w} style={{
          display: 'flex', alignItems: 'flex-start', gap: 6,
          padding: '5px 8px', marginBottom: 4,
          background: '#fffbeb', border: '1px solid #fcd34d',
          borderRadius: 4, fontSize: 11, color: '#92400e', lineHeight: 1.4,
        }}>
          <span style={{ flexShrink: 0, marginTop: 1 }}>⚠</span>
          <span>{w}</span>
        </div>
      ))}
    </div>
  );
}

function isNode(el: Node | Edge): el is Node {
  return 'position' in el;
}

const panelStyle: React.CSSProperties = {
  width: '100%',
  height: '100%',
  background: '#f8fafc',
  display: 'flex',
  flexDirection: 'column',
  boxSizing: 'border-box',
  overflow: 'hidden',
};

const scrollableStyle: React.CSSProperties = {
  flex: 1,
  overflowY: 'auto',
  padding: '14px 12px',
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

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={rowStyle}>
      <span style={labelStyle}>{label}</span>
      {children}
    </div>
  );
}

function CheckboxField({ label, checked, onChange, accent }: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  accent: string;
}) {
  return (
    <div style={{ ...rowStyle, cursor: 'pointer' }} onClick={() => onChange(!checked)}>
      <span style={labelStyle}>{label}</span>
      <div style={{
        width: 16, height: 16, borderRadius: 3, flexShrink: 0,
        border: `1.5px solid ${checked ? accent : '#cbd5e1'}`,
        background: checked ? accent : '#ffffff',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        transition: 'all 0.1s',
      }}>
        {checked && (
          <svg width={10} height={10} viewBox="0 0 10 10" fill="none">
            <path d="M1.5 5l2.5 2.5 4.5-4.5" stroke="#fff" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        )}
      </div>
      <span style={{ fontSize: 12, color: '#475569', marginLeft: -2 }}>
        Interrupting
      </span>
    </div>
  );
}

// ── Shared boundary event wiring diagram ─────────────────────────────────────

function BoundaryWiringDiagram() {
  return (
    <div style={{ margin: '2px 0 8px', borderRadius: 5, background: '#f1f5f9', padding: '7px 8px' }}>
      <div style={{ display: 'flex', gap: 12, marginBottom: 6, fontSize: 10, color: '#64748b', flexWrap: 'wrap' }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <span style={{ width: 8, height: 8, borderRadius: 2, background: '#f59e0b', display: 'inline-block', flexShrink: 0 }} />
          bottom port → attach to host task
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#6366f1', display: 'inline-block', flexShrink: 0 }} />
          right port → path taken when fired
        </span>
      </div>
      <svg width={200} height={88} viewBox="0 0 200 88" style={{ display: 'block', overflow: 'visible' }}>
        {/* Service Task */}
        <rect x={4} y={6} width={62} height={24} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
        <text x={35} y={22} textAnchor="middle" fontSize={9} fill="#3730a3" fontWeight={500}>Service Task</text>
        {/* Normal path */}
        <text x={84} y={14} textAnchor="middle" fontSize={7} fill="#94a3b8">normal</text>
        <line x1={66} y1={18} x2={110} y2={18} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="110,15 110,21 116,18" fill="#64748b"/>
        {/* End event */}
        <circle cx={125} cy={18} r={7} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>
        {/* Amber attachment port */}
        <rect x={32} y={30} width={7} height={7} rx={1.5} fill="#f59e0b"/>
        {/* Dashed attachment line down */}
        <line x1={35} y1={37} x2={35} y2={59} stroke="#94a3b8" strokeWidth={1.2} strokeDasharray="3 2"/>
        {/* Boundary event (timer style) */}
        <circle cx={35} cy={71} r={11} fill="#fff7ed" stroke="#ea580c" strokeWidth={1.4} strokeDasharray="3 2"/>
        <circle cx={35} cy={71} r={6} fill="none" stroke="#ea580c" strokeWidth={0.9}/>
        <path d="M35 68v3l2 1.2" stroke="#ea580c" strokeWidth={0.9} strokeLinecap="round"/>
        {/* Indigo sequence flow port */}
        <circle cx={46} cy={71} r={3.5} fill="#6366f1"/>
        {/* On-fire path */}
        <text x={71} y={67} textAnchor="middle" fontSize={7} fill="#94a3b8">on fire</text>
        <line x1={50} y1={71} x2={110} y2={71} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="110,68 110,74 116,71" fill="#64748b"/>
        {/* Escalation box */}
        <rect x={116} y={64} width={52} height={14} rx={2} fill="#f0fdf4" stroke="#16a34a" strokeWidth={1.2}/>
        <text x={142} y={74} textAnchor="middle" fontSize={8} fill="#15803d">Escalation</text>
      </svg>
    </div>
  );
}

// ── Documentation content per element type ────────────────────────────────────

const DOCS: Partial<Record<string, React.ReactNode>> = {
  timerStartEvent: (
    <>
      <p>Starts a new process instance when a timer fires. The engine reschedules cycles automatically.</p>
      <table>
        <thead><tr><th>Type</th><th>Expression</th><th>Meaning</th></tr></thead>
        <tbody>
          <tr><td>Cycle</td><td><code>R/PT1H</code></td><td>Repeat every hour</td></tr>
          <tr><td>Cycle</td><td><code>R/PT30M</code></td><td>Repeat every 30 min</td></tr>
          <tr><td>Duration</td><td><code>PT5M</code></td><td>Fire once after 5 min</td></tr>
          <tr><td>Date</td><td><code>2026-06-01T09:00:00Z</code></td><td>Fire once at exact time</td></tr>
        </tbody>
      </table>
      <p>Cycle format: <code>R[n]/&lt;duration&gt;</code> — omit <code>n</code> to repeat forever.<br/>Duration format: <code>PT&lt;H&gt;H&lt;M&gt;M&lt;S&gt;S</code> per ISO 8601.</p>
    </>
  ),
  messageStartEvent: (
    <>
      <p>Starts a new process instance when a message with a matching name is received via the API.</p>
      <p><strong>Message Name</strong> must match the <code>message_name</code> field in the POST body:</p>
      <pre><code>{`POST /api/v1/messages
{
  "org_id": "...",
  "message_name": "OrderReceived",
  "correlation_key": null,
  "variables": [...]
}`}</code></pre>
    </>
  ),
  serviceTask: (
    <>
      <p>Executes work outside the engine. Two modes:</p>
      <p><strong>External worker</strong> — set a <em>Topic</em>. Workers poll <code>POST /api/v1/external-tasks/fetch-and-lock</code>, do the work, then call <code>POST /api/v1/external-tasks/:id/complete</code>.</p>
      <p><strong>HTTP push</strong> — set a <em>URL</em>. The engine POSTs to the URL with the instance variables and advances when it gets a 2xx response.</p>
    </>
  ),
  sendTask: (
    <>
      <p>Throws a named message and continues immediately (fire-and-continue). If a <code>ReceiveTask</code> or <code>IntermediateCatchEvent (message)</code> is waiting with a matching name and correlation key, it is delivered; otherwise the message is dropped.</p>
      <p><strong>Message Name</strong> must match the target element's message name.</p>
      <p><strong>Correlation Key</strong> is optional — set it to route to a specific instance (e.g. <code>orderId</code>).</p>
    </>
  ),
  receiveTask: (
    <>
      <p>Pauses the token and waits for an inbound message with a matching name. A <code>SendTask</code> or an external <code>POST /api/v1/messages</code> call can deliver it.</p>
      <p><strong>Correlation Key</strong> identifies which instance should receive the message (e.g. set <code>orderId</code> and pass the same value when sending).</p>
    </>
  ),
  userTask: (
    <>
      <p>Creates a task waiting for a human to complete it.</p>
      <p>Poll for tasks: <code>GET /api/v1/tasks?org_id=...</code><br/>Complete: <code>POST /api/v1/tasks/:id/complete</code> with a <code>variables</code> array.</p>
    </>
  ),
  businessRuleTask: (
    <>
      <p>Evaluates a DMN decision table and writes the outputs back as process variables.</p>
      <p>Deploy your decision first: <code>POST /api/v1/decisions</code> with a DMN XML body. The <strong>Decision Ref</strong> must match the decision's <code>id</code> attribute in the DMN.</p>
    </>
  ),
  boundaryErrorEvent: (
    <>
      <BoundaryWiringDiagram />
      <p>Catches a BPMN business error thrown by a service task worker via <code>POST /api/v1/external-tasks/:id/bpmn-error</code>.</p>
      <p>Set <strong>Error Code</strong> to match a specific error code. Leave blank to catch <em>any</em> error (catch-all).</p>
      <p>When <strong>Interrupting</strong> is checked, the host task is cancelled when the error fires. Uncheck for non-interrupting (host continues alongside the error path).</p>
    </>
  ),
  boundaryTimerEvent: (
    <>
      <BoundaryWiringDiagram />
      <p>Fires after the host task has been running for the given duration, then routes to the boundary's outgoing flow.</p>
      <p>Use ISO 8601 duration format: <code>PT30M</code> (30 minutes), <code>PT2H</code> (2 hours), <code>P1D</code> (1 day).</p>
    </>
  ),
  boundarySignalEvent: (
    <>
      <BoundaryWiringDiagram />
      <p>Fires when a signal with the matching name is broadcast via <code>POST /api/v1/signals/broadcast</code>.</p>
    </>
  ),
  intermediateCatchTimerEvent: (
    <>
      <p>Pauses the token at this point until the timer fires, then continues along the outgoing flow.</p>
    </>
  ),
  intermediateCatchMessageEvent: (
    <>
      <p>Pauses the token until a message with the matching name and correlation key is received via <code>POST /api/v1/messages</code>.</p>
    </>
  ),
  intermediateCatchSignalEvent: (
    <>
      <p>Pauses the token until a signal with the matching name is broadcast via <code>POST /api/v1/signals/broadcast</code>.</p>
    </>
  ),
  exclusiveGateway: (
    <>
      <p>Routes to exactly one outgoing flow. Each flow can have a <strong>Condition</strong> expression. The first flow whose condition evaluates to <code>true</code> is taken; leave one flow without a condition as a default fallback.</p>
      <p>Expressions use <a href="https://rhai.rs" target="_blank" rel="noreferrer">Rhai</a> syntax and can reference process variables:</p>
      <pre><code>{`approved == true
amount > 1000
status == "pending"`}</code></pre>
    </>
  ),
  parallelGateway: (
    <>
      <p>Fork: creates a token on every outgoing flow simultaneously.<br/>Join: waits until <em>all</em> incoming tokens have arrived before continuing.</p>
    </>
  ),
  inclusiveGateway: (
    <>
      <p>Fork: activates every outgoing flow whose condition is <code>true</code> (one or more).<br/>Join: waits for all <em>activated</em> branches to complete before continuing.</p>
    </>
  ),
  sequenceFlow: (
    <>
      <p>A <strong>Condition</strong> is only evaluated when the source is an Exclusive or Inclusive Gateway. Leave blank for unconditional flow.</p>
      <p>Rhai expression — references process variables by name:</p>
      <pre><code>{`approved == true
score >= 80`}</code></pre>
    </>
  ),
};

const DOCS_TITLES: Partial<Record<string, string>> = {
  timerStartEvent:               'Timer Start Event',
  messageStartEvent:             'Message Start Event',
  serviceTask:                   'Service Task',
  userTask:                      'User Task',
  businessRuleTask:              'Business Rule Task',
  sendTask:                      'Send Task',
  receiveTask:                   'Receive Task',
  boundaryErrorEvent:            'Boundary Error Event',
  boundaryTimerEvent:            'Boundary Timer Event',
  boundarySignalEvent:           'Boundary Signal Event',
  intermediateCatchTimerEvent:   'Timer Catch Event',
  intermediateCatchMessageEvent: 'Message Catch Event',
  intermediateCatchSignalEvent:  'Signal Catch Event',
  exclusiveGateway:              'Exclusive Gateway',
  parallelGateway:               'Parallel Gateway',
  inclusiveGateway:              'Inclusive Gateway',
  sequenceFlow:                  'Sequence Flow',
};

function DocsDrawer({ docKey }: { docKey: string }) {
  const [open, setOpen] = useState(false);
  const content = DOCS[docKey];
  if (!content) return null;

  return (
    <div style={{
      borderTop: '1px solid #e2e8f0',
      background: '#f8fafc',
      flexShrink: 0,
    }}>
      <button
        onClick={() => setOpen(o => !o)}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '8px 12px',
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          fontSize: 11,
          fontWeight: 600,
          color: '#64748b',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
        }}
        onMouseEnter={e => (e.currentTarget.style.background = '#f1f5f9')}
        onMouseLeave={e => (e.currentTarget.style.background = 'none')}
      >
        <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <svg width={12} height={12} viewBox="0 0 16 16" fill="none" stroke="#6366f1" strokeWidth={1.8}>
            <circle cx={8} cy={8} r={6.5}/>
            <path d="M6 6.2C6 5 6.9 4 8 4s2 .9 2 2c0 1.5-2 1.8-2 3.5" strokeLinecap="round" strokeLinejoin="round"/>
            <circle cx={8} cy={12} r={0.6} fill="#6366f1" stroke="none"/>
          </svg>
          Help: {DOCS_TITLES[docKey] ?? docKey}
        </span>
        <svg
          width={10} height={10} viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.8}
          style={{ transform: open ? 'rotate(0deg)' : 'rotate(180deg)', transition: 'transform 0.18s' }}
        >
          <path d="M2 4l4 4 4-4" strokeLinecap="round" strokeLinejoin="round"/>
        </svg>
      </button>

      {open && (
        <div style={{
          padding: '0 12px 12px',
          maxHeight: 320,
          overflowY: 'auto',
          fontSize: 12,
          lineHeight: 1.6,
          color: '#475569',
        }}>
          <style>{`
            .bpmn-docs p { margin: 0 0 8px; }
            .bpmn-docs p:last-child { margin-bottom: 0; }
            .bpmn-docs code {
              font-family: ui-monospace, monospace;
              font-size: 11px;
              background: #e2e8f0;
              color: #0f172a;
              padding: 1px 4px;
              border-radius: 3px;
            }
            .bpmn-docs pre {
              background: #1e293b;
              color: #e2e8f0;
              border-radius: 5px;
              padding: 8px 10px;
              margin: 6px 0;
              overflow-x: auto;
              font-size: 11px;
              line-height: 1.5;
            }
            .bpmn-docs pre code {
              background: none;
              color: inherit;
              padding: 0;
            }
            .bpmn-docs table {
              width: 100%;
              border-collapse: collapse;
              margin: 6px 0 8px;
              font-size: 11px;
            }
            .bpmn-docs th {
              text-align: left;
              color: #94a3b8;
              font-weight: 600;
              padding: 3px 6px 3px 0;
              border-bottom: 1px solid #e2e8f0;
            }
            .bpmn-docs td {
              padding: 3px 6px 3px 0;
              vertical-align: top;
              border-bottom: 1px solid #f1f5f9;
            }
            .bpmn-docs a { color: #6366f1; text-decoration: none; }
            .bpmn-docs a:hover { text-decoration: underline; }
            .bpmn-docs strong { color: #0f172a; font-weight: 600; }
          `}</style>
          <div className="bpmn-docs">{content}</div>
        </div>
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function BpmnProperties({
  selected,
  nodes,
  edges,
  onNodeChange,
  onEdgeChange,
  processKey,
  processName,
  onProcessNameChange,
  processSchema,
  onProcessSchemaChange,
}: Props) {
  if (!selected) {
    return (
      <div style={panelStyle}>
        <div style={scrollableStyle}>
          <div style={headingStyle}>Process</div>
          {(processName !== undefined || processKey) && (
            <div style={{ marginBottom: 10 }}>
              {processName !== undefined && (
                <Field label="Name">
                  <input
                    style={onProcessNameChange ? inputStyle : readonlyStyle}
                    value={processName}
                    readOnly={!onProcessNameChange}
                    onChange={onProcessNameChange ? (e) => onProcessNameChange(e.target.value) : undefined}
                    placeholder="Untitled process"
                  />
                </Field>
              )}
              {processKey && (
                <Field label="Key">
                  <input
                    style={{ ...readonlyStyle, fontFamily: 'monospace' }}
                    value={processKey}
                    readOnly
                  />
                </Field>
              )}
            </div>
          )}
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
      </div>
    );
  }

  if (isNode(selected)) {
    const d = selected.data as BpmnNodeData;
    const accentColor = ELEMENT_COLORS[d.bpmnType]?.stroke ?? '#6366f1';
    const warnings = computeNodeWarnings(selected.id, d.bpmnType, nodes, edges);

    return (
      <div style={panelStyle}>
        <div style={scrollableStyle}>
          <div style={{ ...headingStyle, color: accentColor }}>Properties</div>
          <ValidationWarnings warnings={warnings} />

          <Field label="ID">
            <input style={readonlyStyle} value={selected.id} readOnly />
          </Field>

          <Field label="Name">
            <input
              style={inputStyle}
              value={d.label ?? ''}
              onChange={e => onNodeChange(selected.id, { label: e.target.value })}
              onFocus={e => (e.target.style.borderColor = accentColor)}
              onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
            />
          </Field>

          {d.bpmnType === 'messageStartEvent' && (
            <Field label="Message">
              <input
                style={inputStyle}
                value={d.messageName ?? ''}
                placeholder="e.g. OrderReceived"
                onChange={e => onNodeChange(selected.id, { messageName: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
          )}

          {d.bpmnType === 'timerStartEvent' && (
            <>
              <Field label="Timer">
                <select
                  style={selectStyle}
                  value={d.timerType ?? 'cycle'}
                  onChange={e => onNodeChange(selected.id, { timerType: e.target.value as BpmnNodeData['timerType'] })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                >
                  <option value="cycle">Cycle (R/...)</option>
                  <option value="duration">Duration (PT...)</option>
                  <option value="date">Date (ISO 8601)</option>
                </select>
              </Field>
              <Field label="Expr.">
                <input
                  style={inputStyle}
                  value={d.timerExpression ?? ''}
                  placeholder={d.timerType === 'date' ? '2026-01-01T09:00:00Z' : d.timerType === 'duration' ? 'PT1H' : 'R/PT1H'}
                  onChange={e => onNodeChange(selected.id, { timerExpression: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
            </>
          )}

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

          {d.bpmnType === 'businessRuleTask' && (
            <Field label="Decision">
              <input
                style={inputStyle}
                value={d.decisionRef ?? ''}
                placeholder="e.g. credit-score"
                onChange={e => onNodeChange(selected.id, { decisionRef: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
          )}

          {d.bpmnType === 'sendTask' && (
            <>
              <Field label="Message">
                <input
                  style={inputStyle}
                  value={d.messageName ?? ''}
                  placeholder="e.g. OrderShipped"
                  onChange={e => onNodeChange(selected.id, { messageName: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <Field label="Corr. Key">
                <input
                  style={inputStyle}
                  value={d.correlationKey ?? ''}
                  placeholder="e.g. orderId"
                  onChange={e => onNodeChange(selected.id, { correlationKey: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
            </>
          )}

          {d.bpmnType === 'receiveTask' && (
            <>
              <Field label="Message">
                <input
                  style={inputStyle}
                  value={d.messageName ?? ''}
                  placeholder="e.g. OrderShipped"
                  onChange={e => onNodeChange(selected.id, { messageName: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <Field label="Corr. Key">
                <input
                  style={inputStyle}
                  value={d.correlationKey ?? ''}
                  placeholder="e.g. orderId"
                  onChange={e => onNodeChange(selected.id, { correlationKey: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
            </>
          )}

          {d.bpmnType === 'boundaryTimerEvent' && (
            <>
              <Field label="Duration">
                <input
                  style={inputStyle}
                  value={d.timerExpression ?? ''}
                  placeholder="e.g. PT30M"
                  onChange={e => onNodeChange(selected.id, { timerExpression: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <CheckboxField
                label=""
                checked={d.cancelling !== false}
                onChange={v => onNodeChange(selected.id, { cancelling: v })}
                accent={accentColor}
              />
            </>
          )}

          {d.bpmnType === 'boundarySignalEvent' && (
            <>
              <Field label="Signal">
                <input
                  style={inputStyle}
                  value={d.signalName ?? ''}
                  placeholder="e.g. OrderCancelled"
                  onChange={e => onNodeChange(selected.id, { signalName: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <CheckboxField
                label=""
                checked={d.cancelling !== false}
                onChange={v => onNodeChange(selected.id, { cancelling: v })}
                accent={accentColor}
              />
            </>
          )}

          {d.bpmnType === 'boundaryErrorEvent' && (
            <>
              <Field label="Err Code">
                <input
                  style={inputStyle}
                  value={d.errorCode ?? ''}
                  placeholder="blank = catch all"
                  onChange={e => onNodeChange(selected.id, { errorCode: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <CheckboxField
                label=""
                checked={d.cancelling !== false}
                onChange={v => onNodeChange(selected.id, { cancelling: v })}
                accent={accentColor}
              />
            </>
          )}

          {d.bpmnType === 'intermediateCatchTimerEvent' && (
            <>
              <Field label="Timer">
                <select
                  style={selectStyle}
                  value={d.timerType ?? 'duration'}
                  onChange={e => onNodeChange(selected.id, { timerType: e.target.value as BpmnNodeData['timerType'] })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                >
                  <option value="duration">Duration (PT...)</option>
                  <option value="date">Date (ISO 8601)</option>
                  <option value="cycle">Cycle (R/...)</option>
                </select>
              </Field>
              <Field label="Expr.">
                <input
                  style={inputStyle}
                  value={d.timerExpression ?? ''}
                  placeholder={d.timerType === 'date' ? '2026-01-01T09:00:00Z' : d.timerType === 'cycle' ? 'R/PT1H' : 'PT5M'}
                  onChange={e => onNodeChange(selected.id, { timerExpression: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
            </>
          )}

          {d.bpmnType === 'intermediateCatchMessageEvent' && (
            <Field label="Message">
              <input
                style={inputStyle}
                value={d.messageName ?? ''}
                placeholder="e.g. PaymentReceived"
                onChange={e => onNodeChange(selected.id, { messageName: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
          )}

          {d.bpmnType === 'intermediateCatchSignalEvent' && (
            <Field label="Signal">
              <input
                style={inputStyle}
                value={d.signalName ?? ''}
                placeholder="e.g. StockUpdated"
                onChange={e => onNodeChange(selected.id, { signalName: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
            </Field>
          )}

          {(d.bpmnType === 'userTask' || d.bpmnType === 'serviceTask') && (
            <BpmnSchemaBuilder
              value={d.schema}
              onChange={schema => onNodeChange(selected.id, { schema })}
              accentColor={accentColor}
            />
          )}
        </div>

        <DocsDrawer docKey={d.bpmnType} />
      </div>
    );
  }

  // Edge
  const d = (selected.data ?? {}) as BpmnEdgeData;

  if (d.kind === 'attachment') {
    return (
      <div style={panelStyle}>
        <div style={scrollableStyle}>
          <div style={headingStyle}>Attachment</div>
          <Field label="ID">
            <input style={readonlyStyle} value={selected.id} readOnly />
          </Field>
          <div style={{ margin: '8px 0', borderRadius: 5, background: '#f1f5f9', padding: '7px 8px' }}>
            <svg width={120} height={64} viewBox="0 0 120 64" style={{ display: 'block' }}>
              {/* Task */}
              <rect x={4} y={4} width={52} height={22} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
              <text x={30} y={18} textAnchor="middle" fontSize={8} fill="#3730a3" fontWeight={500}>Task</text>
              {/* Amber port */}
              <rect x={27} y={26} width={6} height={6} rx={1.5} fill="#f59e0b"/>
              {/* Dashed attachment line */}
              <line x1={30} y1={32} x2={30} y2={46} stroke="#94a3b8" strokeWidth={1.2} strokeDasharray="3 2"/>
              {/* Boundary event */}
              <circle cx={30} cy={56} r={8} fill="#fff7ed" stroke="#ea580c" strokeWidth={1.4} strokeDasharray="3 2"/>
              <circle cx={30} cy={56} r={4.5} fill="none" stroke="#ea580c" strokeWidth={0.9}/>
              <path d="M30 53.5v2.5l1.5 1" stroke="#ea580c" strokeWidth={0.9} strokeLinecap="round"/>
              {/* Label */}
              <text x={50} y={53} fontSize={8} fill="#94a3b8">ownership,</text>
              <text x={50} y={62} fontSize={8} fill="#94a3b8">not a flow</text>
            </svg>
          </div>
          <div style={{ fontSize: 11, color: '#94a3b8', lineHeight: 1.5 }}>
            This dashed line attaches a boundary event to its host task. It is not a sequence flow and carries no condition.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={panelStyle}>
      <div style={scrollableStyle}>
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

      <DocsDrawer docKey="sequenceFlow" />
    </div>
  );
}
