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
