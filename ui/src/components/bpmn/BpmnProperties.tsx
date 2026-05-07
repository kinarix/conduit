import {
  forwardRef,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type Ref,
} from 'react';
import type { Node, Edge } from '@xyflow/react';
import { useQuery } from '@tanstack/react-query';
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror';
import { json } from '@codemirror/lang-json';
import { autocompletion, type CompletionContext } from '@codemirror/autocomplete';
import { EditorView } from '@codemirror/view';
import type {
  BpmnNodeData,
  BpmnEdgeData,
  HttpAuthType,
  HttpConnectorConfig,
} from './bpmnTypes';
import { ELEMENT_COLORS, ELEMENT_LABELS } from './bpmnTypes';
import BpmnSchemaBuilder from './BpmnSchemaBuilder';
import { computeNodeWarnings } from './bpmnValidation';
import { useOrg } from '../../App';
import { fetchSecrets } from '../../api/secrets';
import { DOCS_BASE_URL, ELEMENT_DOC_SLUGS, helpIconStyle, helpIconHover } from '../../lib/docs';

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
  background: 'var(--bg-primary)',
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
  fontSize: 'var(--text-xs)',
  fontWeight: 'var(--weight-bold)',
  color: 'var(--text-tertiary)',
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
  fontSize: 'var(--text-sm)',
  color: 'var(--text-tertiary)',
  fontWeight: 'var(--weight-medium)',
  flexShrink: 0,
  textAlign: 'right',
};

const inputStyle: React.CSSProperties = {
  flex: 1,
  padding: '4px 7px',
  fontSize: 'var(--text-md)',
  border: '1px solid var(--border-primary)',
  borderRadius: 4,
  background: 'var(--bg-secondary)',
  color: 'var(--text-primary)',
  outline: 'none',
  boxSizing: 'border-box',
  minWidth: 0,
};

const readonlyStyle: React.CSSProperties = {
  ...inputStyle,
  background: 'var(--bg-tertiary)',
  color: 'var(--text-tertiary)',
  fontSize: 'var(--text-sm)',
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
  const [httpModalOpen, setHttpModalOpen] = useState(false);
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
          <div style={{ display: 'flex', alignItems: 'center', marginBottom: 10 }}>
            <div style={{ ...headingStyle, marginBottom: 0, flex: 1, color: accentColor }}>{ELEMENT_LABELS[d.bpmnType] ?? 'Properties'}</div>
            {(() => {
              const slug = ELEMENT_DOC_SLUGS[d.bpmnType];
              return slug ? (
                <a
                  href={`${DOCS_BASE_URL}/docs/elements/${slug}/`}
                  target="_blank"
                  rel="noopener noreferrer"
                  title="Open documentation"
                  style={helpIconStyle}
                  onMouseEnter={e => helpIconHover(e.currentTarget as HTMLAnchorElement, true)}
                  onMouseLeave={e => helpIconHover(e.currentTarget as HTMLAnchorElement, false)}
                >
                  ?
                </a>
              ) : null;
            })()}
          </div>
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
                  placeholder="e.g. email-sender (external worker pattern)"
                  onChange={e => onNodeChange(selected.id, { topic: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
              <HttpConnectorSummary
                url={d.url}
                http={d.http}
                onOpen={() => setHttpModalOpen(true)}
              />
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

          {d.bpmnType === 'scriptTask' && (
            <>
              <div style={{ marginBottom: 4, fontSize: 11, color: '#94a3b8' }}>Script (FEEL)</div>
              <textarea
                style={{ ...inputStyle, width: '100%', minHeight: 72, fontFamily: 'ui-monospace, monospace', resize: 'vertical', boxSizing: 'border-box' }}
                value={d.script ?? ''}
                placeholder={'{ fee: amount * 0.05 }'}
                onChange={e => onNodeChange(selected.id, { script: e.target.value || undefined })}
                onFocus={e => (e.target.style.borderColor = accentColor)}
                onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
              />
              <Field label="Result">
                <input
                  style={inputStyle}
                  value={d.resultVariable ?? ''}
                  placeholder="e.g. total (for scalar output)"
                  onChange={e => onNodeChange(selected.id, { resultVariable: e.target.value || undefined })}
                  onFocus={e => (e.target.style.borderColor = accentColor)}
                  onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
                />
              </Field>
            </>
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

          {(d.bpmnType === 'exclusiveGateway' || d.bpmnType === 'inclusiveGateway' || d.bpmnType === 'parallelGateway') && (() => {
            const isAttachment = (e: typeof edges[number]) => (e.data as BpmnEdgeData | undefined)?.kind === 'attachment';
            const outgoing = edges.filter(e => e.source === selected.id && !isAttachment(e));
            const incoming = edges.filter(e => e.target === selected.id && !isAttachment(e));
            const conditionable = d.bpmnType === 'exclusiveGateway' || d.bpmnType === 'inclusiveGateway';
            const outgoingWithCondition = conditionable
              ? outgoing.filter(e => ((e.data as BpmnEdgeData | undefined)?.condition ?? '').trim() !== '').length
              : 0;
            return (
              <div style={{ marginTop: 8, padding: '8px 10px', background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: 4 }}>
                <div style={{ fontSize: 10, fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: 0.5, marginBottom: 6 }}>
                  Flows
                </div>
                <div style={{ display: 'flex', gap: 12, fontSize: 11, color: '#0f172a' }}>
                  <span><strong>{incoming.length}</strong> in</span>
                  <span><strong>{outgoing.length}</strong> out</span>
                  {conditionable && (
                    <span style={{ color: '#475569' }}>
                      <strong>{outgoingWithCondition}</strong>/<strong>{outgoing.length}</strong> conditioned
                    </span>
                  )}
                </div>
                {conditionable && outgoing.length > 0 && outgoingWithCondition === 0 && (
                  <div style={{ marginTop: 6, fontSize: 10, color: '#b45309' }}>
                    No outgoing flow has a condition — every routing decision will fall through.
                  </div>
                )}
              </div>
            );
          })()}
        </div>


        {httpModalOpen && d.bpmnType === 'serviceTask' && (
          <HttpConnectorModal
            url={d.url}
            http={d.http}
            onUrlChange={url => onNodeChange(selected.id, { url })}
            onChange={cfg => onNodeChange(selected.id, { http: cfg })}
            onClose={() => setHttpModalOpen(false)}
          />
        )}
      </div>
    );
  }

  // Edge
  const d = (selected.data ?? {}) as BpmnEdgeData;
  const sourceNode = nodes.find(n => n.id === (selected as Edge).source);
  const sourceType = (sourceNode?.data as BpmnNodeData | undefined)?.bpmnType;
  const isGatewayEdge = sourceType === 'exclusiveGateway' || sourceType === 'inclusiveGateway';

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
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 10 }}>
          <div style={{ ...headingStyle, marginBottom: 0, flex: 1 }}>Flow</div>
          <a
            href={`${DOCS_BASE_URL}/docs/elements/sequence-flow/`}
            target="_blank"
            rel="noopener noreferrer"
            title="Open documentation"
            style={helpIconStyle}
            onMouseEnter={e => helpIconHover(e.currentTarget as HTMLAnchorElement, true)}
            onMouseLeave={e => helpIconHover(e.currentTarget as HTMLAnchorElement, false)}
          >
            ?
          </a>
        </div>

        <Field label="ID">
          <input style={readonlyStyle} value={selected.id} readOnly />
        </Field>

        {isGatewayEdge && (
          <Field label="Cond.">
            <input
              style={inputStyle}
              value={d.condition ?? ''}
              placeholder='e.g. amount > 1000 and tier = "gold"'
              onChange={e => onEdgeChange(selected.id, { condition: e.target.value || undefined })}
              onFocus={e => (e.target.style.borderColor = '#6366f1')}
              onBlur={e => (e.target.style.borderColor = '#e2e8f0')}
            />
          </Field>
        )}

        {isGatewayEdge && (
          <Field label="Default">
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={d.isDefault ?? false}
                onChange={ev => {
                  if (ev.target.checked) {
                    // Clear isDefault from all other edges with same source
                    edges
                      .filter(e => e.id !== selected.id && e.source === (selected as Edge).source)
                      .forEach(e => onEdgeChange(e.id, { isDefault: undefined }));
                    onEdgeChange(selected.id, { isDefault: true });
                  } else {
                    onEdgeChange(selected.id, { isDefault: undefined });
                  }
                }}
              />
              <span style={{ fontSize: 11, color: '#475569' }}>
                Take this flow when no condition matches
              </span>
            </label>
          </Field>
        )}
      </div>

    </div>
  );
}

// ─── HTTP connector ─────────────────────────────────────────────────────────

const REQUEST_TRANSFORM_HINT =
  '// input doc: { instance_id, execution_id, vars }\n// output: { body?, headers?, query?, path? }\n{\n  body: { amount: .vars.amount }\n}';

const RESPONSE_TRANSFORM_HINT =
  '// input doc: { status, headers, body }\n// output: flat { var_name: value, ... }\n// headers are lowercased: .headers["x-rate-limit"]\n{\n  result_id: .body.id\n}';

function HttpConnectorSummary({
  url,
  http,
  onOpen,
}: {
  url: string | undefined;
  http: HttpConnectorConfig | undefined;
  onOpen: () => void;
}) {
  const configured = !!url || !!http;
  const summary = describeHttpConfig(url, http);
  return (
    <div style={{ marginTop: 4 }}>
      <div
        style={{
          padding: '8px 10px',
          border: '1px solid #e2e8f0',
          borderRadius: 6,
          background: configured ? '#f0f9ff' : '#fafafa',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            marginBottom: 6,
          }}
        >
          <div
            style={{
              fontWeight: 600,
              color: '#475569',
              fontSize: 10,
              letterSpacing: 0.3,
              textTransform: 'uppercase',
            }}
          >
            HTTP connector
          </div>
          <button
            type="button"
            onClick={onOpen}
            style={{
              padding: '4px 10px',
              fontSize: 11,
              border: '1px solid #cbd5e1',
              borderRadius: 4,
              background: '#ffffff',
              color: '#0f172a',
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            {configured ? 'Edit' : 'Configure'}
          </button>
        </div>
        <div
          style={{
            color: configured ? '#0f172a' : '#94a3b8',
            fontFamily: 'ui-monospace, monospace',
            fontSize: 11,
            lineHeight: 1.5,
            wordBreak: 'break-all',
          }}
        >
          {summary}
        </div>
      </div>
    </div>
  );
}

function describeHttpConfig(
  url: string | undefined,
  http: HttpConnectorConfig | undefined,
): React.ReactNode {
  if (!url && !http) return 'Not configured';
  const parts: string[] = [];
  parts.push(http?.method ?? 'POST');
  if (http?.authType && http.authType !== 'none') {
    parts.push(
      http.secretRef ? `${http.authType} via ${http.secretRef}` : http.authType,
    );
  }
  if (http?.requestTransform?.trim()) parts.push('req transform');
  if (http?.responseTransform?.trim()) parts.push('resp transform');
  if (http?.retry?.max && http.retry.max > 0) parts.push(`retry ×${http.retry.max}`);
  return (
    <>
      {url ? (
        <div style={{ color: '#0f172a' }}>{url}</div>
      ) : (
        <div style={{ color: '#dc2626' }}>(URL not set)</div>
      )}
      <div style={{ color: '#475569', marginTop: 2 }}>{parts.join(' · ')}</div>
    </>
  );
}

function HttpConnectorModal({
  url,
  http,
  onUrlChange,
  onChange,
  onClose,
}: {
  url: string | undefined;
  http: HttpConnectorConfig | undefined;
  onUrlChange: (url: string | undefined) => void;
  onChange: (cfg: HttpConnectorConfig | undefined) => void;
  onClose: () => void;
}) {
  const cfg = http ?? {};
  const update = (patch: Partial<HttpConnectorConfig>) => {
    const next = { ...cfg, ...patch };
    // Drop the http config entirely once everything is at defaults so we don't
    // emit an empty <conduit:http/> into the XML.
    const empty =
      (!next.method || next.method === 'POST') &&
      next.timeoutMs === undefined &&
      (!next.authType || next.authType === 'none') &&
      !next.secretRef &&
      !next.apiKeyHeader &&
      !next.requestTransform?.trim() &&
      !next.responseTransform?.trim() &&
      !next.errorCodeExpression?.trim() &&
      (!next.retry || Object.keys(next.retry).length === 0);
    onChange(empty ? undefined : next);
  };

  const updateRetry = (patch: Partial<NonNullable<HttpConnectorConfig['retry']>>) => {
    update({ retry: { ...(cfg.retry ?? {}), ...patch } });
  };

  const [exampleFilter, setExampleFilter] = useState<SidePanelTab>('all');
  const [showExamples, setShowExamples] = useState(false);
  const requestEditorRef = useRef<JqEditorHandle>(null);
  const responseEditorRef = useRef<JqEditorHandle>(null);
  const errorCodeEditorRef = useRef<JqEditorHandle>(null);

  // Modal + side panel share a single height so they sit flush. Tall enough
  // to give the transform editors real working room without spilling on
  // smaller screens.
  const sharedHeight = 'min(900px, 92vh)';

  const openExamples = (kind: TransformKind) => {
    if (showExamples && exampleFilter === kind) {
      setShowExamples(false);
    } else {
      setExampleFilter(kind);
      setShowExamples(true);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      {/* The relative wrapper holds the modal in the normal flow; the side
          panel is absolutely positioned off its right edge so toggling the
          panel never shifts the modal's position. */}
      <div
        style={{ position: 'relative' }}
        onClick={e => e.stopPropagation()}
      >
        <div
          className="modal"
          style={{
            maxWidth: 760,
            width: 'min(760px, 95vw)',
            height: sharedHeight,
            display: 'flex',
            flexDirection: 'column',
            padding: 0,
          }}
        >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'baseline',
            padding: '20px 24px 12px',
            borderBottom: '1px solid var(--color-border)',
            flex: '0 0 auto',
          }}
        >
          <div>
            <h3 style={{ margin: 0 }}>HTTP connector</h3>
            <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: '4px 0 0' }}>
              Edits apply live. Close when done.
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            style={{
              border: 'none',
              background: 'transparent',
              fontSize: 20,
              color: 'var(--text-tertiary)',
              cursor: 'pointer',
              lineHeight: 1,
            }}
            aria-label="Close"
          >
            ×
          </button>
        </div>

        <div style={{ overflow: 'auto', flex: 1, padding: '16px 24px 0' }}>

        <Field label="URL">
          <input
            style={inputStyle}
            value={url ?? ''}
            placeholder="https://api.example.com/v1/things/:id"
            onChange={e => onUrlChange(e.target.value || undefined)}
            autoFocus
          />
        </Field>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: '8px 16px',
            marginTop: 8,
          }}
        >
          <Field label="Method">
            <select
              style={selectStyle}
              value={cfg.method ?? 'POST'}
              onChange={e => update({ method: e.target.value })}
            >
              {['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD'].map(m => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </Field>

          <Field label="Timeout (ms)">
            <input
              style={inputStyle}
              type="number"
              min={1}
              value={cfg.timeoutMs ?? ''}
              placeholder="(client default)"
              onChange={e =>
                update({
                  timeoutMs: e.target.value ? Number(e.target.value) : undefined,
                })
              }
            />
          </Field>

          <Field label="Auth">
            <select
              style={selectStyle}
              value={cfg.authType ?? 'none'}
              onChange={e =>
                update({ authType: e.target.value as HttpAuthType })
              }
            >
              <option value="none">None</option>
              <option value="basic">Basic (user:pass)</option>
              <option value="bearer">Bearer token</option>
              <option value="apiKey">API key (custom header)</option>
            </select>
          </Field>

          {cfg.authType && cfg.authType !== 'none' ? (
            <SecretRefField
              value={cfg.secretRef}
              onChange={ref => update({ secretRef: ref })}
            />
          ) : (
            <div />
          )}

          {cfg.authType === 'apiKey' && (
            <Field label="Header">
              <input
                style={inputStyle}
                value={cfg.apiKeyHeader ?? ''}
                placeholder="X-API-Key"
                onChange={e =>
                  update({ apiKeyHeader: e.target.value || undefined })
                }
              />
            </Field>
          )}
        </div>

        <TransformField
          kind="request"
          label="Request"
          value={cfg.requestTransform}
          placeholder={REQUEST_TRANSFORM_HINT}
          onChange={v => update({ requestTransform: v })}
          height={280}
          examplesOpenForThisKind={showExamples && exampleFilter === 'request'}
          onToggleExamples={() => openExamples('request')}
          editorRef={requestEditorRef}
        />

        <TransformField
          kind="response"
          label="Response"
          value={cfg.responseTransform}
          placeholder={RESPONSE_TRANSFORM_HINT}
          onChange={v => update({ responseTransform: v })}
          height={280}
          examplesOpenForThisKind={showExamples && exampleFilter === 'response'}
          onToggleExamples={() => openExamples('response')}
          editorRef={responseEditorRef}
        />

        <TransformField
          kind="errorCode"
          label="Error code"
          suffix="expr"
          value={cfg.errorCodeExpression}
          placeholder='.body.errorCode // ""'
          onChange={v => update({ errorCodeExpression: v })}
          height={120}
          examplesOpenForThisKind={showExamples && exampleFilter === 'errorCode'}
          onToggleExamples={() => openExamples('errorCode')}
          editorRef={errorCodeEditorRef}
        />

        <div style={{ marginTop: 14 }}>
          <div
            style={{
              fontSize: 12,
              fontWeight: 600,
              color: '#475569',
              marginBottom: 6,
            }}
          >
            Retry policy
          </div>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(4, 1fr)',
              gap: '8px 12px',
            }}
          >
            <Field label="Max">
              <input
                style={inputStyle}
                type="number"
                min={0}
                value={cfg.retry?.max ?? ''}
                placeholder="0"
                onChange={e =>
                  updateRetry({
                    max: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
              />
            </Field>
            <Field label="Backoff (ms)">
              <input
                style={inputStyle}
                type="number"
                min={0}
                value={cfg.retry?.backoffMs ?? ''}
                placeholder="1000"
                onChange={e =>
                  updateRetry({
                    backoffMs: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
              />
            </Field>
            <Field label="×">
              <input
                style={inputStyle}
                type="number"
                step={0.1}
                min={1}
                value={cfg.retry?.multiplier ?? ''}
                placeholder="2"
                onChange={e =>
                  updateRetry({
                    multiplier: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
              />
            </Field>
            <Field label="Retry on">
              <input
                style={inputStyle}
                value={cfg.retry?.retryOn ?? ''}
                placeholder="5xx,timeout,network"
                onChange={e =>
                  updateRetry({ retryOn: e.target.value || undefined })
                }
              />
            </Field>
          </div>
        </div>

        </div>

        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            gap: 8,
            padding: '12px 24px',
            borderTop: '1px solid var(--color-border)',
            flex: '0 0 auto',
          }}
        >
          <button className="btn-primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>

        {showExamples && (
          <div
            style={{
              position: 'absolute',
              top: 0,
              left: 'calc(100% + 12px)',
              height: '100%',
            }}
          >
            <JqExamplesSidePanel
              height={sharedHeight}
              filter={exampleFilter}
              onFilterChange={setExampleFilter}
              onUseRequest={snippet =>
                requestEditorRef.current?.appendAndFocus(snippet)
              }
              onUseResponse={snippet =>
                responseEditorRef.current?.appendAndFocus(snippet)
              }
              onUseErrorCode={snippet =>
                errorCodeEditorRef.current?.appendAndFocus(snippet)
              }
              onClose={() => setShowExamples(false)}
            />
          </div>
        )}
      </div>
    </div>
  );
}

function SecretRefField({
  value,
  onChange,
}: {
  value: string | undefined;
  onChange: (ref: string | undefined) => void;
}) {
  const { org } = useOrg();
  const { data: secrets = [] } = useQuery({
    queryKey: ['secrets', org?.id],
    queryFn: () => fetchSecrets(org!.id),
    enabled: !!org,
  });

  return (
    <Field label="Secret">
      <select
        style={selectStyle}
        value={value ?? ''}
        onChange={e => onChange(e.target.value || undefined)}
      >
        <option value="">(select secret)</option>
        {secrets.map(s => (
          <option key={s.id} value={s.name}>
            {s.name}
          </option>
        ))}
      </select>
    </Field>
  );
}

type TransformKind = 'request' | 'response' | 'errorCode';

interface JqExample {
  kind: TransformKind;
  name: string;
  description: string;
  snippet: string;
}

const REQUEST_EXAMPLES: JqExample[] = [
  {
    kind: 'request',
    name: 'POST body from vars',
    description: 'Build a JSON body from instance variables',
    snippet: `{
  body: {
    customer: .vars.customer_name,
    amount: .vars.amount,
    currency: "usd"
  }
}`,
  },
  {
    kind: 'request',
    name: 'Query params',
    description: 'Append ?key=value to the URL (typical for GET / DELETE)',
    snippet: `{
  query: {
    id: .vars.customer_id,
    expand: "card"
  }
}`,
  },
  {
    kind: 'request',
    name: 'URL :placeholders',
    description: 'Substitute :name segments in the URL with var values',
    snippet: `{
  path: { id: .vars.charge_id }
}`,
  },
  {
    kind: 'request',
    name: 'Custom headers',
    description: 'Set arbitrary request headers (auth headers always win on conflict)',
    snippet: `{
  headers: {
    "X-Idempotency-Key": .instance_id,
    "X-Source": "conduit"
  }
}`,
  },
  {
    kind: 'request',
    name: 'Everything together',
    description: 'Combine body + query + path + headers in one filter',
    snippet: `{
  body:    { amount: .vars.amount, currency: "usd" },
  query:   { idempotency_key: .instance_id },
  path:    { customer_id: .vars.customer_id },
  headers: { "X-Source": "conduit" }
}`,
  },
  {
    kind: 'request',
    name: 'Nested body',
    description: 'Mix static values with dynamic ones at any depth',
    snippet: `{
  body: {
    kind: "charge",
    metadata: {
      instance: .instance_id,
      tier:     .vars.tier
    },
    amount: .vars.amount
  }
}`,
  },
  {
    kind: 'request',
    name: 'Array from a list var',
    description: 'Map each element of an array variable into a request item',
    snippet: `{
  body: {
    items: [
      .vars.line_items[] | { id: .id, qty: .quantity }
    ]
  }
}`,
  },
  {
    kind: 'request',
    name: 'Conditional field',
    description: 'Only include a field when a condition is met',
    snippet: `{
  body: {
    amount: .vars.amount,
    notify: (if .vars.email_opt_in then true else null end)
  }
}`,
  },
  {
    kind: 'request',
    name: 'String interpolation',
    description: 'Use \\( … ) to compose strings from variables',
    snippet: `{
  body: {
    description: "Charge for instance \\(.instance_id)"
  }
}`,
  },
  {
    kind: 'request',
    name: 'Pluck only id list',
    description: 'Reduce a list of objects to an array of one field',
    snippet: `{
  body: {
    ids: [.vars.items[].id]
  }
}`,
  },
  {
    kind: 'request',
    name: 'Sum a numeric list',
    description: 'Compute the total of a numeric array variable',
    snippet: `{
  body: {
    total: ([.vars.amounts[]] | add)
  }
}`,
  },
  {
    kind: 'request',
    name: 'Defensive defaults',
    description: 'Use // to substitute a fallback when a var is missing',
    snippet: `{
  body: {
    customer: (.vars.customer_id // "anonymous"),
    amount:   (.vars.amount      // 0)
  }
}`,
  },
];

const RESPONSE_EXAMPLES: JqExample[] = [
  {
    kind: 'response',
    name: 'Extract one field',
    description: 'Pull a single value from the response body into a variable',
    snippet: `{
  charge_id: .body.id
}`,
  },
  {
    kind: 'response',
    name: 'Multiple fields',
    description: 'Set several variables at once including the HTTP status code',
    snippet: `{
  charge_id:     .body.id,
  charge_status: .body.status,
  http_status:   .status
}`,
  },
  {
    kind: 'response',
    name: 'Header value',
    description: 'Read a response header — keys are always lowercased',
    snippet: `{
  rate_limit: (.headers["x-rate-limit-remaining"] | tonumber? // null)
}`,
  },
  {
    kind: 'response',
    name: 'Defensive default',
    description: 'Fall back to null when a field is missing — variable is left unset',
    snippet: `{
  result: (.body.result // null),
  count:  (.body.items | length? // 0)
}`,
  },
  {
    kind: 'response',
    name: 'Boolean derivation',
    description: 'Compute a boolean variable from a comparison',
    snippet: `{
  approved: (.body.status == "succeeded"),
  is_paid:  ((.body.amount_paid // 0) >= .body.amount_due)
}`,
  },
  {
    kind: 'response',
    name: 'Array length',
    description: 'Count items in a list returned by the API',
    snippet: `{
  item_count: (.body.items | length)
}`,
  },
  {
    kind: 'response',
    name: 'First / last item',
    description: 'Pick the first or last element from a returned array',
    snippet: `{
  first_id: (.body.items[0].id // null),
  last_id:  (.body.items[-1].id // null)
}`,
  },
  {
    kind: 'response',
    name: 'Conditional value',
    description: 'Branch on a numeric or string comparison',
    snippet: `{
  tier: (if .body.amount > 10000 then "high"
         elif .body.amount > 1000 then "mid"
         else "low" end)
}`,
  },
  {
    kind: 'response',
    name: 'String interpolation',
    description: 'Compose a single human-readable summary variable',
    snippet: `{
  summary: "\\(.body.id) → \\(.body.status) (\\(.status))"
}`,
  },
  {
    kind: 'response',
    name: 'Filter then collect',
    description: 'Use select() to keep only matching items, then collect a field',
    snippet: `{
  active_ids: [.body.items[] | select(.active) | .id]
}`,
  },
  {
    kind: 'response',
    name: 'Concat error messages',
    description: 'Join an array of error strings into one variable',
    snippet: `{
  error_summary: ((.body.errors // []) | map(.message) | join("; "))
}`,
  },
  {
    kind: 'response',
    name: 'Object → array of pairs',
    description: 'Flatten an object response into a list-of-pairs variable',
    snippet: `{
  meta: (.body.metadata // {} | to_entries | map({ k: .key, v: .value }))
}`,
  },
];

const ERROR_CODE_EXAMPLES: JqExample[] = [
  {
    kind: 'errorCode',
    name: 'Field from body',
    description: 'Use a top-level error code field; empty string passes through normally',
    snippet: `.body.errorCode // ""`,
  },
  {
    kind: 'errorCode',
    name: 'Nested field',
    description: 'Drill into a nested error object',
    snippet: `.body.error.code // ""`,
  },
  {
    kind: 'errorCode',
    name: '4xx/5xx → generic error',
    description: 'Route any non-2xx response to an error boundary',
    snippet: `if .status >= 400 then "HTTP_ERROR" else "" end`,
  },
  {
    kind: 'errorCode',
    name: '5xx only',
    description: 'Only treat server errors as BPMN errors; 4xx passes through normally',
    snippet: `if .status >= 500 then "SERVER_ERROR" else "" end`,
  },
  {
    kind: 'errorCode',
    name: 'Map status to code',
    description: 'Return a named code per HTTP status',
    snippet: `if .status == 401 then "UNAUTHORIZED"
elif .status == 403 then "FORBIDDEN"
elif .status == 404 then "NOT_FOUND"
elif .status >= 500 then "SERVER_ERROR"
else "" end`,
  },
  {
    kind: 'errorCode',
    name: 'Error flag in body',
    description: 'Treat a boolean success flag as a routing signal',
    snippet: `if .body.success == false then (.body.code // "FAILED") else "" end`,
  },
  {
    kind: 'errorCode',
    name: 'Header-based error',
    description: 'Route based on a custom response header',
    snippet: `if (.headers["x-error-code"] // "") != "" then .headers["x-error-code"] else "" end`,
  },
  {
    kind: 'errorCode',
    name: 'Always route to boundary',
    description: 'Unconditionally send to a BoundaryErrorEvent (useful for testing)',
    snippet: `"ALWAYS_ERROR"`,
  },
];

const ALL_EXAMPLES: JqExample[] = [...REQUEST_EXAMPLES, ...RESPONSE_EXAMPLES, ...ERROR_CODE_EXAMPLES];

// jq builtin functions exposed via autocomplete. Not exhaustive — covers
// the operators most users reach for in transform filters.
const JQ_BUILTINS = [
  'length', 'keys', 'keys_unsorted', 'values', 'has', 'in', 'inside', 'contains',
  'type', 'tonumber', 'tostring', 'ascii_downcase', 'ascii_upcase',
  'select', 'map', 'reduce', 'foreach',
  'add', 'any', 'all', 'min', 'max', 'min_by', 'max_by',
  'sort', 'sort_by', 'unique', 'unique_by', 'group_by', 'reverse', 'flatten',
  'first', 'last', 'limit', 'range', 'recurse', 'walk',
  'to_entries', 'from_entries', 'with_entries',
  'startswith', 'endswith', 'split', 'join', 'ltrimstr', 'rtrimstr',
  'test', 'match', 'capture', 'scan', 'sub', 'gsub', 'splits',
  'fromjson', 'tojson', 'tojson', 'env', 'now', 'todate', 'fromdate',
  'paths', 'leaf_paths', 'getpath', 'setpath', 'delpaths',
  'empty', 'error', 'not', 'if', 'then', 'else', 'elif', 'end',
];

function jqCompletions(kind: TransformKind) {
  // Top-level keys that exist on the input doc the engine hands the filter.
  const inputKeys =
    kind === 'request'
      ? [
          { label: '.instance_id', detail: 'string' },
          { label: '.execution_id', detail: 'string' },
          { label: '.vars', detail: 'object' },
        ]
      : [
          { label: '.status', detail: 'integer (HTTP status)' },
          { label: '.headers', detail: 'object (lowercased keys)' },
          { label: '.body', detail: 'parsed JSON body' },
        ];

  // Output-shape keys the engine consumes from the request transform.
  const outputKeys =
    kind === 'request'
      ? ['body', 'headers', 'query', 'path']
      : [];

  return (ctx: CompletionContext) => {
    const word = ctx.matchBefore(/[\w.\-]+/);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;
    return {
      from: word.from,
      options: [
        ...inputKeys.map(k => ({
          label: k.label,
          detail: k.detail,
          type: 'variable',
          boost: 99,
        })),
        ...outputKeys.map(k => ({
          label: k,
          detail: 'output key',
          type: 'property',
          boost: 90,
        })),
        ...JQ_BUILTINS.map(label => ({
          label,
          type: 'function',
        })),
      ],
    };
  };
}

const editorBaseTheme = EditorView.theme({
  '&': {
    fontSize: '12px',
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
    border: '1px solid #e2e8f0',
    borderRadius: '4px',
    overflow: 'hidden',
    background: '#ffffff',
  },
  '.cm-scroller': { lineHeight: '1.5' },
  '.cm-gutters': {
    background: '#f8fafc',
    border: 'none',
    color: '#94a3b8',
  },
  '.cm-activeLineGutter': { background: '#eef2ff' },
  '.cm-activeLine': { background: '#fafafa' },
  '&.cm-focused': { outline: '2px solid #6366f1', outlineOffset: '-2px' },
});

export interface JqEditorHandle {
  /** Append text to the current document and focus the editor with caret at the end. */
  appendAndFocus: (snippet: string) => void;
  focus: () => void;
}

const JqEditor = forwardRef<
  JqEditorHandle,
  {
    kind: TransformKind;
    value: string | undefined;
    placeholder: string;
    onChange: (v: string | undefined) => void;
    height: number;
  }
>(function JqEditor({ kind, value, placeholder, onChange, height }, ref) {
  const cmRef = useRef<ReactCodeMirrorRef>(null);

  useImperativeHandle(
    ref,
    (): JqEditorHandle => ({
      focus: () => cmRef.current?.view?.focus(),
      appendAndFocus: (snippet: string) => {
        const view = cmRef.current?.view;
        if (!view) return;
        const current = view.state.doc.toString();
        const insertText = current.trim() ? `\n\n${snippet}` : snippet;
        const at = current.length;
        view.dispatch({
          changes: { from: at, to: at, insert: insertText },
          selection: { anchor: at + insertText.length },
          scrollIntoView: true,
        });
        view.focus();
      },
    }),
    [],
  );

  const extensions = useMemo(
    () => [
      json(),
      autocompletion({ override: [jqCompletions(kind)] }),
      editorBaseTheme,
      EditorView.lineWrapping,
    ],
    [kind],
  );

  return (
    <CodeMirror
      ref={cmRef}
      value={value ?? ''}
      placeholder={placeholder}
      height={`${height}px`}
      extensions={extensions}
      onChange={v => onChange(v || undefined)}
      basicSetup={{
        lineNumbers: true,
        foldGutter: false,
        bracketMatching: true,
        closeBrackets: true,
        autocompletion: true,
        highlightActiveLine: true,
        highlightSelectionMatches: false,
        searchKeymap: false,
      }}
    />
  );
});

function TransformField({
  kind,
  label,
  suffix = 'transform',
  value,
  placeholder,
  onChange,
  height = 280,
  examplesOpenForThisKind,
  onToggleExamples,
  editorRef,
}: {
  kind: TransformKind;
  label: string;
  suffix?: string;
  value: string | undefined;
  placeholder: string;
  onChange: (v: string | undefined) => void;
  height?: number;
  examplesOpenForThisKind: boolean;
  onToggleExamples: () => void;
  editorRef?: Ref<JqEditorHandle>;
}) {
  return (
    <div style={{ marginTop: 12 }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'baseline',
          marginBottom: 6,
        }}
      >
        <div
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: '#475569',
          }}
        >
          {label} {suffix}{' '}
          <span style={{ color: '#94a3b8', fontWeight: 400 }}>(jq filter)</span>
        </div>
        <button
          type="button"
          onClick={onToggleExamples}
          style={{
            padding: '3px 10px',
            fontSize: 11,
            border: '1px solid #cbd5e1',
            borderRadius: 4,
            background: examplesOpenForThisKind ? '#0f172a' : '#ffffff',
            color: examplesOpenForThisKind ? '#ffffff' : '#0f172a',
            cursor: 'pointer',
            whiteSpace: 'nowrap',
          }}
        >
          {examplesOpenForThisKind ? 'Hide examples' : 'Show examples'}
        </button>
      </div>
      <JqEditor
        ref={editorRef}
        kind={kind}
        value={value}
        placeholder={placeholder}
        onChange={onChange}
        height={height}
      />
    </div>
  );
}

type SidePanelTab = 'all' | TransformKind | 'reference';

function JqExamplesSidePanel({
  height,
  filter,
  onFilterChange,
  onUseRequest,
  onUseResponse,
  onUseErrorCode,
  onClose,
}: {
  height: string;
  filter: SidePanelTab;
  onFilterChange: (f: SidePanelTab) => void;
  onUseRequest: (snippet: string) => void;
  onUseResponse: (snippet: string) => void;
  onUseErrorCode: (snippet: string) => void;
  onClose: () => void;
}) {
  const visible =
    filter === 'all' || filter === 'reference'
      ? ALL_EXAMPLES
      : ALL_EXAMPLES.filter(e => e.kind === filter);

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        width: 380,
        height,
        background: 'var(--color-surface)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius)',
        boxShadow: '0 4px 16px rgba(0, 0, 0, 0.08)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        style={{
          padding: '14px 16px 10px',
          borderBottom: '1px solid var(--color-border)',
          flex: '0 0 auto',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'flex-start',
          }}
        >
          <div>
            <div style={{ fontSize: 13, fontWeight: 600, color: '#0f172a', marginBottom: 2 }}>
              jq examples
            </div>
            <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 8 }}>
              Click <em>Use</em> to drop a snippet into the matching transform field.
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close examples"
            style={{
              border: 'none',
              background: 'transparent',
              fontSize: 18,
              color: 'var(--text-tertiary)',
              cursor: 'pointer',
              lineHeight: 1,
              padding: 0,
            }}
          >
            ×
          </button>
        </div>
        <div
          style={{
            display: 'inline-flex',
            border: '1px solid #e2e8f0',
            borderRadius: 4,
            overflow: 'hidden',
            fontSize: 11,
          }}
        >
          {(['all', 'request', 'response', 'errorCode', 'reference'] as const).map(f => (
            <button
              key={f}
              type="button"
              onClick={() => onFilterChange(f)}
              style={{
                padding: '4px 10px',
                border: 'none',
                background: filter === f ? '#0f172a' : '#ffffff',
                color: filter === f ? '#ffffff' : '#475569',
                cursor: 'pointer',
              }}
            >
              {{ all: 'All', request: 'Request', response: 'Response', errorCode: 'Errors', reference: 'Reference' }[f]}
            </button>
          ))}
        </div>
      </div>

      <div style={{ overflow: 'auto', flex: 1, padding: 10 }}>
        {filter === 'reference' ? (
          <JqReference />
        ) : (
          visible.map((ex, i) => (
            <ExampleCard
              key={`${ex.kind}-${ex.name}`}
              example={ex}
              isFirst={i === 0}
              onUse={
                ex.kind === 'request'
                  ? () => onUseRequest(ex.snippet)
                  : ex.kind === 'errorCode'
                    ? () => onUseErrorCode(ex.snippet)
                    : () => onUseResponse(ex.snippet)
              }
            />
          ))
        )}
      </div>
    </div>
  );
}

function JqReference() {
  return (
    <div style={{ padding: '4px 6px', fontSize: 12, lineHeight: 1.55, color: '#0f172a' }}>
      <RefSection title="Request input doc">
        <p>
          What the <em>request</em> transform receives. Compose any of these
          into your output object.
        </p>
        <RefCode>{`{
  instance_id:  "<uuid>",          // current process instance
  execution_id: "<uuid>",          // current token / execution
  vars: {                          // every instance variable
    <name>: <value>, ...
  }
}`}</RefCode>
        <p>
          Access vars by name: <code>.vars.amount</code>,{' '}
          <code>.vars.customer.email</code>. Missing vars resolve to{' '}
          <code>null</code>; pair with <code>// fallback</code> for safety.
        </p>
      </RefSection>

      <RefSection title="Request output shape">
        <p>
          What the engine consumes from the filter. Every key is optional —
          omit what you don't need.
        </p>
        <RefCode>{`{
  body:    <any>,                  // JSON body (skipped for GET/HEAD/DELETE)
  headers: { "<Name>": "<value>" },// merged with auth (auth wins on conflict)
  query:   { "<key>": "<value>" }, // urlencoded ?k=v pairs
  path:    { "<key>": "<value>" }  // substitutes :key in the URL
}`}</RefCode>
      </RefSection>

      <RefSection title="Response input doc">
        <p>What the <em>response</em> transform receives.</p>
        <RefCode>{`{
  status:  200,                    // HTTP status (integer)
  headers: { "x-rate-limit": "59" },// keys ALWAYS lowercased
  body:    <any>                   // parsed JSON, or raw string if non-JSON
}`}</RefCode>
      </RefSection>

      <RefSection title="Response output shape">
        <p>
          A flat object whose keys become instance variables. <code>null</code>{' '}
          values leave the variable unset rather than failing the task.
        </p>
        <RefCode>{`{
  charge_id:   .body.id,
  http_status: .status,
  rate_limit:  (.headers["x-rate-limit"] | tonumber? // null)
}`}</RefCode>
        <p>
          Variable types are inferred from JSON: strings → <code>string</code>,
          ints → <code>integer</code>, bools → <code>boolean</code>, anything
          else (floats, arrays, objects) → <code>json</code>.
        </p>
      </RefSection>

      <RefSection title="jq cheat sheet">
        <RefRow op=".field" desc="object field" />
        <RefRow op=".items[]" desc="iterate array elements" />
        <RefRow op=".items[0]" desc="index (negative ok: .items[-1])" />
        <RefRow op="a | b" desc="pipe — feed a's output into b" />
        <RefRow op="a // b" desc="default — b if a is null/false" />
        <RefRow op=".x?" desc="optional — null instead of error if .x missing" />
        <RefRow op={'"\\(.x)"'} desc="string interpolation" />
        <RefRow op="[ … ]" desc="array constructor" />
        <RefRow op="{ a: .x }" desc="object constructor" />
        <RefRow
          op="if c then a elif … else b end"
          desc="conditional"
        />
      </RefSection>

      <RefSection title="Common functions">
        <RefRow op="length" desc="array/string/object size" />
        <RefRow op="keys" desc="object keys (sorted)" />
        <RefRow op="select(p)" desc="keep values where p is truthy" />
        <RefRow op="map(f)" desc="apply f to each array element" />
        <RefRow op="tonumber" desc="parse number (use ? to soft-fail)" />
        <RefRow op="tostring" desc="coerce to string" />
        <RefRow op="contains(x)" desc="substring/subset check" />
        <RefRow op="to_entries" desc="object → [{ key, value }]" />
        <RefRow op="from_entries" desc="reverse of to_entries" />
        <RefRow op="add" desc="sum / concat / object-merge a list" />
        <RefRow op="join(sep)" desc="join an array of strings" />
        <RefRow op="now | todate" desc="current ISO 8601 timestamp" />
      </RefSection>

      <RefSection title="Behaviour notes">
        <ul style={{ margin: 0, paddingLeft: 18 }}>
          <li>
            Auth headers (<code>Authorization</code>, API-key header) always
            win on conflict — a transform-supplied <code>Authorization</code>{' '}
            is silently overwritten.
          </li>
          <li>Response header keys are <strong>lowercased</strong> before the filter sees them.</li>
          <li>
            A response variable resolved to <code>null</code> leaves the
            instance variable <em>unset</em>; it does not fail the task.
          </li>
          <li>
            Filters that fail to compile reject the deployment. Runtime errors
            (e.g. <code>tonumber</code> on a non-numeric string) fail the
            HTTP task, not the whole engine.
          </li>
          <li>
            Filter source is snapshotted onto the job at enqueue time, so
            redeploying the definition does not mutate in-flight calls.
          </li>
        </ul>
      </RefSection>
    </div>
  );
}

function RefSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 14 }}>
      <div
        style={{
          fontSize: 11,
          fontWeight: 700,
          color: '#475569',
          textTransform: 'uppercase',
          letterSpacing: 0.4,
          marginBottom: 6,
        }}
      >
        {title}
      </div>
      {children}
    </div>
  );
}

function RefCode({ children }: { children: string }) {
  return (
    <pre
      style={{
        margin: '6px 0',
        padding: 8,
        fontFamily: 'ui-monospace, monospace',
        fontSize: 11,
        lineHeight: 1.45,
        background: '#f8fafc',
        border: '1px solid #e2e8f0',
        borderRadius: 4,
        color: '#0f172a',
        overflow: 'auto',
        whiteSpace: 'pre',
      }}
    >
      {children}
    </pre>
  );
}

function RefRow({ op, desc }: { op: string; desc: string }) {
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: '120px 1fr',
        gap: 8,
        padding: '2px 0',
        fontSize: 11,
      }}
    >
      <code
        style={{
          fontFamily: 'ui-monospace, monospace',
          color: '#1e293b',
          background: '#f1f5f9',
          padding: '1px 6px',
          borderRadius: 3,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}
      >
        {op}
      </code>
      <span style={{ color: '#475569' }}>{desc}</span>
    </div>
  );
}

function ExampleCard({
  example,
  isFirst,
  onUse,
}: {
  example: JqExample;
  isFirst: boolean;
  onUse: () => void;
}) {
  return (
    <div
      style={{
        padding: '10px 4px',
        borderTop: isFirst ? 'none' : '1px solid #f1f5f9',
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'baseline',
          marginBottom: 4,
          gap: 8,
        }}
      >
        <div style={{ minWidth: 0, flex: 1 }}>
          <div
            style={{
              fontSize: 11,
              color: '#64748b',
              marginBottom: 1,
              textTransform: 'uppercase',
              letterSpacing: 0.4,
            }}
          >
            {example.kind}
          </div>
          <div style={{ fontSize: 12, fontWeight: 600, color: '#0f172a' }}>
            {example.name}
          </div>
          <div style={{ fontSize: 11, color: '#64748b', marginTop: 2 }}>
            {example.description}
          </div>
        </div>
        <button
          type="button"
          title={`Append to ${example.kind} transform`}
          onClick={onUse}
          style={{
            padding: '3px 10px',
            fontSize: 11,
            border: '1px solid #cbd5e1',
            borderRadius: 4,
            background: '#ffffff',
            color: '#0f172a',
            cursor: 'pointer',
            whiteSpace: 'nowrap',
          }}
        >
          Use
        </button>
      </div>
      <pre
        style={{
          margin: 0,
          padding: 8,
          fontFamily: 'ui-monospace, monospace',
          fontSize: 11,
          lineHeight: 1.5,
          background: '#f8fafc',
          border: '1px solid #e2e8f0',
          borderRadius: 4,
          color: '#0f172a',
          overflow: 'auto',
          whiteSpace: 'pre',
        }}
      >
        {example.snippet}
      </pre>
    </div>
  );
}
