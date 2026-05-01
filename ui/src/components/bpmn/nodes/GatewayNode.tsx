import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS, RUNTIME_STATUS_COLOR } from '../bpmnTypes';
import { useIsConnecting } from '../connectingContext';
import { useNodeWarnings } from '../warningsContext';

const MARKER: Record<string, string> = {
  exclusiveGateway: '×',
  parallelGateway:  '+',
  inclusiveGateway: '○',
};

export default function GatewayNode({ id, data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const colors = ELEMENT_COLORS[d.bpmnType];
  const statusColor = d.runtimeStatus ? RUNTIME_STATUS_COLOR[d.runtimeStatus] : null;
  const stroke = selected ? '#6366f1' : (statusColor ?? colors.stroke);
  const isConnecting = useIsConnecting();
  const handleStyle = (base?: React.CSSProperties) => ({
    ...base,
    background: colors.stroke,
    opacity: selected || isConnecting ? 1 : 0,
  });
  const warnings = useNodeWarnings(id);
  const opacity = d.runtimeStatus === 'pending' ? 0.55 : 1;
  const dropShadow = d.runtimeStatus === 'active'
    ? `drop-shadow(0 0 4px ${statusColor!}88)`
    : undefined;

  return (
    <div
      style={{ position: 'relative', width: 30, height: 30, opacity, filter: dropShadow }}
      title={warnings.length > 0 ? warnings.join('\n') : undefined}
    >
      {warnings.length > 0 && (
        <div style={{
          position: 'absolute', top: -6, right: -6,
          width: 16, height: 16, borderRadius: '50%',
          background: '#f59e0b', border: '1.5px solid #fff',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 9, fontWeight: 700, color: '#fff',
          zIndex: 10, pointerEvents: 'none', lineHeight: 1,
        }}>
          {warnings.length}
        </div>
      )}
      <Handle id="left-target"   type="target" position={Position.Left}   style={handleStyle()} />
      <Handle id="left-source"   type="source" position={Position.Left}   style={handleStyle()} />
      <Handle id="top-target"    type="target" position={Position.Top}    style={handleStyle()} />
      <Handle id="top-source"    type="source" position={Position.Top}    style={handleStyle()} />

      <svg width={30} height={30}>
        <rect
          x={5} y={5} width={20} height={20}
          transform="rotate(45 15 15)"
          fill={colors.fill}
          stroke={stroke}
          strokeWidth={statusColor ? 2.5 : 1.5}
        />
        <text
          x={15} y={19}
          textAnchor="middle"
          fontSize={10}
          fontWeight="bold"
          fill={stroke}
        >
          {MARKER[d.bpmnType] ?? '?'}
        </text>
      </svg>

      {d.label && (
        <div style={{
          position: 'absolute',
          top: 34,
          left: '50%',
          transform: 'translateX(-50%)',
          whiteSpace: 'nowrap',
          fontSize: 10,
          color: '#0f172a',
          pointerEvents: 'none',
        }}>
          {d.label}
        </div>
      )}

      <Handle id="right-target"  type="target" position={Position.Right}  style={handleStyle()} />
      <Handle id="right-source"  type="source" position={Position.Right}  style={handleStyle()} />
      <Handle id="bottom-target" type="target" position={Position.Bottom} style={handleStyle()} />
      <Handle id="bottom-source" type="source" position={Position.Bottom} style={handleStyle()} />
    </div>
  );
}
