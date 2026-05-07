import type { CSSProperties } from 'react';

export const DOCS_BASE_URL =
  (import.meta.env.VITE_DOCS_BASE_URL as string | undefined) ?? 'https://conduit.kinarix.com';

export const ELEMENT_DOC_SLUGS: Partial<Record<string, string>> = {
  startEvent:                    'start-event',
  messageStartEvent:             'message-start-event',
  timerStartEvent:               'timer-start-event',
  endEvent:                      'end-event',
  userTask:                      'user-task',
  serviceTask:                   'service-task',
  scriptTask:                    'script-task',
  businessRuleTask:              'business-rule-task',
  subProcess:                    'sub-process',
  sendTask:                      'send-task',
  receiveTask:                   'receive-task',
  exclusiveGateway:              'exclusive-gateway',
  parallelGateway:               'parallel-gateway',
  inclusiveGateway:              'inclusive-gateway',
  boundaryTimerEvent:            'boundary-timer-event',
  boundarySignalEvent:           'boundary-signal-event',
  boundaryErrorEvent:            'boundary-error-event',
  intermediateCatchTimerEvent:   'timer-catch-event',
  intermediateCatchMessageEvent: 'message-catch-event',
  intermediateCatchSignalEvent:  'signal-catch-event',
  sequenceFlow:                  'sequence-flow',
};

export const helpIconStyle: CSSProperties = {
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  width: 18, height: 18, borderRadius: '50%',
  background: 'var(--accent-soft)',
  border: '1px solid var(--accent)',
  fontSize: 10, fontWeight: 700, color: 'var(--accent)',
  textDecoration: 'none', flexShrink: 0, lineHeight: 1,
  cursor: 'pointer', transition: 'background 150ms, color 150ms',
};

export function helpIconHover(el: HTMLAnchorElement, enter: boolean) {
  el.style.background = enter ? 'var(--accent)' : 'var(--accent-soft)';
  el.style.color = enter ? '#fff' : 'var(--accent)';
}
