export type BpmnElementType =
  | 'startEvent'
  | 'messageStartEvent'
  | 'timerStartEvent'
  | 'endEvent'
  | 'userTask'
  | 'serviceTask'
  | 'businessRuleTask'
  | 'subProcess'
  | 'sendTask'
  | 'receiveTask'
  | 'exclusiveGateway'
  | 'parallelGateway'
  | 'inclusiveGateway'
  | 'boundaryTimerEvent'
  | 'boundarySignalEvent'
  | 'boundaryErrorEvent'
  | 'intermediateCatchTimerEvent'
  | 'intermediateCatchMessageEvent'
  | 'intermediateCatchSignalEvent';

export type RuntimeStatus = 'pending' | 'active' | 'completed' | 'error' | 'cancelled';

export type HttpAuthType = 'none' | 'basic' | 'bearer' | 'apiKey';

export interface HttpRetryConfig {
  max?: number;
  backoffMs?: number;
  multiplier?: number;
  retryOn?: string;
}

/** Phase 16 — declarative HTTP connector config attached to a service task. */
export interface HttpConnectorConfig {
  method?: string;
  timeoutMs?: number;
  authType?: HttpAuthType;
  secretRef?: string;
  /** Header name used when authType=apiKey. */
  apiKeyHeader?: string;
  /** Raw jq filter. Input: { instance_id, execution_id, vars }. Output: { body?, headers?, query?, path? } */
  requestTransform?: string;
  /** Raw jq filter. Input: { status, headers, body }. Output: flat { var: value, ... } */
  responseTransform?: string;
  retry?: HttpRetryConfig;
}

export interface BpmnNodeData extends Record<string, unknown> {
  bpmnType: BpmnElementType;
  label: string;
  topic?: string;
  url?: string;
  schema?: string;
  messageName?: string;
  correlationKey?: string;
  timerType?: 'date' | 'duration' | 'cycle';
  timerExpression?: string;
  signalName?: string;
  errorCode?: string;
  cancelling?: boolean;
  decisionRef?: string;
  /** Phase 16: HTTP connector config for service tasks. */
  http?: HttpConnectorConfig;
  /** When set, the node is rendered in viewer mode with this runtime overlay. */
  runtimeStatus?: RuntimeStatus;
}

export const RUNTIME_STATUS_COLOR: Record<RuntimeStatus, string> = {
  pending:   '#cbd5e1',
  active:    '#2563eb',
  completed: '#16a34a',
  error:     '#dc2626',
  cancelled: '#94a3b8',
};

export interface BpmnEdgeData extends Record<string, unknown> {
  condition?: string;
  kind?: 'attachment';
}

export const ELEMENT_LABELS: Record<BpmnElementType, string> = {
  startEvent:                    'Start Event',
  messageStartEvent:             'Msg Start',
  timerStartEvent:               'Timer Start',
  endEvent:                      'End Event',
  userTask:                      'User Task',
  serviceTask:                   'Service Task',
  businessRuleTask:              'Rule Task',
  subProcess:                    'Sub Process',
  sendTask:                      'Send Task',
  receiveTask:                   'Receive Task',
  exclusiveGateway:              'Exclusive GW',
  parallelGateway:               'Parallel GW',
  inclusiveGateway:              'Inclusive GW',
  boundaryTimerEvent:            'Boundary Timer',
  boundarySignalEvent:           'Boundary Signal',
  boundaryErrorEvent:            'Boundary Error',
  intermediateCatchTimerEvent:   'Timer Catch',
  intermediateCatchMessageEvent: 'Msg Catch',
  intermediateCatchSignalEvent:  'Signal Catch',
};

export const NODE_DIMENSIONS: Record<string, { width: number; height: number }> = {
  startEvent:                    { width: 22, height: 22 },
  messageStartEvent:             { width: 22, height: 22 },
  timerStartEvent:               { width: 22, height: 22 },
  endEvent:                      { width: 22, height: 22 },
  userTask:                      { width: 72, height: 36 },
  serviceTask:                   { width: 72, height: 36 },
  businessRuleTask:              { width: 72, height: 36 },
  subProcess:                    { width: 80, height: 44 },
  sendTask:                      { width: 72, height: 36 },
  receiveTask:                   { width: 72, height: 36 },
  exclusiveGateway:              { width: 30, height: 30 },
  parallelGateway:               { width: 30, height: 30 },
  inclusiveGateway:              { width: 30, height: 30 },
  boundaryTimerEvent:            { width: 22, height: 22 },
  boundarySignalEvent:           { width: 22, height: 22 },
  boundaryErrorEvent:            { width: 22, height: 22 },
  intermediateCatchTimerEvent:   { width: 22, height: 22 },
  intermediateCatchMessageEvent: { width: 22, height: 22 },
  intermediateCatchSignalEvent:  { width: 22, height: 22 },
};

export const ELEMENT_COLORS: Record<BpmnElementType, { stroke: string; fill: string; icon: string }> = {
  startEvent:                    { stroke: '#16a34a', fill: '#f0fdf4', icon: '#16a34a' },
  messageStartEvent:             { stroke: '#16a34a', fill: '#f0fdf4', icon: '#16a34a' },
  timerStartEvent:               { stroke: '#16a34a', fill: '#f0fdf4', icon: '#16a34a' },
  endEvent:                      { stroke: '#dc2626', fill: '#fef2f2', icon: '#dc2626' },
  userTask:                      { stroke: '#2563eb', fill: '#eff6ff', icon: '#2563eb' },
  serviceTask:                   { stroke: '#7c3aed', fill: '#f5f3ff', icon: '#7c3aed' },
  businessRuleTask:              { stroke: '#4338ca', fill: '#eef2ff', icon: '#4338ca' },
  subProcess:                    { stroke: '#475569', fill: '#f8fafc', icon: '#475569' },
  sendTask:                      { stroke: '#0891b2', fill: '#ecfeff', icon: '#0891b2' },
  receiveTask:                   { stroke: '#4f46e5', fill: '#eef2ff', icon: '#4f46e5' },
  exclusiveGateway:              { stroke: '#d97706', fill: '#fffbeb', icon: '#d97706' },
  parallelGateway:               { stroke: '#0d9488', fill: '#f0fdfa', icon: '#0d9488' },
  inclusiveGateway:              { stroke: '#0284c7', fill: '#f0f9ff', icon: '#0284c7' },
  boundaryTimerEvent:            { stroke: '#ea580c', fill: '#fff7ed', icon: '#ea580c' },
  boundarySignalEvent:           { stroke: '#ea580c', fill: '#fff7ed', icon: '#ea580c' },
  boundaryErrorEvent:            { stroke: '#b91c1c', fill: '#fef2f2', icon: '#b91c1c' },
  intermediateCatchTimerEvent:   { stroke: '#0d9488', fill: '#f0fdfa', icon: '#0d9488' },
  intermediateCatchMessageEvent: { stroke: '#0d9488', fill: '#f0fdfa', icon: '#0d9488' },
  intermediateCatchSignalEvent:  { stroke: '#0d9488', fill: '#f0fdfa', icon: '#0d9488' },
};
