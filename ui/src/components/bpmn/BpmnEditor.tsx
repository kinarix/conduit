import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useRef,
  useState,
  useEffect,
} from 'react';
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  Panel,
  addEdge,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type OnConnect,
  type NodeTypes,
  type Connection,
  ReactFlowProvider,
  useReactFlow,
  useViewport,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import EventNode from './nodes/EventNode';
import TaskNode from './nodes/TaskNode';
import GatewayNode from './nodes/GatewayNode';
import BpmnPalette from './BpmnPalette';
import BpmnProperties from './BpmnProperties';
import { toXml, fromXml } from './bpmnXml';
import type { BpmnNodeData, BpmnEdgeData, BpmnElementType } from './bpmnTypes';
import { NODE_DIMENSIONS } from './bpmnTypes';
import { applyAutoLayout } from './autoLayout';

function ZoomDisplay() {
  const { zoom } = useViewport();
  return (
    <div style={{
      position: 'absolute',
      bottom: 12,
      right: 12,
      zIndex: 5,
      background: 'rgba(255,255,255,0.85)',
      border: '1px solid #e2e8f0',
      borderRadius: 4,
      padding: '2px 7px',
      fontSize: 11,
      fontWeight: 500,
      color: '#64748b',
      pointerEvents: 'none',
    }}>
      {Math.round(zoom * 100)}%
    </div>
  );
}

const nodeTypes: NodeTypes = {
  bpmnEvent:   EventNode,
  bpmnTask:    TaskNode,
  bpmnGateway: GatewayNode,
};

function nodeTypeFor(t: BpmnElementType): string {
  if (t === 'startEvent' || t === 'endEvent') return 'bpmnEvent';
  if (t === 'userTask' || t === 'serviceTask') return 'bpmnTask';
  return 'bpmnGateway';
}

const DEFAULT_NODES: Node[] = [
  {
    id: 'start_1',
    type: 'bpmnEvent',
    position: { x: 80, y: 180 },
    data: { bpmnType: 'startEvent', label: 'Start' } as BpmnNodeData,
  },
];

export interface BpmnEditorHandle {
  getXml: () => Promise<string>;
}

interface Props {
  xml?: string;
  processId?: string;
  processName?: string;
}

function BpmnEditorInner({ xml, processId: initPid, processName: initPname }: Props, ref: React.Ref<BpmnEditorHandle>) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>(DEFAULT_NODES);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [selected, setSelected] = useState<Node | Edge | null>(null);
  const [processId, setProcessId] = useState(initPid ?? `process_${Date.now()}`);
  const [processName, setProcessName] = useState(initPname ?? '');
  const [processSchema, setProcessSchema] = useState<string | undefined>(undefined);
  const [propWidth, setPropWidth] = useState(240);
  const reactFlow = useReactFlow();
  const wrapperRef = useRef<HTMLDivElement>(null);
  const resizingRef = useRef(false);
  const resizeStartX = useRef(0);
  const resizeStartW = useRef(0);

  const onResizeStart = useCallback((e: React.MouseEvent) => {
    resizingRef.current = true;
    resizeStartX.current = e.clientX;
    resizeStartW.current = propWidth;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const onMove = (ev: MouseEvent) => {
      if (!resizingRef.current) return;
      const delta = resizeStartX.current - ev.clientX;
      setPropWidth(Math.max(180, Math.min(520, resizeStartW.current + delta)));
    };
    const onUp = () => {
      resizingRef.current = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, [propWidth]);

  useEffect(() => {
    if (!xml) return;
    try {
      const parsed = fromXml(xml);
      setNodes(parsed.nodes);
      setEdges(parsed.edges);
      setProcessId(parsed.processId);
      setProcessName(parsed.processName);
      setProcessSchema(parsed.inputSchema);
    } catch (e) {
      console.error('Failed to parse BPMN XML', e);
    }
  }, [xml]);

  useImperativeHandle(ref, () => ({
    getXml: async () => toXml(nodes, edges, processId, processName, processSchema),
  }));

  const onConnect: OnConnect = useCallback(
    (conn: Connection) => {
      const id = `flow_${Date.now()}`;
      setEdges(eds => addEdge({ ...conn, id, data: {} as BpmnEdgeData }, eds));
    },
    [setEdges],
  );

  const onDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const bpmnType = e.dataTransfer.getData('application/bpmn-type') as BpmnElementType;
      if (!bpmnType) return;

      const pos = reactFlow.screenToFlowPosition({
        x: e.clientX,
        y: e.clientY,
      });

      const id = `${bpmnType}_${Date.now()}`;
      const dim = NODE_DIMENSIONS[bpmnType];
      const newNode: Node = {
        id,
        type: nodeTypeFor(bpmnType),
        position: { x: pos.x - dim.width / 2, y: pos.y - dim.height / 2 },
        data: { bpmnType, label: '' } as BpmnNodeData,
      };
      setNodes(ns => [...ns, newNode]);
    },
    [reactFlow, setNodes],
  );

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelected(node);
  }, []);

  const onEdgeClick = useCallback((_: React.MouseEvent, edge: Edge) => {
    setSelected(edge);
  }, []);

  const onPaneClick = useCallback(() => setSelected(null), []);

  const onNodeChange = useCallback((id: string, patch: Partial<BpmnNodeData>) => {
    setNodes(ns =>
      ns.map(n => n.id === id ? { ...n, data: { ...n.data, ...patch } } : n),
    );
    setSelected(prev =>
      prev && 'position' in prev && prev.id === id
        ? { ...prev, data: { ...prev.data, ...patch } }
        : prev,
    );
  }, [setNodes]);

  const onAutoLayout = useCallback(() => {
    setNodes(ns => applyAutoLayout(ns, edges));
    setTimeout(() => reactFlow.fitView({ padding: 0.15, duration: 300 }), 50);
  }, [edges, setNodes, reactFlow]);

  const onEdgeChange = useCallback((id: string, patch: Partial<BpmnEdgeData>) => {
    setEdges(es =>
      es.map(e => e.id === id
        ? { ...e, data: { ...e.data, ...patch }, label: patch.condition ?? e.label }
        : e),
    );
    setSelected(prev =>
      prev && !('position' in prev) && prev.id === id
        ? { ...prev, data: { ...prev.data, ...patch } }
        : prev,
    );
  }, [setEdges]);

  return (
    <div style={{ display: 'flex', height: '100%', width: '100%' }}>
      <div ref={wrapperRef} style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onDrop={onDrop}
          onDragOver={onDragOver}
          onNodeClick={onNodeClick}
          onEdgeClick={onEdgeClick}
          onPaneClick={onPaneClick}
          fitView
          proOptions={{ hideAttribution: true }}
          style={{ background: '#f8fafc' }}
        >
          <Background variant={BackgroundVariant.Dots} color="#cbd5e1" gap={20} size={1.5} />
          <Controls />
          <MiniMap nodeColor={() => '#cbd5e1'} maskColor="rgba(248,250,252,0.7)" />
          <Panel position="top-right">
            <button
              onClick={onAutoLayout}
              title="Auto-arrange nodes using left-to-right layout"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                padding: '5px 10px',
                fontSize: 12,
                fontWeight: 500,
                color: '#475569',
                background: 'rgba(255,255,255,0.97)',
                border: '1px solid #e2e8f0',
                borderRadius: 6,
                cursor: 'pointer',
                boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
              }}
              onMouseEnter={e => { e.currentTarget.style.background = '#f1f5f9'; e.currentTarget.style.borderColor = '#94a3b8'; }}
              onMouseLeave={e => { e.currentTarget.style.background = 'rgba(255,255,255,0.97)'; e.currentTarget.style.borderColor = '#e2e8f0'; }}
            >
              <svg width={13} height={13} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.6}>
                <rect x={1} y={1} width={4} height={4} rx={1}/>
                <rect x={1} y={6} width={4} height={4} rx={1}/>
                <rect x={1} y={11} width={4} height={4} rx={1}/>
                <rect x={6} y={3.5} width={4} height={4} rx={1}/>
                <rect x={6} y={8.5} width={4} height={4} rx={1}/>
                <rect x={11} y={6} width={4} height={4} rx={1}/>
              </svg>
              Auto Arrange
            </button>
          </Panel>
        </ReactFlow>
        <BpmnPalette />
        <ZoomDisplay />
      </div>

      {/* Resize handle */}
      <div
        onMouseDown={onResizeStart}
        style={{
          width: 5,
          flexShrink: 0,
          cursor: 'col-resize',
          background: 'transparent',
          borderLeft: '1px solid #e2e8f0',
          transition: 'background 0.15s',
        }}
        onMouseEnter={e => (e.currentTarget.style.background = '#e2e8f0')}
        onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
      />

      {/* Properties panel */}
      <div style={{ width: propWidth, flexShrink: 0, borderLeft: 'none', overflow: 'hidden' }}>
        <BpmnProperties
          selected={selected}
          onNodeChange={onNodeChange}
          onEdgeChange={onEdgeChange}
          processSchema={processSchema}
          onProcessSchemaChange={setProcessSchema}
        />
      </div>
    </div>
  );
}

const BpmnEditorInnerRef = forwardRef(BpmnEditorInner);

const BpmnEditor = forwardRef<BpmnEditorHandle, Props>((props, ref) => (
  <ReactFlowProvider>
    <BpmnEditorInnerRef {...props} ref={ref} />
  </ReactFlowProvider>
));

BpmnEditor.displayName = 'BpmnEditor';

export default BpmnEditor;
