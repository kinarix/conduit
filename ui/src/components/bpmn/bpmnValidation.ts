import type { Node, Edge } from '@xyflow/react';
import type { BpmnNodeData, BpmnEdgeData } from './bpmnTypes';

export const START_TYPES    = new Set(['startEvent', 'messageStartEvent', 'timerStartEvent']);
export const END_TYPES      = new Set(['endEvent']);
export const BOUNDARY_TYPES = new Set([
  'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
]);

const REQUIRED_FIELDS: Partial<Record<string, { field: keyof BpmnNodeData; msg: string }>> = {
  timerStartEvent:               { field: 'timerExpression', msg: 'Timer expression is required' },
  messageStartEvent:             { field: 'messageName',     msg: 'Message name is required' },
  intermediateCatchTimerEvent:   { field: 'timerExpression', msg: 'Timer expression is required' },
  intermediateCatchMessageEvent: { field: 'messageName',     msg: 'Message name is required' },
  intermediateCatchSignalEvent:  { field: 'signalName',      msg: 'Signal name is required' },
  boundaryTimerEvent:            { field: 'timerExpression', msg: 'Duration expression is required (e.g. PT30M)' },
  boundarySignalEvent:           { field: 'signalName',      msg: 'Signal name is required' },
  businessRuleTask:              { field: 'decisionRef',     msg: 'Decision ref is required' },
  sendTask:                      { field: 'messageName',     msg: 'Message name is required' },
  receiveTask:                   { field: 'messageName',     msg: 'Message name is required' },
};

export function computeNodeWarnings(nodeId: string, bpmnType: string, nodes: Node[], edges: Edge[]): string[] {
  const warnings: string[] = [];
  const d = (nodes.find(n => n.id === nodeId)?.data ?? {}) as BpmnNodeData;

  if (!START_TYPES.has(bpmnType)) {
    const hasStart = nodes.some(n => START_TYPES.has((n.data as BpmnNodeData).bpmnType));
    if (!hasStart) warnings.push('No start event in diagram — process cannot run');
  }

  if (START_TYPES.has(bpmnType)) {
    const hasIncoming = edges.some(e => e.target === nodeId && (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');
    if (hasIncoming) warnings.push('Start event must not have incoming flows');
  }

  if (END_TYPES.has(bpmnType)) {
    const hasOutgoing = edges.some(e => e.source === nodeId && (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');
    if (hasOutgoing) warnings.push('End event must not have outgoing flows');
  }

  if (!START_TYPES.has(bpmnType) && !BOUNDARY_TYPES.has(bpmnType)) {
    const hasIncoming = edges.some(e => e.target === nodeId && (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');
    if (!hasIncoming) warnings.push('No incoming flow — element is unreachable');
  }

  if (!END_TYPES.has(bpmnType)) {
    const hasOutgoing = edges.some(e => e.source === nodeId && (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');
    if (!hasOutgoing) warnings.push('No outgoing flow — process will stall here');
  }

  if (BOUNDARY_TYPES.has(bpmnType)) {
    const attachments = edges.filter(e => e.source === nodeId && (e.data as BpmnEdgeData | undefined)?.kind === 'attachment');
    if (attachments.length === 0) warnings.push('Not attached to a host task — draw a line from this event to a task');
    if (attachments.length > 1) warnings.push('Attached to multiple tasks — only one host allowed');
  }

  if (bpmnType === 'exclusiveGateway' || bpmnType === 'inclusiveGateway') {
    const outgoing = edges.filter(e => e.source === nodeId && (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');
    if (outgoing.length === 1) warnings.push('Gateway with only one outgoing flow will always take the same path');
  }

  const check = REQUIRED_FIELDS[bpmnType];
  if (check && !d[check.field]) warnings.push(check.msg);

  if (bpmnType === 'serviceTask' && !d.topic && !d.url) {
    warnings.push('Service task requires either a Topic or a URL');
  }

  return warnings;
}

export function computeWarningsMap(nodes: Node[], edges: Edge[]): Record<string, string[]> {
  const map: Record<string, string[]> = {};
  for (const node of nodes) {
    const bpmnType = (node.data as BpmnNodeData).bpmnType;
    const ws = computeNodeWarnings(node.id, bpmnType, nodes, edges);
    if (ws.length > 0) map[node.id] = ws;
  }
  return map;
}

export function computeInvalidEdgeIds(nodes: Node[], edges: Edge[]): Set<string> {
  const startIds = new Set(
    nodes.filter(n => START_TYPES.has((n.data as BpmnNodeData).bpmnType)).map(n => n.id),
  );
  const endIds = new Set(
    nodes.filter(n => END_TYPES.has((n.data as BpmnNodeData).bpmnType)).map(n => n.id),
  );
  const invalid = new Set<string>();
  for (const edge of edges) {
    const data = edge.data as BpmnEdgeData | undefined;
    if (data?.kind === 'attachment') continue;
    if (startIds.has(edge.target) || endIds.has(edge.source)) {
      invalid.add(edge.id);
    }
  }
  return invalid;
}
