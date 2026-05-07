import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  useEffect,
} from 'react';
import { ConnectingContext } from './connectingContext';
import { WarningsContext } from './warningsContext';
import { computeWarningsMap, computeInvalidEdgeIds } from './bpmnValidation';
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
  type EdgeTypes,
  type Connection,
  type EdgeProps,
  Position,
  type NodeChange,
  type EdgeChange,
  ReactFlowProvider,
  useReactFlow,
  useViewport,
  BaseEdge,
  getBezierPath,
  ConnectionMode,
  MarkerType,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import EventNode from './nodes/EventNode';
import TaskNode from './nodes/TaskNode';
import GatewayNode from './nodes/GatewayNode';
import BpmnPalette from './BpmnPalette';
import BpmnProperties from './BpmnProperties';
import { toXml, fromXml } from './bpmnXml';
import type { BpmnNodeData, BpmnEdgeData, BpmnElementType } from './bpmnTypes';
import type { LayoutData } from '../../api/deployments';
import { NODE_DIMENSIONS } from './bpmnTypes';
import { applyAutoLayout, recomputeAllEdgeHandles, spreadGatewayHandles } from './autoLayout';

function ZoomDisplay() {
  const { zoom } = useViewport();
  return (
    <div style={{
      position: 'absolute',
      bottom: 12,
      right: 12,
      zIndex: 5,
      background: 'rgba(255,255,255,0.85)',
      border: '1px solid var(--border-primary)',
      borderRadius: 4,
      padding: '2px 7px',
      fontSize: 'var(--text-sm)',
      fontWeight: 'var(--weight-medium)',
      color: '#64748b',
      pointerEvents: 'none',
    }}>
      {Math.round(zoom * 100)}%
    </div>
  );
}

function AttachmentEdge({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, selected }: EdgeProps) {
  const [edgePath] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });
  return (
    <BaseEdge
      id={id}
      path={edgePath}
      style={{
        stroke: selected ? '#6366f1' : '#94a3b8',
        strokeDasharray: '5 4',
        strokeWidth: selected ? 2 : 1.5,
      }}
    />
  );
}

const nodeTypes: NodeTypes = {
  bpmnEvent:   EventNode,
  bpmnTask:    TaskNode,
  bpmnGateway: GatewayNode,
};

const edgeTypes: EdgeTypes = {
  attachment: AttachmentEdge,
};

function CustomConnectionLine({ fromX, fromY, toX, toY, fromPosition, toPosition, connectionStatus, fromNode }: {
  fromX: number; fromY: number; toX: number; toY: number;
  fromPosition: Position; toPosition: Position;
  connectionStatus: 'valid' | 'invalid' | null;
  fromNode?: Node;
}) {
  const isInvalid = connectionStatus === 'invalid';
  const fromType = (fromNode?.data as BpmnNodeData | undefined)?.bpmnType;
  const isBoundarySrc = fromType && BOUNDARY_TYPES.has(fromType as BpmnElementType);
  const stroke = isInvalid ? '#ef4444' : '#94a3b8';
  const [edgePath] = getBezierPath({
    sourceX: fromX, sourceY: fromY, sourcePosition: fromPosition,
    targetX: toX, targetY: toY, targetPosition: toPosition,
  });
  return (
    <path
      fill="none"
      stroke={stroke}
      strokeWidth={1.5}
      strokeDasharray={(isInvalid || isBoundarySrc) ? '5 4' : undefined}
      d={edgePath}
    />
  );
}

const BOUNDARY_TYPES = new Set<BpmnElementType>([
  'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
]);

const TASK_TYPES = new Set<BpmnElementType>([
  'userTask', 'serviceTask', 'scriptTask', 'businessRuleTask', 'subProcess', 'sendTask', 'receiveTask',
]);

const START_EVENT_TYPES = new Set<BpmnElementType>([
  'startEvent', 'messageStartEvent', 'timerStartEvent',
]);

function nodeTypeFor(t: BpmnElementType): string {
  const eventTypes: BpmnElementType[] = [
    'startEvent', 'messageStartEvent', 'timerStartEvent', 'endEvent',
    'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
    'intermediateCatchTimerEvent', 'intermediateCatchMessageEvent', 'intermediateCatchSignalEvent',
  ];
  if (eventTypes.includes(t)) return 'bpmnEvent';
  if (t === 'userTask' || t === 'serviceTask' || t === 'scriptTask' || t === 'businessRuleTask' || t === 'subProcess' || t === 'sendTask' || t === 'receiveTask') return 'bpmnTask';
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
  loadXml: (xml: string) => void;
}

interface Props {
  xml?: string;
  processId?: string;
  processName?: string;
  onProcessNameChange?: (name: string) => void;
  initialLayout?: LayoutData;
  onLayoutChange?: (layout: LayoutData) => void;
}

function BpmnEditorInner({ xml, processId: initPid, processName: initPname, onProcessNameChange, initialLayout, onLayoutChange }: Props, ref: React.Ref<BpmnEditorHandle>) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>(DEFAULT_NODES);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [selected, setSelected] = useState<Node | Edge | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [processId, setProcessId] = useState(initPid ?? `process_${Date.now()}`);
  const [processName, setProcessName] = useState(initPname ?? '');
  const [processSchema, setProcessSchema] = useState<string | undefined>(undefined);
  const [propWidth, setPropWidth] = useState(240);
  const reactFlow = useReactFlow();
  const wrapperRef = useRef<HTMLDivElement>(null);
  const resizingRef = useRef(false);
  const resizeStartX = useRef(0);
  const resizeStartW = useRef(0);

  // Refs so getXml always reads the latest state, regardless of closure timing.
  const nodesRef = useRef(nodes);
  const edgesRef = useRef(edges);
  const processIdRef = useRef(processId);
  const processNameRef = useRef(processName);
  const processSchemaRef = useRef(processSchema);
  nodesRef.current = nodes;
  edgesRef.current = edges;
  processIdRef.current = processId;
  processNameRef.current = processName;
  processSchemaRef.current = processSchema;

  const suppressLayoutSave = useRef(false);
  const layoutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const warningsMap = useMemo(() => computeWarningsMap(nodes, edges), [nodes, edges]);
  const invalidEdgeIds = useMemo(() => computeInvalidEdgeIds(nodes, edges), [nodes, edges]);
  const displayEdges = useMemo(
    () => edges.map(e => {
      const data = e.data as BpmnEdgeData | undefined;
      if (data?.kind === 'attachment') return e;
      const isInvalid = invalidEdgeIds.has(e.id);
      const stroke = isInvalid ? '#ef4444' : (e.selected ? '#6366f1' : '#94a3b8');
      const strokeWidth = e.selected ? 2.5 : 1.5;
      return {
        ...e,
        style: {
          stroke,
          strokeWidth,
          ...(isInvalid ? { strokeDasharray: '5 4' } : {}),
        },
        markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: stroke },
      };
    }),
    [edges, invalidEdgeIds],
  );

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
      suppressLayoutSave.current = true;
      setNodes(parsed.nodes);
      setEdges(parsed.edges);
      setProcessId(parsed.processId);
      setProcessName(parsed.processName);
      onProcessNameChange?.(parsed.processName);
      setProcessSchema(parsed.inputSchema);
    } catch (e) {
      console.error('Failed to parse BPMN XML', e);
    }
  }, [xml]);

  // Apply saved layout positions/handles over the parsed nodes/edges.
  useEffect(() => {
    if (!initialLayout) return;
    suppressLayoutSave.current = true;
    setNodes(ns => ns.map(n => {
      const pos = (initialLayout.nodes ?? {})[n.id];
      return pos ? { ...n, position: pos } : n;
    }));
    setEdges(es => es.map(e => {
      const el = (initialLayout.edges ?? {})[e.id];
      if (!el) return e;
      return {
        ...e,
        ...(el.sourceHandle !== undefined ? { sourceHandle: el.sourceHandle } : {}),
        ...(el.targetHandle !== undefined ? { targetHandle: el.targetHandle } : {}),
      };
    }));
  }, [initialLayout]);

  // Debounced layout save — skipped immediately after XML load or layout overlay.
  useEffect(() => {
    if (suppressLayoutSave.current) {
      suppressLayoutSave.current = false;
      return;
    }
    if (!onLayoutChange) return;
    if (layoutTimerRef.current) clearTimeout(layoutTimerRef.current);
    layoutTimerRef.current = setTimeout(() => {
      const layout: LayoutData = {
        nodes: Object.fromEntries(nodesRef.current.map(n => [n.id, { x: n.position.x, y: n.position.y }])),
        edges: Object.fromEntries(edgesRef.current.map(e => [e.id, {
          sourceHandle: e.sourceHandle ?? undefined,
          targetHandle: e.targetHandle ?? undefined,
        }])),
      };
      onLayoutChange(layout);
    }, 800);
    return () => { if (layoutTimerRef.current) clearTimeout(layoutTimerRef.current); };
  }, [nodes, edges, onLayoutChange]);

  useImperativeHandle(ref, () => ({
    getXml: async () => toXml(nodesRef.current, edgesRef.current, processIdRef.current, processNameRef.current, processSchemaRef.current),
    loadXml: (xml: string) => {
      try {
        const parsed = fromXml(xml);
        suppressLayoutSave.current = true;
        setNodes(parsed.nodes);
        setEdges(parsed.edges);
        setProcessId(parsed.processId);
        setProcessName(parsed.processName);
        onProcessNameChange?.(parsed.processName);
        setProcessSchema(parsed.inputSchema);
      } catch (e) {
        console.error('Failed to parse imported BPMN XML', e);
        throw e;
      }
    },
  }));

  const onConnect: OnConnect = useCallback(
    (conn: Connection) => {
      if (conn.source === conn.target) return;
      if (edges.some(e => e.source === conn.source && e.target === conn.target)) return;
      const id = `flow_${Date.now()}`;
      const sourceNode = nodes.find(n => n.id === conn.source);
      const targetNode = nodes.find(n => n.id === conn.target);
      const sourceType = (sourceNode?.data as BpmnNodeData | undefined)?.bpmnType;
      const targetType = (targetNode?.data as BpmnNodeData | undefined)?.bpmnType;
      const isBoundarySource = sourceType && BOUNDARY_TYPES.has(sourceType);
      const isTaskTarget = targetType && TASK_TYPES.has(targetType);
      const alreadyAttached = edges.some(
        e => (e.data as BpmnEdgeData | undefined)?.kind === 'attachment' && e.source === conn.source,
      );
      const isAttachment = isBoundarySource && isTaskTarget && !alreadyAttached;
      if (isAttachment) {
        setEdges(eds => addEdge({
          ...conn,
          id,
          type: 'attachment',
          data: { kind: 'attachment' } as BpmnEdgeData,
        }, eds));
      } else {
        setEdges(eds => addEdge({ ...conn, id, data: {} as BpmnEdgeData }, eds));
      }
    },
    [setEdges, nodes, edges],
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

  const handleNodesChange = useCallback((changes: NodeChange[]) => {
    onNodesChange(changes);
    const removedIds = new Set(
      changes.filter(c => c.type === 'remove').map(c => (c as { id: string }).id),
    );
    if (removedIds.size > 0) {
      setSelected(prev => (prev && removedIds.has(prev.id) ? null : prev));
    }
  }, [onNodesChange]);

  const handleEdgesChange = useCallback((changes: EdgeChange[]) => {
    onEdgesChange(changes);
    const removedIds = new Set(
      changes.filter(c => c.type === 'remove').map(c => (c as { id: string }).id),
    );
    if (removedIds.size > 0) {
      setSelected(prev => (prev && removedIds.has(prev.id) ? null : prev));
    }
  }, [onEdgesChange]);

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
    const newNodes = applyAutoLayout(nodes, edges);
    const recomputed = recomputeAllEdgeHandles(newNodes, edges);
    const newEdges = spreadGatewayHandles(newNodes, recomputed);
    setNodes(newNodes);
    setEdges(newEdges);
    setTimeout(() => reactFlow.fitView({ padding: 0.15, duration: 300 }), 50);
  }, [edges, nodes, setNodes, setEdges, reactFlow]);

  const onEdgeChange = useCallback((id: string, patch: Partial<BpmnEdgeData>) => {
    setEdges(es =>
      es.map(e => {
        if (e.id !== id) return e;
        const d = (e.data ?? {}) as BpmnEdgeData;
        if (d.kind === 'attachment') return e;
        return { ...e, data: { ...d, ...patch }, label: patch.condition ?? e.label };
      }),
    );
    setSelected(prev =>
      prev && !('position' in prev) && prev.id === id
        ? { ...prev, data: { ...prev.data, ...patch } }
        : prev,
    );
  }, [setEdges]);

  const isValidConnection = useCallback((conn: Connection | Edge) => {
    if (conn.source === conn.target) return false;
    if (edges.some(e => e.source === conn.source && e.target === conn.target)) return false;
    const sourceType = (nodes.find(n => n.id === conn.source)?.data as BpmnNodeData | undefined)?.bpmnType;
    const targetType = (nodes.find(n => n.id === conn.target)?.data as BpmnNodeData | undefined)?.bpmnType;
    if (sourceType === 'endEvent') return false;
    if (targetType && START_EVENT_TYPES.has(targetType)) return false;
    return true;
  }, [nodes, edges]);

  return (
    <ConnectingContext.Provider value={connecting}>
    <WarningsContext.Provider value={warningsMap}>
    <div style={{ display: 'flex', height: '100%', width: '100%' }}>
      <div ref={wrapperRef} style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
        <ReactFlow
          nodes={nodes}
          edges={displayEdges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onNodesChange={handleNodesChange}
          onEdgesChange={handleEdgesChange}
          onConnect={onConnect}
          onDrop={onDrop}
          onDragOver={onDragOver}
          onNodeClick={onNodeClick}
          onEdgeClick={onEdgeClick}
          onPaneClick={onPaneClick}
          onConnectStart={() => setConnecting(true)}
          onConnectEnd={() => setConnecting(false)}
          connectionLineComponent={CustomConnectionLine}
          isValidConnection={isValidConnection}
          fitView
          connectionMode={ConnectionMode.Loose}
          proOptions={{ hideAttribution: true }}
          style={{ background: '#f8fafc' }}
        >
          <Background variant={BackgroundVariant.Dots} color="#cbd5e1" gap={20} size={1.5} />
          <Controls position="bottom-right" style={{ right: 12, bottom: 34 }} />
          <MiniMap nodeColor={() => '#cbd5e1'} maskColor="rgba(248,250,252,0.7)" position="bottom-left" />
          <Panel position="top-right">
            <button
              onClick={onAutoLayout}
              title="Auto-arrange nodes using left-to-right layout"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                padding: '5px 10px',
                fontSize: 'var(--text-md)',
                fontWeight: 'var(--weight-medium)',
                color: 'var(--text-secondary)',
                background: 'rgba(255,255,255,0.97)',
                border: '1px solid var(--border-primary)',
                borderRadius: 6,
                cursor: 'pointer',
                boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
              }}
              onMouseEnter={e => { e.currentTarget.style.background = 'var(--bg-tertiary)'; e.currentTarget.style.borderColor = 'var(--text-tertiary)'; }}
              onMouseLeave={e => { e.currentTarget.style.background = 'rgba(255,255,255,0.97)'; e.currentTarget.style.borderColor = 'var(--border-primary)'; }}
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
          borderLeft: '1px solid var(--border-primary)',
          transition: 'background 0.15s',
        }}
        onMouseEnter={e => (e.currentTarget.style.background = 'var(--bg-hover)')}
        onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
      />

      {/* Properties panel */}
      <div style={{ width: propWidth, flexShrink: 0, borderLeft: 'none', overflow: 'hidden' }}>
        <BpmnProperties
          selected={selected}
          nodes={nodes}
          edges={edges}
          onNodeChange={onNodeChange}
          onEdgeChange={onEdgeChange}
          processKey={processId}
          processName={processName}
          onProcessNameChange={(n) => {
            setProcessName(n);
            onProcessNameChange?.(n);
          }}
          processSchema={processSchema}
          onProcessSchemaChange={setProcessSchema}
        />
      </div>
    </div>
    </WarningsContext.Provider>
    </ConnectingContext.Provider>
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
