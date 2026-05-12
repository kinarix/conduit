import { useEffect, useMemo, useState } from 'react'
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  MarkerType,
  type Node,
  type Edge,
  type NodeTypes,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'

import EventNode from './nodes/EventNode'
import TaskNode from './nodes/TaskNode'
import GatewayNode from './nodes/GatewayNode'
import { fromXml } from './bpmnXml'
import { ConnectingContext } from './connectingContext'
import { WarningsContext } from './warningsContext'
import type { BpmnNodeData, BpmnElementType, RuntimeStatus } from './bpmnTypes'

interface Props {
  xml: string
  /** Map<element_id, RuntimeStatus>. Nodes without an entry default to 'pending'. */
  elementStates?: Map<string, RuntimeStatus>
  height?: number | string
}

const nodeTypes: NodeTypes = {
  bpmnEvent: EventNode,
  bpmnTask: TaskNode,
  bpmnGateway: GatewayNode,
}

function nodeTypeFor(t: BpmnElementType): string {
  const eventTypes: BpmnElementType[] = [
    'startEvent', 'messageStartEvent', 'timerStartEvent', 'endEvent',
    'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
    'intermediateCatchTimerEvent', 'intermediateCatchMessageEvent', 'intermediateCatchSignalEvent',
  ]
  if (eventTypes.includes(t)) return 'bpmnEvent'
  if (
    t === 'userTask' || t === 'serviceTask' || t === 'scriptTask' ||
    t === 'businessRuleTask' || t === 'subProcess' || t === 'sendTask' || t === 'receiveTask'
  ) return 'bpmnTask'
  return 'bpmnGateway'
}

// The BPMN editor stores sourceHandle/targetHandle in XML. These can occasionally
// be malformed (e.g. "left-source" saved as a targetHandle), which triggers
// ReactFlow error #008 and makes edges invisible. Keep valid handles so the viewer
// matches the modeller's port routing; recompute from geometry only when missing
// or malformed.
const VALID_SOURCE = new Set(['left-source', 'right-source', 'top-source', 'bottom-source'])
const VALID_TARGET = new Set(['left-target', 'right-target', 'top-target', 'bottom-target'])

function fixEdgeHandles(edges: Edge[], nodes: Node[]): Edge[] {
  const posMap = new Map(nodes.map(n => [n.id, n.position]))
  return edges.map(e => {
    // Attachment edges (boundary events) have no geometry-based handles
    if ((e.data as { kind?: string } | undefined)?.kind === 'attachment') return e

    const sourceOk = typeof e.sourceHandle === 'string' && VALID_SOURCE.has(e.sourceHandle)
    const targetOk = typeof e.targetHandle === 'string' && VALID_TARGET.has(e.targetHandle)
    if (sourceOk && targetOk) return e

    const srcPos = posMap.get(e.source)
    const tgtPos = posMap.get(e.target)
    if (!srcPos || !tgtPos) return { ...e, sourceHandle: undefined, targetHandle: undefined }

    const dx = tgtPos.x - srcPos.x
    const dy = tgtPos.y - srcPos.y
    let sourceHandle: string
    let targetHandle: string
    if (Math.abs(dx) >= Math.abs(dy)) {
      sourceHandle = dx >= 0 ? 'right-source' : 'left-source'
      targetHandle = dx >= 0 ? 'left-target' : 'right-target'
    } else {
      sourceHandle = dy >= 0 ? 'bottom-source' : 'top-source'
      targetHandle = dy >= 0 ? 'top-target' : 'bottom-target'
    }
    return {
      ...e,
      sourceHandle: sourceOk ? e.sourceHandle : sourceHandle,
      targetHandle: targetOk ? e.targetHandle : targetHandle,
    }
  })
}

export default function BpmnViewer({ xml, elementStates, height = '100%' }: Props) {
  return (
    <div style={{ width: '100%', height }}>
      <ReactFlowProvider>
        <ConnectingContext.Provider value={false}>
          <WarningsContext.Provider value={{}}>
            <ViewerInner xml={xml} elementStates={elementStates} />
          </WarningsContext.Provider>
        </ConnectingContext.Provider>
      </ReactFlowProvider>
    </div>
  )
}

function ViewerInner({ xml, elementStates }: { xml: string; elementStates?: Map<string, RuntimeStatus> }) {
  const [nodes, setNodes] = useState<Node[]>([])
  const [edges, setEdges] = useState<Edge[]>([])

  useEffect(() => {
    if (!xml) return
    try {
      const parsed = fromXml(xml)
      setNodes(parsed.nodes)
      setEdges(fixEdgeHandles(parsed.edges, parsed.nodes))
    } catch (e) {
      console.error('BpmnViewer: failed to parse XML', e)
    }
  }, [xml])

  const decoratedNodes = useMemo(
    () =>
      nodes.map(n => {
        const data = n.data as BpmnNodeData
        const status = elementStates?.get(n.id) ?? 'pending'
        return {
          ...n,
          type: nodeTypeFor(data.bpmnType),
          draggable: false,
          selectable: false,
          data: { ...data, runtimeStatus: status },
        }
      }),
    [nodes, elementStates],
  )

  const styledEdges = useMemo(
    () =>
      edges.map(e => {
        const sourceStatus = elementStates?.get(e.source)
        const targetStatus = elementStates?.get(e.target)
        // Priority: error > active (waiting at target) > traversed (source completed) > pending
        const isError     = targetStatus === 'error'
        const isActive    = !isError && targetStatus === 'active'
        const isTraversed = !isError && !isActive && sourceStatus === 'completed'
        const stroke = isError ? '#dc2626' : isActive ? '#2563eb' : isTraversed ? '#16a34a' : '#cbd5e1'
        return {
          ...e,
          type: 'smoothstep',
          pathOptions: { borderRadius: 8 },
          animated: isActive,
          style: { stroke, strokeWidth: isTraversed || isActive || isError ? 2 : 1.2 },
          markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: stroke },
        }
      }),
    [edges, elementStates],
  )

  return (
    <ReactFlow
      nodes={decoratedNodes}
      edges={styledEdges}
      nodeTypes={nodeTypes}
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable={false}
      panOnDrag
      zoomOnScroll
      fitView
      fitViewOptions={{ padding: 0.2 }}
      proOptions={{ hideAttribution: true }}
    >
      <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="var(--border-primary)" />
      <Controls showInteractive={false} />
      <MiniMap
        pannable
        zoomable
        nodeColor={(n: Node) => {
          const data = n.data as BpmnNodeData
          const status = data.runtimeStatus
          if (status === 'active') return '#2563eb'
          if (status === 'completed') return '#16a34a'
          if (status === 'error') return '#dc2626'
          return '#cbd5e1'
        }}
      />
    </ReactFlow>
  )
}
