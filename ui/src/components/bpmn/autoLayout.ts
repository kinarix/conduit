import dagre from '@dagrejs/dagre';
import type { Node, Edge } from '@xyflow/react';
import { NODE_DIMENSIONS } from './bpmnTypes';
import type { BpmnNodeData, BpmnEdgeData } from './bpmnTypes';

const BOUNDARY_DIM = { width: 22, height: 22 };
const BOUNDARY_GAP = 12; // clear gap between host edge and boundary event

interface BoundaryInfo {
  nodeId: string;
  side: 'top' | 'bottom';
}

const GATEWAY_TYPES = new Set([
  'exclusiveGateway', 'inclusiveGateway', 'parallelGateway',
]);

const SOURCE_HANDLE_PRIORITY = [
  'right-source',
  'bottom-source',
  'top-source',
  'left-source',
] as const;

const TARGET_HANDLE_FOR_SOURCE: Record<string, string> = {
  'right-source':  'left-target',
  'bottom-source': 'top-target',
  'top-source':    'bottom-target',
  'left-source':   'right-target',
};

/**
 * Spread a gateway's outgoing flows across its 4 source handles in priority
 * order (right → bottom → top → left). Each handle hosts at most one flow
 * until all 4 are used; further flows pile back on the highest-priority
 * available handle. Mutates `edges` in place by setting sourceHandle/targetHandle.
 */
export function spreadGatewayHandles(nodes: Node[], edges: Edge[]): Edge[] {
  const nodeById = new Map(nodes.map(n => [n.id, n]));
  const gatewayIds = new Set(
    nodes
      .filter(n => GATEWAY_TYPES.has((n.data as BpmnNodeData).bpmnType))
      .map(n => n.id),
  );

  const outgoingByGateway = new Map<string, Edge[]>();
  for (const edge of edges) {
    if ((edge.data as BpmnEdgeData | undefined)?.kind === 'attachment') continue;
    if (!gatewayIds.has(edge.source)) continue;
    const list = outgoingByGateway.get(edge.source) ?? [];
    list.push(edge);
    outgoingByGateway.set(edge.source, list);
  }

  return edges.map(edge => {
    if ((edge.data as BpmnEdgeData | undefined)?.kind === 'attachment') return edge;
    if (!gatewayIds.has(edge.source)) return edge;
    const siblings = outgoingByGateway.get(edge.source) ?? [];
    const idx = siblings.indexOf(edge);
    if (idx < 0) return edge;
    const sourceHandle = SOURCE_HANDLE_PRIORITY[idx % SOURCE_HANDLE_PRIORITY.length];
    const targetHandle = TARGET_HANDLE_FOR_SOURCE[sourceHandle];
    // Only override the target handle when the target node is a non-gateway
    // BPMN node we control; otherwise leave whatever was there.
    const targetNode = nodeById.get(edge.target);
    const useTargetHandle = targetNode ? targetHandle : edge.targetHandle;
    return { ...edge, sourceHandle, targetHandle: useTargetHandle };
  });
}

/**
 * Recompute sourceHandle / targetHandle for every non-attachment edge based
 * on the actual direction vector between node centers. Call this after any
 * operation that repositions nodes (auto-layout, paste, etc.).
 */
export function recomputeAllEdgeHandles(nodes: Node[], edges: Edge[]): Edge[] {
  const nodeById = new Map(nodes.map(n => [n.id, n]));

  return edges.map(edge => {
    if ((edge.data as BpmnEdgeData | undefined)?.kind === 'attachment') return edge;

    const srcNode = nodeById.get(edge.source);
    const tgtNode = nodeById.get(edge.target);
    if (!srcNode || !tgtNode) return edge;

    const srcDim = NODE_DIMENSIONS[(srcNode.data as BpmnNodeData).bpmnType] ?? { width: 80, height: 40 };
    const tgtDim = NODE_DIMENSIONS[(tgtNode.data as BpmnNodeData).bpmnType] ?? { width: 80, height: 40 };
    const srcCX = srcNode.position.x + srcDim.width / 2;
    const srcCY = srcNode.position.y + srcDim.height / 2;
    const tgtCX = tgtNode.position.x + tgtDim.width / 2;
    const tgtCY = tgtNode.position.y + tgtDim.height / 2;
    const dx = tgtCX - srcCX;
    const dy = tgtCY - srcCY;

    let sourceHandle: string;
    let targetHandle: string;
    if (Math.abs(dx) >= Math.abs(dy)) {
      sourceHandle = dx >= 0 ? 'right-source' : 'left-source';
      targetHandle = dx >= 0 ? 'left-target' : 'right-target';
    } else {
      sourceHandle = dy >= 0 ? 'bottom-source' : 'top-source';
      targetHandle = dy >= 0 ? 'top-target' : 'bottom-target';
    }
    return { ...edge, sourceHandle, targetHandle };
  });
}

export function applyAutoLayout(nodes: Node[], edges: Edge[]): Node[] {
  if (nodes.length === 0) return nodes;

  const attachmentEdges = edges.filter(e => (e.data as BpmnEdgeData | undefined)?.kind === 'attachment');
  const sequenceEdges   = edges.filter(e => (e.data as BpmnEdgeData | undefined)?.kind !== 'attachment');

  // Build maps: boundary node id ↔ host node id, plus side from targetHandle
  // Attachment edge: source = boundary event, target = host task
  const boundaryToHost   = new Map<string, string>();
  const hostToBoundaries = new Map<string, BoundaryInfo[]>();

  for (const ae of attachmentEdges) {
    boundaryToHost.set(ae.source, ae.target);
    const side: 'top' | 'bottom' =
      ae.targetHandle === 'top-target' || ae.targetHandle === 'target-top'
        ? 'top'
        : 'bottom';
    const list = hostToBoundaries.get(ae.target) ?? [];
    list.push({ nodeId: ae.source, side });
    hostToBoundaries.set(ae.target, list);
  }

  const boundaryNodeIds = new Set(boundaryToHost.keys());

  // Dagre graph — boundary nodes excluded so they don't consume an extra rank
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: 'LR', nodesep: 60, ranksep: 100, marginx: 40, marginy: 40, ranker: 'network-simplex' });

  for (const node of nodes) {
    if (boundaryNodeIds.has(node.id)) continue;
    const d = node.data as BpmnNodeData;
    const dim = NODE_DIMENSIONS[d.bpmnType] ?? { width: 80, height: 40 };
    g.setNode(node.id, { width: dim.width, height: dim.height });
  }

  // Sequence edges: substitute any boundary endpoint with its host so that
  // exception-path downstream nodes still get ranked correctly
  for (const edge of sequenceEdges) {
    const src = boundaryToHost.get(edge.source) ?? edge.source;
    const tgt = boundaryToHost.get(edge.target) ?? edge.target;
    if (src !== tgt && g.hasNode(src) && g.hasNode(tgt)) {
      g.setEdge(src, tgt);
    }
  }

  dagre.layout(g);

  // Preserve the user's original relative vertical ordering within each rank.
  // Dagre assigns nodes to ranks (columns in LR mode) but freely reorders them
  // vertically, which can swap nodes and introduce crossings in diagrams that
  // were already clean. We keep dagre's Y-slot values (the vertical spacing it
  // calculated) but re-assign them in the order the user originally arranged
  // the nodes — lowest original Y keeps the lowest dagre Y slot, etc.
  const rankGroups = new Map<number, string[]>();
  for (const node of nodes) {
    if (boundaryNodeIds.has(node.id)) continue;
    const pos = g.node(node.id);
    if (!pos) continue;
    const list = rankGroups.get(pos.x) ?? [];
    list.push(node.id);
    rankGroups.set(pos.x, list);
  }

  const yOverride = new Map<string, number>(); // nodeId → corrected dagre-center Y
  for (const nodeIds of rankGroups.values()) {
    if (nodeIds.length <= 1) continue;

    // dagre's Y slots for this rank, sorted ascending
    const dagreSlots = nodeIds.map(id => g.node(id).y).sort((a, b) => a - b);

    // nodes sorted by their original center-Y (top-to-bottom in the user's layout)
    const byOriginalY = nodeIds
      .map(id => {
        const node = nodes.find(n => n.id === id)!;
        const dim = NODE_DIMENSIONS[(node.data as BpmnNodeData).bpmnType] ?? { width: 80, height: 40 };
        return { id, origCY: node.position.y + dim.height / 2 };
      })
      .sort((a, b) => a.origCY - b.origCY);

    // assign slots in original order: topmost original node → smallest Y slot
    byOriginalY.forEach(({ id }, i) => yOverride.set(id, dagreSlots[i]));
  }

  // Position boundary events relative to their host after main layout is done.
  // They sit clearly above or below the host (no overlap), centered horizontally
  // on the host, staggered when multiple events share the same side.
  const boundaryPositions = new Map<string, { x: number; y: number }>();

  for (const [hostId, bInfos] of hostToBoundaries) {
    const hostDagre = g.node(hostId);
    if (!hostDagre) continue;

    const hostNode = nodes.find(n => n.id === hostId);
    const hostDim  = NODE_DIMENSIONS[(hostNode?.data as BpmnNodeData)?.bpmnType] ?? { width: 80, height: 40 };

    const topGroup    = bInfos.filter(b => b.side === 'top');
    const bottomGroup = bInfos.filter(b => b.side === 'bottom');
    const xStep = 28;

    // hostDagre.{x,y} are Dagre CENTER coordinates
    const hostCX = hostDagre.x;
    const hostCY = hostDagre.y;

    const placeGroup = (group: BoundaryInfo[], aboveHost: boolean) => {
      group.forEach(({ nodeId }, idx) => {
        const count = group.length;
        const xOffset = count === 1 ? 0 : (idx - (count - 1) / 2) * xStep;

        // top-left x: center boundary on the host's center-x (± stagger)
        const bLeft = hostCX + xOffset - BOUNDARY_DIM.width / 2;

        // top-left y: fully outside the host with a clear gap
        const bTop = aboveHost
          ? hostCY - hostDim.height / 2 - BOUNDARY_GAP - BOUNDARY_DIM.height  // above
          : hostCY + hostDim.height / 2 + BOUNDARY_GAP;                        // below

        boundaryPositions.set(nodeId, { x: bLeft, y: bTop });
      });
    };

    placeGroup(topGroup,    true);
    placeGroup(bottomGroup, false);
  }

  return nodes.map(node => {
    if (boundaryNodeIds.has(node.id)) {
      const pos = boundaryPositions.get(node.id);
      return pos ? { ...node, position: pos } : node;
    }

    const pos = g.node(node.id);
    if (!pos) return node;
    const d   = node.data as BpmnNodeData;
    const dim = NODE_DIMENSIONS[d.bpmnType] ?? { width: 80, height: 40 };
    const centerY = yOverride.get(node.id) ?? pos.y;
    return {
      ...node,
      position: {
        x: pos.x - dim.width / 2,
        y: centerY - dim.height / 2,
      },
    };
  });
}
