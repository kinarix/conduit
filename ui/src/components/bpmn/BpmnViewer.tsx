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
  /** Map<element_id, status>. Nodes without an entry default to 'pending'. */
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
  if (t === 'userTask' || t === 'serviceTask' || t === 'scriptTask' || t === 'businessRuleTask' ||
      t === 'subProcess' || t === 'sendTask' || t === 'receiveTask') return 'bpmnTask'
  return 'bpmnGateway'
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
      setEdges(parsed.edges)
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
        // Highlight an edge as "active" if its source node is completed and target is active.
        const sourceStatus = elementStates?.get(e.source)
        const targetStatus = elementStates?.get(e.target)
        const traversed = sourceStatus === 'completed'
        const active = targetStatus === 'active'
        const stroke = active ? '#2563eb' : traversed ? '#16a34a' : '#cbd5e1'
        return {
          ...e,
          style: { stroke, strokeWidth: traversed || active ? 2 : 1.2 },
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
