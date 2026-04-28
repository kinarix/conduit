import dagre from '@dagrejs/dagre';
import type { Node, Edge } from '@xyflow/react';
import { NODE_DIMENSIONS } from './bpmnTypes';
import type { BpmnNodeData } from './bpmnTypes';

export function applyAutoLayout(nodes: Node[], edges: Edge[]): Node[] {
  if (nodes.length === 0) return nodes;

  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: 'LR', nodesep: 50, ranksep: 80, marginx: 40, marginy: 40 });

  for (const node of nodes) {
    const d = node.data as BpmnNodeData;
    const dim = NODE_DIMENSIONS[d.bpmnType] ?? { width: 80, height: 40 };
    g.setNode(node.id, { width: dim.width, height: dim.height });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  return nodes.map(node => {
    const pos = g.node(node.id);
    const d = node.data as BpmnNodeData;
    const dim = NODE_DIMENSIONS[d.bpmnType] ?? { width: 80, height: 40 };
    return {
      ...node,
      position: {
        x: pos.x - dim.width / 2,
        y: pos.y - dim.height / 2,
      },
    };
  });
}
