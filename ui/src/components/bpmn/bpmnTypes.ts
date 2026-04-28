export type BpmnElementType =
  | 'startEvent'
  | 'endEvent'
  | 'userTask'
  | 'serviceTask'
  | 'exclusiveGateway'
  | 'parallelGateway'
  | 'inclusiveGateway';

export interface BpmnNodeData extends Record<string, unknown> {
  bpmnType: BpmnElementType;
  label: string;
  topic?: string;
  url?: string;
  schema?: string;  // JSON Schema for input variables (startEvent, userTask, serviceTask)
}

export interface BpmnEdgeData extends Record<string, unknown> {
  condition?: string;
}

export const ELEMENT_LABELS: Record<BpmnElementType, string> = {
  startEvent:       'Start Event',
  endEvent:         'End Event',
  userTask:         'User Task',
  serviceTask:      'Service Task',
  exclusiveGateway: 'Exclusive GW',
  parallelGateway:  'Parallel GW',
  inclusiveGateway: 'Inclusive GW',
};

export const NODE_DIMENSIONS: Record<string, { width: number; height: number }> = {
  startEvent:        { width: 22,  height: 22  },
  endEvent:          { width: 22,  height: 22  },
  userTask:          { width: 72,  height: 36  },
  serviceTask:       { width: 72,  height: 36  },
  exclusiveGateway:  { width: 30,  height: 30  },
  parallelGateway:   { width: 30,  height: 30  },
  inclusiveGateway:  { width: 30,  height: 30  },
};

export const ELEMENT_COLORS: Record<BpmnElementType, { stroke: string; fill: string; icon: string }> = {
  startEvent:       { stroke: '#16a34a', fill: '#f0fdf4', icon: '#16a34a' },
  endEvent:         { stroke: '#dc2626', fill: '#fef2f2', icon: '#dc2626' },
  userTask:         { stroke: '#2563eb', fill: '#eff6ff', icon: '#2563eb' },
  serviceTask:      { stroke: '#7c3aed', fill: '#f5f3ff', icon: '#7c3aed' },
  exclusiveGateway: { stroke: '#d97706', fill: '#fffbeb', icon: '#d97706' },
  parallelGateway:  { stroke: '#0d9488', fill: '#f0fdfa', icon: '#0d9488' },
  inclusiveGateway: { stroke: '#0284c7', fill: '#f0f9ff', icon: '#0284c7' },
};
