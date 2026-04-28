import { useState } from 'react';
import type { BpmnElementType } from './bpmnTypes';
import { ELEMENT_COLORS } from './bpmnTypes';

interface PaletteItem {
  type: BpmnElementType;
  label: string;
  icon: (color: string) => React.ReactNode;
}

const PALETTE: PaletteItem[] = [
  {
    type: 'startEvent',
    label: 'Start Event',
    icon: (c) => (
      <svg width={20} height={20}>
        <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.startEvent.fill} stroke={c} strokeWidth={1.5} />
      </svg>
    ),
  },
  {
    type: 'endEvent',
    label: 'End Event',
    icon: (c) => (
      <svg width={20} height={20}>
        <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.endEvent.fill} stroke={c} strokeWidth={4} />
      </svg>
    ),
  },
  {
    type: 'userTask',
    label: 'Task',
    icon: (c) => (
      <svg width={20} height={14} viewBox="0 0 20 14">
        <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.userTask.fill} stroke={c} strokeWidth={1.5} />
        <circle cx={7} cy={6} r={2} stroke={c} strokeWidth={1} fill="none" />
        <path d="M4 12c0-1.66 1.34-3 3-3s3 1.34 3 3" stroke={c} strokeWidth={1} strokeLinecap="round" fill="none" />
      </svg>
    ),
  },
  {
    type: 'exclusiveGateway',
    label: 'Exclusive GW',
    icon: (c) => (
      <svg width={20} height={20}>
        <rect x={4} y={4} width={12} height={12} transform="rotate(45 10 10)" fill={ELEMENT_COLORS.exclusiveGateway.fill} stroke={c} strokeWidth={1.5} />
        <text x={10} y={14} textAnchor="middle" fontSize={10} fontWeight="bold" fill={c}>×</text>
      </svg>
    ),
  },
  {
    type: 'parallelGateway',
    label: 'Parallel GW',
    icon: (c) => (
      <svg width={20} height={20}>
        <rect x={4} y={4} width={12} height={12} transform="rotate(45 10 10)" fill={ELEMENT_COLORS.parallelGateway.fill} stroke={c} strokeWidth={1.5} />
        <text x={10} y={14} textAnchor="middle" fontSize={10} fontWeight="bold" fill={c}>+</text>
      </svg>
    ),
  },
  {
    type: 'inclusiveGateway',
    label: 'Inclusive GW',
    icon: (c) => (
      <svg width={20} height={20}>
        <rect x={4} y={4} width={12} height={12} transform="rotate(45 10 10)" fill={ELEMENT_COLORS.inclusiveGateway.fill} stroke={c} strokeWidth={1.5} />
        <circle cx={10} cy={10} r={3} stroke={c} strokeWidth={1.5} fill="none" />
      </svg>
    ),
  },
];

export default function BpmnPalette() {
  const [minimized, setMinimized] = useState(false);

  const onDragStart = (e: React.DragEvent, type: BpmnElementType) => {
    e.dataTransfer.setData('application/bpmn-type', type);
    e.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div style={{
      position: 'absolute',
      top: 12,
      left: 12,
      zIndex: 10,
      width: 164,
      background: 'rgba(255,255,255,0.97)',
      border: '1px solid #e2e8f0',
      borderRadius: 8,
      boxShadow: '0 2px 12px rgba(0,0,0,0.09)',
      overflow: 'hidden',
      pointerEvents: 'all',
    }}>
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '7px 10px',
          background: '#f8fafc',
          borderBottom: minimized ? 'none' : '1px solid #e2e8f0',
          cursor: 'default',
          userSelect: 'none',
        }}
      >
        <span style={{
          fontSize: 11,
          fontWeight: 600,
          color: '#64748b',
          textTransform: 'uppercase',
          letterSpacing: '0.05em',
        }}>
          Elements
        </span>
        <button
          onClick={() => setMinimized(m => !m)}
          title={minimized ? 'Expand' : 'Minimize'}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: '1px 3px',
            color: '#94a3b8',
            fontSize: 13,
            lineHeight: 1,
            borderRadius: 3,
            display: 'flex',
            alignItems: 'center',
          }}
          onMouseEnter={e => (e.currentTarget.style.color = '#475569')}
          onMouseLeave={e => (e.currentTarget.style.color = '#94a3b8')}
        >
          {minimized ? '▾' : '▴'}
        </button>
      </div>

      {/* Items */}
      {!minimized && (
        <div style={{ padding: '6px 8px', display: 'flex', flexDirection: 'column', gap: 4 }}>
          {PALETTE.map(({ type, label, icon }) => {
            const color = ELEMENT_COLORS[type].stroke;
            return (
              <div
                key={type}
                draggable
                onDragStart={(e) => onDragStart(e, type)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '6px 8px',
                  borderRadius: 6,
                  cursor: 'grab',
                  border: `1px solid ${color}22`,
                  borderLeft: `3px solid ${color}`,
                  background: '#ffffff',
                  userSelect: 'none',
                }}
                onMouseEnter={e => (e.currentTarget.style.background = ELEMENT_COLORS[type].fill)}
                onMouseLeave={e => (e.currentTarget.style.background = '#ffffff')}
              >
                <div style={{ flexShrink: 0 }}>{icon(color)}</div>
                <span style={{ fontSize: 12, color: '#0f172a' }}>{label}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
