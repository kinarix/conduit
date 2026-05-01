import type { Node, Edge } from '@xyflow/react';
import type {
  BpmnNodeData,
  BpmnEdgeData,
  BpmnElementType,
  HttpAuthType,
  HttpConnectorConfig,
  HttpRetryConfig,
} from './bpmnTypes';
import { NODE_DIMENSIONS } from './bpmnTypes';

const BPMN_NS    = 'http://www.omg.org/spec/BPMN/20100524/MODEL';
const DI_NS      = 'http://www.omg.org/spec/BPMN/20100524/DI';
const DC_NS      = 'http://www.omg.org/spec/DD/20100524/DC';
const CONDUIT_NS = 'http://conduit.io/ext';
const CAMUNDA_NS = 'http://camunda.org/schema/1.0/bpmn';

const BOUNDARY_BPMN_TYPES = new Set<BpmnElementType>([
  'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
]);

const TAG_MAP: Record<BpmnElementType, string> = {
  startEvent:                    'startEvent',
  messageStartEvent:             'startEvent',
  timerStartEvent:               'startEvent',
  endEvent:                      'endEvent',
  userTask:                      'userTask',
  serviceTask:                   'serviceTask',
  businessRuleTask:              'businessRuleTask',
  subProcess:                    'subProcess',
  sendTask:                      'sendTask',
  receiveTask:                   'receiveTask',
  exclusiveGateway:              'exclusiveGateway',
  parallelGateway:               'parallelGateway',
  inclusiveGateway:              'inclusiveGateway',
  boundaryTimerEvent:            'boundaryEvent',
  boundarySignalEvent:           'boundaryEvent',
  boundaryErrorEvent:            'boundaryEvent',
  intermediateCatchTimerEvent:   'intermediateCatchEvent',
  intermediateCatchMessageEvent: 'intermediateCatchEvent',
  intermediateCatchSignalEvent:  'intermediateCatchEvent',
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

  // Build attachment map: boundaryEventId → hostTaskId
  const attachedToMap = new Map<string, string>();
  for (const e of edges) {
    const d = e.data as BpmnEdgeData | undefined;
    if (d?.kind === 'attachment') attachedToMap.set(e.source, e.target);
  }

  // Collect definitions-level signal / error / message entries
  const defsSignals: string[] = [];
  const defsErrors: string[] = [];
  const defsMessages: string[] = [];

  for (const n of nodes) {
    const d = n.data as BpmnNodeData;
    if (d.bpmnType === 'boundarySignalEvent' || d.bpmnType === 'intermediateCatchSignalEvent') {
      if (d.signalName) defsSignals.push(`  <signal id="sig_${esc(n.id)}" name="${esc(d.signalName)}"/>`);
    }
    if (d.bpmnType === 'boundaryErrorEvent') {
      const codeAttr = d.errorCode ? ` errorCode="${esc(d.errorCode)}"` : '';
      defsErrors.push(`  <error id="err_${esc(n.id)}"${codeAttr}/>`);
    }
    if (d.bpmnType === 'messageStartEvent' || d.bpmnType === 'intermediateCatchMessageEvent'
        || d.bpmnType === 'sendTask' || d.bpmnType === 'receiveTask') {
      if (d.messageName) defsMessages.push(`  <message id="msg_${esc(n.id)}" name="${esc(d.messageName)}"/>`);
    }
  }

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

    if (BOUNDARY_BPMN_TYPES.has(d.bpmnType)) {
      const hostId = attachedToMap.get(n.id);
      if (hostId) attrs.push(`attachedToRef="${esc(hostId)}"`);
      attrs.push(`cancelActivity="${d.cancelling !== false ? 'true' : 'false'}"`);
    }

    if (d.bpmnType === 'serviceTask') {
      if (d.topic) attrs.push(`topic="${esc(d.topic)}"`);
      if (d.url)   attrs.push(`url="${esc(d.url)}"`);
    }

    if (d.bpmnType === 'businessRuleTask' && d.decisionRef) {
      attrs.push(`camunda:decisionRef="${esc(d.decisionRef)}"`);
    }

    if (d.bpmnType === 'receiveTask' && d.correlationKey) {
      attrs.push(`correlationKey="${esc(d.correlationKey)}"`);
    }

    const children: string[] = [];

    // Build a single <extensionElements> block holding all extension children
    // for this node — schema, http config, and any future additions. BPMN
    // allows at most one such block per element.
    const extLines: string[] = [];
    if (d.schema?.trim()) {
      extLines.push(`        <conduit:inputSchema>${esc(d.schema.trim())}</conduit:inputSchema>`);
    }
    if (d.bpmnType === 'serviceTask' && d.http && hasMeaningfulHttpConfig(d.http)) {
      extLines.push(serializeHttpConfig(d.http));
    }
    if (extLines.length > 0) {
      children.push(
        `      <extensionElements>` +
        `\n${extLines.join('\n')}` +
        `\n      </extensionElements>`,
      );
    }

    if (d.bpmnType === 'messageStartEvent') {
      const ref = d.messageName ? `msg_${esc(n.id)}` : '';
      children.push(`      <messageEventDefinition messageRef="${ref}"/>`);
    }

    if (d.bpmnType === 'timerStartEvent' && d.timerExpression) {
      const timerTag = d.timerType === 'date' ? 'timeDate'
        : d.timerType === 'duration' ? 'timeDuration'
        : 'timeCycle';
      children.push(
        `      <timerEventDefinition>` +
        `\n        <${timerTag}>${esc(d.timerExpression)}</${timerTag}>` +
        `\n      </timerEventDefinition>`,
      );
    }

    if (d.bpmnType === 'boundaryTimerEvent' && d.timerExpression) {
      children.push(
        `      <timerEventDefinition>` +
        `\n        <timeDuration>${esc(d.timerExpression)}</timeDuration>` +
        `\n      </timerEventDefinition>`,
      );
    }

    if (d.bpmnType === 'boundarySignalEvent') {
      const ref = d.signalName ? `sig_${esc(n.id)}` : '';
      children.push(`      <signalEventDefinition signalRef="${ref}"/>`);
    }

    if (d.bpmnType === 'boundaryErrorEvent') {
      children.push(`      <errorEventDefinition errorRef="err_${esc(n.id)}"/>`);
    }

    if (d.bpmnType === 'intermediateCatchTimerEvent' && d.timerExpression) {
      const timerTag = d.timerType === 'date' ? 'timeDate'
        : d.timerType === 'duration' ? 'timeDuration'
        : 'timeCycle';
      children.push(
        `      <timerEventDefinition>` +
        `\n        <${timerTag}>${esc(d.timerExpression)}</${timerTag}>` +
        `\n      </timerEventDefinition>`,
      );
    }

    if (d.bpmnType === 'intermediateCatchMessageEvent') {
      const ref = d.messageName ? `msg_${esc(n.id)}` : '';
      children.push(`      <messageEventDefinition messageRef="${ref}"/>`);
    }

    if (d.bpmnType === 'intermediateCatchSignalEvent') {
      const ref = d.signalName ? `sig_${esc(n.id)}` : '';
      children.push(`      <signalEventDefinition signalRef="${ref}"/>`);
    }

    if (d.bpmnType === 'sendTask' || d.bpmnType === 'receiveTask') {
      const ref = d.messageName ? `msg_${esc(n.id)}` : '';
      children.push(`      <messageEventDefinition messageRef="${ref}"/>`);
    }

    if (children.length > 0) {
      elementLines.push(`    <${tag} ${attrs.join(' ')}>\n${children.join('\n')}\n    </${tag}>`);
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
    // Attachment edges become attachedToRef on the boundary event element — skip here
    if (d?.kind === 'attachment') continue;

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

  const defsEntries = [...defsSignals, ...defsErrors, ...defsMessages].join('\n');

  return `<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI"
             xmlns:dc="http://www.omg.org/spec/DD/20100524/DC"
             xmlns:conduit="http://conduit.io/ext"
             xmlns:camunda="http://camunda.org/schema/1.0/bpmn"
             targetNamespace="http://conduit.io">
${defsEntries ? defsEntries + '\n' : ''}  <process id="${esc(processId)}" name="${esc(processName)}">
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
  businessRuleTask: 'businessRuleTask',
  subProcess:       'subProcess',
  sendTask:         'sendTask',
  receiveTask:      'receiveTask',
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

  // Collect definitions-level signal / error / message maps
  const signalMap  = new Map<string, string>(); // id → name
  const errorMap   = new Map<string, string>(); // id → errorCode (may be empty)
  const messageMap = new Map<string, string>(); // id → name

  for (const child of Array.from(doc.documentElement.children)) {
    const ln = child.localName;
    if (ln === 'signal') {
      const id = child.getAttribute('id');
      if (id) signalMap.set(id, child.getAttribute('name') ?? '');
    } else if (ln === 'error') {
      const id = child.getAttribute('id');
      if (id) errorMap.set(id, child.getAttribute('errorCode') ?? '');
    } else if (ln === 'message') {
      const id = child.getAttribute('id');
      if (id) messageMap.set(id, child.getAttribute('name') ?? '');
    }
  }

  const processEl =
    doc.getElementsByTagNameNS(BPMN_NS, 'process')[0] ??
    doc.querySelector('process');
  if (!processEl) throw new Error('No <process> element found');

  const processId = processEl.getAttribute('id') ?? 'process_1';
  const processName = processEl.getAttribute('name') ?? processId;

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
    let bpmnType = REVERSE_TAG[localName];

    // Refine startEvent subtypes
    if (bpmnType === 'startEvent') {
      const hasMsgDef = child.querySelector('messageEventDefinition') !== null
        || child.getElementsByTagNameNS(BPMN_NS, 'messageEventDefinition').length > 0;
      const hasTimerDef = child.querySelector('timerEventDefinition') !== null
        || child.getElementsByTagNameNS(BPMN_NS, 'timerEventDefinition').length > 0;
      if (hasMsgDef) bpmnType = 'messageStartEvent';
      else if (hasTimerDef) bpmnType = 'timerStartEvent';
    }

    if (bpmnType) {
      const id    = child.getAttribute('id') ?? `node_${nodes.length}`;
      const label = child.getAttribute('name') ?? '';
      const data: BpmnNodeData = { bpmnType, label };

      if (bpmnType === 'serviceTask') {
        data.topic = child.getAttribute('topic') ?? undefined;
        data.url   = child.getAttribute('url') ?? undefined;
        const http = extractHttpConfig(child);
        if (http) data.http = http;
      }

      if (bpmnType === 'businessRuleTask') {
        const ref = child.getAttributeNS(CAMUNDA_NS, 'decisionRef')
          ?? child.getAttribute('camunda:decisionRef');
        if (ref) data.decisionRef = ref;
      }

      if (bpmnType === 'sendTask' || bpmnType === 'receiveTask') {
        const msgDef = child.querySelector('messageEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'messageEventDefinition')[0];
        if (msgDef) {
          const ref = msgDef.getAttribute('messageRef');
          data.messageName = (ref ? (messageMap.get(ref) ?? ref) : undefined) || undefined;
        }
        if (bpmnType === 'receiveTask') {
          data.correlationKey = child.getAttribute('correlationKey') ?? undefined;
        }
      }

      if (bpmnType === 'messageStartEvent') {
        const msgDef = child.querySelector('messageEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'messageEventDefinition')[0];
        if (msgDef) {
          const ref = msgDef.getAttribute('messageRef');
          data.messageName = (ref ? (messageMap.get(ref) ?? ref) : undefined) || undefined;
        }
      }

      if (bpmnType === 'timerStartEvent') {
        const timerDef = child.querySelector('timerEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'timerEventDefinition')[0];
        if (timerDef) extractTimerData(timerDef, data);
      }

      const nodeSchema = extractSchemaText(child);
      if (nodeSchema) data.schema = nodeSchema;

      const pos = posMap.get(id) ?? { x: autoX, y: 150 };
      autoX += 160;
      nodes.push({ id, type: nodeTypeFor(bpmnType), position: pos, data });

    } else if (localName === 'boundaryEvent') {
      const id            = child.getAttribute('id') ?? `node_${nodes.length}`;
      const label         = child.getAttribute('name') ?? '';
      const attachedToRef = child.getAttribute('attachedToRef') ?? '';
      const cancelling    = child.getAttribute('cancelActivity') !== 'false';

      const hasTimer  = hasChildDef(child, 'timerEventDefinition');
      const hasSignal = hasChildDef(child, 'signalEventDefinition');
      const hasError  = hasChildDef(child, 'errorEventDefinition');

      let boundaryType: BpmnElementType | null = null;
      const data: BpmnNodeData = { bpmnType: 'boundaryTimerEvent', label, cancelling };

      if (hasTimer) {
        boundaryType = 'boundaryTimerEvent';
        const timerDef = child.querySelector('timerEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'timerEventDefinition')[0];
        if (timerDef) {
          const dur = timerDef.querySelector('timeDuration')
            ?? timerDef.getElementsByTagNameNS(BPMN_NS, 'timeDuration')[0];
          if (dur) data.timerExpression = dur.textContent?.trim() || undefined;
        }
      } else if (hasSignal) {
        boundaryType = 'boundarySignalEvent';
        const sigDef = child.querySelector('signalEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'signalEventDefinition')[0];
        if (sigDef) {
          const ref = sigDef.getAttribute('signalRef');
          data.signalName = (ref ? (signalMap.get(ref) ?? ref) : undefined) || undefined;
        }
      } else if (hasError) {
        boundaryType = 'boundaryErrorEvent';
        const errDef = child.querySelector('errorEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'errorEventDefinition')[0];
        if (errDef) {
          const ref = errDef.getAttribute('errorRef');
          if (ref) {
            const code = errorMap.get(ref);
            data.errorCode = (code !== undefined ? code : undefined) || undefined;
          }
        }
      }

      if (!boundaryType) continue;
      data.bpmnType = boundaryType;

      const pos = posMap.get(id) ?? { x: autoX, y: 150 };
      autoX += 160;
      nodes.push({ id, type: 'bpmnEvent', position: pos, data });

      if (attachedToRef) {
        edges.push({
          id:     `attach_${id}`,
          source: id,
          target: attachedToRef,
          type:   'attachment',
          data:   { kind: 'attachment' } as BpmnEdgeData,
        });
      }

    } else if (localName === 'intermediateCatchEvent') {
      const id    = child.getAttribute('id') ?? `node_${nodes.length}`;
      const label = child.getAttribute('name') ?? '';

      const hasTimer  = hasChildDef(child, 'timerEventDefinition');
      const hasMsg    = hasChildDef(child, 'messageEventDefinition');
      const hasSignal = hasChildDef(child, 'signalEventDefinition');

      const data: BpmnNodeData = { bpmnType: 'intermediateCatchTimerEvent', label };

      if (hasTimer) {
        data.bpmnType = 'intermediateCatchTimerEvent';
        const timerDef = child.querySelector('timerEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'timerEventDefinition')[0];
        if (timerDef) extractTimerData(timerDef, data);
      } else if (hasMsg) {
        data.bpmnType = 'intermediateCatchMessageEvent';
        const msgDef = child.querySelector('messageEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'messageEventDefinition')[0];
        if (msgDef) {
          const ref = msgDef.getAttribute('messageRef');
          data.messageName = (ref ? (messageMap.get(ref) ?? ref) : undefined) || undefined;
        }
      } else if (hasSignal) {
        data.bpmnType = 'intermediateCatchSignalEvent';
        const sigDef = child.querySelector('signalEventDefinition')
          ?? child.getElementsByTagNameNS(BPMN_NS, 'signalEventDefinition')[0];
        if (sigDef) {
          const ref = sigDef.getAttribute('signalRef');
          data.signalName = (ref ? (signalMap.get(ref) ?? ref) : undefined) || undefined;
        }
      }

      const pos = posMap.get(id) ?? { x: autoX, y: 150 };
      autoX += 160;
      nodes.push({ id, type: 'bpmnEvent', position: pos, data });

    } else if (localName === 'sequenceFlow') {
      const id     = child.getAttribute('id') ?? `edge_${edges.length}`;
      const source = child.getAttribute('sourceRef') ?? '';
      const target = child.getAttribute('targetRef') ?? '';
      const condEl = child.querySelector('conditionExpression');
      const data: BpmnEdgeData = condEl ? { condition: condEl.textContent ?? '' } : {};
      edges.push({ id, source, target, data, label: data.condition ?? undefined });
    }
  }

  return { nodes, edges, processId, processName, inputSchema };
}

function hasChildDef(el: Element, defLocalName: string): boolean {
  return el.querySelector(defLocalName) !== null
    || el.getElementsByTagNameNS(BPMN_NS, defLocalName).length > 0;
}

function extractTimerData(timerDef: Element, data: BpmnNodeData): void {
  const timeCycle    = timerDef.querySelector('timeCycle')    ?? timerDef.getElementsByTagNameNS(BPMN_NS, 'timeCycle')[0];
  const timeDate     = timerDef.querySelector('timeDate')     ?? timerDef.getElementsByTagNameNS(BPMN_NS, 'timeDate')[0];
  const timeDuration = timerDef.querySelector('timeDuration') ?? timerDef.getElementsByTagNameNS(BPMN_NS, 'timeDuration')[0];
  if (timeCycle)    { data.timerType = 'cycle';    data.timerExpression = timeCycle.textContent?.trim()    || undefined; }
  else if (timeDate)     { data.timerType = 'date';     data.timerExpression = timeDate.textContent?.trim()     || undefined; }
  else if (timeDuration) { data.timerType = 'duration'; data.timerExpression = timeDuration.textContent?.trim() || undefined; }
}

const xmlEsc = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

function hasMeaningfulHttpConfig(h: HttpConnectorConfig): boolean {
  // An empty/all-defaults config shouldn't be serialized — keeps the XML
  // clean for service tasks that don't actually use the connector.
  return !!(
    (h.method && h.method !== 'POST') ||
    h.timeoutMs !== undefined ||
    (h.authType && h.authType !== 'none') ||
    h.secretRef ||
    h.apiKeyHeader ||
    (h.requestTransform && h.requestTransform.trim()) ||
    (h.responseTransform && h.responseTransform.trim()) ||
    (h.retry && Object.keys(h.retry).length > 0)
  );
}

function serializeHttpConfig(h: HttpConnectorConfig): string {
  const attrs: string[] = [];
  attrs.push(`method="${xmlEsc(h.method ?? 'POST')}"`);
  if (h.timeoutMs !== undefined) attrs.push(`timeoutMs="${h.timeoutMs}"`);
  if (h.authType && h.authType !== 'none') attrs.push(`authType="${xmlEsc(h.authType)}"`);
  if (h.secretRef) attrs.push(`secretRef="${xmlEsc(h.secretRef)}"`);
  if (h.apiKeyHeader) attrs.push(`headerName="${xmlEsc(h.apiKeyHeader)}"`);

  const inner: string[] = [];
  if (h.requestTransform?.trim()) {
    inner.push(
      `          <conduit:requestTransform><![CDATA[\n${h.requestTransform.trim()}\n          ]]></conduit:requestTransform>`,
    );
  }
  if (h.responseTransform?.trim()) {
    inner.push(
      `          <conduit:responseTransform><![CDATA[\n${h.responseTransform.trim()}\n          ]]></conduit:responseTransform>`,
    );
  }
  if (h.retry) {
    const r = h.retry;
    const ra: string[] = [];
    if (r.max !== undefined) ra.push(`max="${r.max}"`);
    if (r.backoffMs !== undefined) ra.push(`backoffMs="${r.backoffMs}"`);
    if (r.multiplier !== undefined) ra.push(`multiplier="${r.multiplier}"`);
    if (r.retryOn) ra.push(`retryOn="${xmlEsc(r.retryOn)}"`);
    if (ra.length > 0) inner.push(`          <conduit:retry ${ra.join(' ')}/>`);
  }

  if (inner.length === 0) {
    return `        <conduit:http ${attrs.join(' ')}/>`;
  }
  return (
    `        <conduit:http ${attrs.join(' ')}>` +
    `\n${inner.join('\n')}` +
    `\n        </conduit:http>`
  );
}

function extractHttpConfig(el: Element): HttpConnectorConfig | undefined {
  for (const child of Array.from(el.children)) {
    if (child.localName !== 'extensionElements') continue;
    for (const inner of Array.from(child.children)) {
      const ns = inner.namespaceURI ?? '';
      if (inner.localName !== 'http' || ns !== CONDUIT_NS) continue;

      const cfg: HttpConnectorConfig = {};
      const method = inner.getAttribute('method');
      if (method) cfg.method = method;
      const t = inner.getAttribute('timeoutMs');
      if (t) {
        const n = parseInt(t, 10);
        if (!isNaN(n)) cfg.timeoutMs = n;
      }
      const auth = inner.getAttribute('authType') as HttpAuthType | null;
      if (auth) cfg.authType = auth;
      const ref = inner.getAttribute('secretRef');
      if (ref) cfg.secretRef = ref;
      const apiKeyHeader = inner.getAttribute('headerName');
      if (apiKeyHeader) cfg.apiKeyHeader = apiKeyHeader;

      for (const sub of Array.from(inner.children)) {
        const sns = sub.namespaceURI ?? '';
        if (sns !== CONDUIT_NS) continue;
        if (sub.localName === 'requestTransform') {
          const text = sub.textContent?.trim();
          if (text) cfg.requestTransform = text;
        } else if (sub.localName === 'responseTransform') {
          const text = sub.textContent?.trim();
          if (text) cfg.responseTransform = text;
        } else if (sub.localName === 'retry') {
          const retry: HttpRetryConfig = {};
          const max = sub.getAttribute('max');
          if (max) {
            const n = parseInt(max, 10);
            if (!isNaN(n)) retry.max = n;
          }
          const backoff = sub.getAttribute('backoffMs');
          if (backoff) {
            const n = parseInt(backoff, 10);
            if (!isNaN(n)) retry.backoffMs = n;
          }
          const mult = sub.getAttribute('multiplier');
          if (mult) {
            const n = parseFloat(mult);
            if (!isNaN(n)) retry.multiplier = n;
          }
          const retryOn = sub.getAttribute('retryOn');
          if (retryOn) retry.retryOn = retryOn;
          if (Object.keys(retry).length > 0) cfg.retry = retry;
        }
      }
      return cfg;
    }
  }
  return undefined;
}

function extractSchemaText(el: Element): string | undefined {
  for (const child of Array.from(el.children)) {
    if (child.localName !== 'extensionElements') continue;
    for (const inner of Array.from(child.children)) {
      const ns = inner.namespaceURI ?? inner.getAttribute('xmlns:conduit') ?? '';
      if (inner.localName === 'inputSchema' && ns === CONDUIT_NS) {
        const text = inner.textContent?.trim();
        if (text) return text;
      }
    }
  }
  return undefined;
}

function nodeTypeFor(t: BpmnElementType): string {
  const eventTypes: BpmnElementType[] = [
    'startEvent', 'messageStartEvent', 'timerStartEvent', 'endEvent',
    'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
    'intermediateCatchTimerEvent', 'intermediateCatchMessageEvent', 'intermediateCatchSignalEvent',
  ];
  if (eventTypes.includes(t)) return 'bpmnEvent';
  if (t === 'userTask' || t === 'serviceTask' || t === 'businessRuleTask' || t === 'subProcess' || t === 'sendTask' || t === 'receiveTask') return 'bpmnTask';
  return 'bpmnGateway';
}
