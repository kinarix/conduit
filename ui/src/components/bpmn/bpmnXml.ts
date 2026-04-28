import type { Node, Edge } from '@xyflow/react';
import type { BpmnNodeData, BpmnEdgeData, BpmnElementType } from './bpmnTypes';
import { NODE_DIMENSIONS } from './bpmnTypes';

const BPMN_NS    = 'http://www.omg.org/spec/BPMN/20100524/MODEL';
const DI_NS      = 'http://www.omg.org/spec/BPMN/20100524/DI';
const DC_NS      = 'http://www.omg.org/spec/DD/20100524/DC';
const CONDUIT_NS = 'http://conduit.io/ext';

const TAG_MAP: Record<BpmnElementType, string> = {
  startEvent:       'startEvent',
  endEvent:         'endEvent',
  userTask:         'userTask',
  serviceTask:      'serviceTask',
  exclusiveGateway: 'exclusiveGateway',
  parallelGateway:  'parallelGateway',
  inclusiveGateway: 'inclusiveGateway',
};

export function toXml(
  nodes: Node[],
  edges: Edge[],
  processId: string,
  processName: string,
  inputSchema?: string,
): string {
  const esc = (s: string) =>
    s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

  const elementLines: string[] = [];
  const shapeLines: string[] = [];

  if (inputSchema?.trim()) {
    elementLines.push(
      `    <extensionElements>` +
      `\n      <conduit:inputSchema>${esc(inputSchema.trim())}</conduit:inputSchema>` +
      `\n    </extensionElements>`,
    );
  }

  for (const n of nodes) {
    const d = n.data as BpmnNodeData;
    const tag = TAG_MAP[d.bpmnType];
    const attrs: string[] = [`id="${esc(n.id)}"`];
    if (d.label) attrs.push(`name="${esc(d.label)}"`);
    if (d.bpmnType === 'serviceTask') {
      if (d.topic) attrs.push(`topic="${esc(d.topic)}"`);
      if (d.url)   attrs.push(`url="${esc(d.url)}"`);
    }

    if (d.schema?.trim()) {
      elementLines.push(
        `    <${tag} ${attrs.join(' ')}>` +
        `\n      <extensionElements>` +
        `\n        <conduit:inputSchema>${esc(d.schema.trim())}</conduit:inputSchema>` +
        `\n      </extensionElements>` +
        `\n    </${tag}>`,
      );
    } else {
      elementLines.push(`    <${tag} ${attrs.join(' ')}/>`);
    }

    const dim = NODE_DIMENSIONS[d.bpmnType] ?? { width: 80, height: 40 };
    shapeLines.push(
      `    <bpmndi:BPMNShape id="${esc(n.id)}_di" bpmnElement="${esc(n.id)}">` +
      `\n      <dc:Bounds x="${Math.round(n.position.x)}" y="${Math.round(n.position.y)}" width="${dim.width}" height="${dim.height}"/>` +
      `\n    </bpmndi:BPMNShape>`,
    );
  }

  const edgeLines: string[] = [];
  const waypointLines: string[] = [];

  for (const e of edges) {
    const d = e.data as BpmnEdgeData | undefined;
    const attrs = [
      `id="${esc(e.id)}"`,
      `sourceRef="${esc(e.source)}"`,
      `targetRef="${esc(e.target)}"`,
    ];
    if (d?.condition) {
      edgeLines.push(
        `    <sequenceFlow ${attrs.join(' ')}>` +
        `\n      <conditionExpression>${esc(d.condition)}</conditionExpression>` +
        `\n    </sequenceFlow>`,
      );
    } else {
      edgeLines.push(`    <sequenceFlow ${attrs.join(' ')}/>`);
    }

    waypointLines.push(
      `    <bpmndi:BPMNEdge id="${esc(e.id)}_di" bpmnElement="${esc(e.id)}">` +
      `\n    </bpmndi:BPMNEdge>`,
    );
  }

  return `<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI"
             xmlns:dc="http://www.omg.org/spec/DD/20100524/DC"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io">
  <process id="${esc(processId)}" name="${esc(processName)}">
${elementLines.join('\n')}
${edgeLines.join('\n')}
  </process>
  <bpmndi:BPMNDiagram id="diagram">
    <bpmndi:BPMNPlane bpmnElement="${esc(processId)}">
${shapeLines.join('\n')}
${waypointLines.join('\n')}
    </bpmndi:BPMNPlane>
  </bpmndi:BPMNDiagram>
</definitions>`;
}

const REVERSE_TAG: Partial<Record<string, BpmnElementType>> = {
  startEvent:       'startEvent',
  endEvent:         'endEvent',
  userTask:         'userTask',
  serviceTask:      'serviceTask',
  exclusiveGateway: 'exclusiveGateway',
  parallelGateway:  'parallelGateway',
  inclusiveGateway: 'inclusiveGateway',
};

export interface ParsedBpmn {
  nodes: Node[];
  edges: Edge[];
  processId: string;
  processName: string;
  inputSchema?: string;
}

export function fromXml(xml: string): ParsedBpmn {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xml, 'application/xml');

  const parseErr = doc.querySelector('parsererror');
  if (parseErr) throw new Error('Invalid XML');

  const processEl =
    doc.getElementsByTagNameNS(BPMN_NS, 'process')[0] ??
    doc.querySelector('process');
  if (!processEl) throw new Error('No <process> element found');

  const processId = processEl.getAttribute('id') ?? 'process_1';
  const processName = processEl.getAttribute('name') ?? processId;

  // Parse process-level inputSchema from extensionElements
  const inputSchema = extractSchemaText(processEl);

  // Parse positions from BPMNDiagram section
  const posMap = new Map<string, { x: number; y: number }>();
  const allShapes = [
    ...Array.from(doc.getElementsByTagNameNS(DI_NS, 'BPMNShape')),
    ...Array.from(doc.querySelectorAll('BPMNShape')),
  ];
  for (const shape of allShapes) {
    const ref = shape.getAttribute('bpmnElement');
    const bounds =
      shape.getElementsByTagNameNS(DC_NS, 'Bounds')[0] ??
      shape.querySelector('Bounds');
    if (ref && bounds) {
      posMap.set(ref, {
        x: parseFloat(bounds.getAttribute('x') ?? '0'),
        y: parseFloat(bounds.getAttribute('y') ?? '0'),
      });
    }
  }

  const nodes: Node[] = [];
  const edges: Edge[] = [];
  let autoX = 80;

  for (const child of Array.from(processEl.children)) {
    const localName = child.localName;
    const bpmnType = REVERSE_TAG[localName];
    if (bpmnType) {
      const id = child.getAttribute('id') ?? `node_${nodes.length}`;
      const label = child.getAttribute('name') ?? '';
      const data: BpmnNodeData = { bpmnType, label };
      if (bpmnType === 'serviceTask') {
        data.topic = child.getAttribute('topic') ?? undefined;
        data.url   = child.getAttribute('url') ?? undefined;
      }
      const nodeSchema = extractSchemaText(child);
      if (nodeSchema) data.schema = nodeSchema;

      const pos = posMap.get(id) ?? { x: autoX, y: 150 };
      autoX += 160;
      nodes.push({
        id,
        type: nodeTypeFor(bpmnType),
        position: pos,
        data,
      });
    } else if (localName === 'sequenceFlow') {
      const id     = child.getAttribute('id') ?? `edge_${edges.length}`;
      const source = child.getAttribute('sourceRef') ?? '';
      const target = child.getAttribute('targetRef') ?? '';
      const condEl = child.querySelector('conditionExpression');
      const data: BpmnEdgeData = condEl ? { condition: condEl.textContent ?? '' } : {};
      edges.push({
        id,
        source,
        target,
        data,
        label: data.condition ?? undefined,
      });
    }
  }

  return { nodes, edges, processId, processName, inputSchema };
}

function extractSchemaText(el: Element): string | undefined {
  for (const child of Array.from(el.children)) {
    if (child.localName !== 'extensionElements') continue;
    for (const inner of Array.from(child.children)) {
      const ns = inner.namespaceURI ?? inner.getAttribute('xmlns:conduit') ?? '';
      const isConduitSchema =
        inner.localName === 'inputSchema' && ns === CONDUIT_NS;
      if (isConduitSchema) {
        const text = inner.textContent?.trim();
        if (text) return text;
      }
    }
  }
  return undefined;
}

function nodeTypeFor(t: BpmnElementType): string {
  if (t === 'startEvent' || t === 'endEvent') return 'bpmnEvent';
  if (t === 'userTask' || t === 'serviceTask') return 'bpmnTask';
  return 'bpmnGateway';
}
