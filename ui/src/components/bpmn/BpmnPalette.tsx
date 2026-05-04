import { useState, useRef } from 'react';
import type { BpmnElementType } from './bpmnTypes';
import { ELEMENT_COLORS } from './bpmnTypes';

interface PaletteItem {
  type: BpmnElementType;
  label: string;
  icon: (color: string) => React.ReactNode;
}

interface PaletteGroup {
  heading: string;
  items: PaletteItem[];
}

const PALETTE: PaletteGroup[] = [
  {
    heading: 'Events',
    items: [
      {
        type: 'startEvent',
        label: 'Start',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.startEvent.fill} stroke={c} strokeWidth={1.5} />
          </svg>
        ),
      },
      {
        type: 'messageStartEvent',
        label: 'Msg Start',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.messageStartEvent.fill} stroke={c} strokeWidth={1.5} />
            <rect x={5} y={6.5} width={10} height={7} rx={0.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <path d="M5 6.5l5 4.5 5-4.5" fill="none" stroke={c} strokeWidth={1.1} strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'timerStartEvent',
        label: 'Timer Start',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.timerStartEvent.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={10} cy={10} r={5} fill="none" stroke={c} strokeWidth={1.1}/>
            <path d="M10 7v3l2 1.5" stroke={c} strokeWidth={1.1} strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'endEvent',
        label: 'End',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.endEvent.fill} stroke={c} strokeWidth={3.5} />
          </svg>
        ),
      },
      {
        type: 'intermediateCatchTimerEvent',
        label: 'Timer Catch',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.intermediateCatchTimerEvent.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={10} cy={10} r={5.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <circle cx={10} cy={10} r={3.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <path d="M10 8v2l1.2 1" stroke={c} strokeWidth={1} strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'intermediateCatchMessageEvent',
        label: 'Msg Catch',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.intermediateCatchMessageEvent.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={10} cy={10} r={5.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <rect x={5.5} y={7.5} width={9} height={6} rx={0.5} fill="none" stroke={c} strokeWidth={1}/>
            <path d="M5.5 7.5l4.5 3.5 4.5-3.5" fill="none" stroke={c} strokeWidth={1} strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'intermediateCatchSignalEvent',
        label: 'Signal Catch',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.intermediateCatchSignalEvent.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={10} cy={10} r={5.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <path d="M10 6l3 6H7L10 6z" fill="none" stroke={c} strokeWidth={1} strokeLinejoin="round"/>
          </svg>
        ),
      },
    ],
  },
  {
    heading: 'Tasks',
    items: [
      {
        type: 'userTask',
        label: 'User Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.userTask.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={7} cy={6} r={2} stroke={c} strokeWidth={1} fill="none" />
            <path d="M4 12c0-1.66 1.34-3 3-3s3 1.34 3 3" stroke={c} strokeWidth={1} strokeLinecap="round" fill="none" />
          </svg>
        ),
      },
      {
        type: 'serviceTask',
        label: 'Service Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.serviceTask.fill} stroke={c} strokeWidth={1.5} />
            <circle cx={10} cy={7} r={2.5} fill="none" stroke={c} strokeWidth={1.1}/>
            <circle cx={10} cy={7} r={1} fill={c}/>
            <path d="M10 3v1.2M10 10.8V12M6.5 5.2l.85.85M13.65 8.95l.85.85M4 7h1.2M14.8 7H16M6.5 8.95l.85-.85M13.65 5.2l.85-.85" stroke={c} strokeWidth={1} strokeLinecap="round"/>
          </svg>
        ),
      },
      {
        type: 'scriptTask',
        label: 'Script Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.scriptTask.fill} stroke={c} strokeWidth={1.5} />
            <path d="M6 5l-2.5 2L6 9" stroke={c} strokeWidth={1.1} strokeLinecap="round" strokeLinejoin="round" fill="none"/>
            <path d="M14 5l2.5 2L14 9" stroke={c} strokeWidth={1.1} strokeLinecap="round" strokeLinejoin="round" fill="none"/>
            <line x1={11.5} y1={3.5} x2={8.5} y2={10.5} stroke={c} strokeWidth={1.1} strokeLinecap="round"/>
          </svg>
        ),
      },
      {
        type: 'businessRuleTask',
        label: 'Rule Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.businessRuleTask.fill} stroke={c} strokeWidth={1.5} />
            <rect x={3} y={3} width={14} height={3} rx={0.5} fill="none" stroke={c} strokeWidth={1}/>
            <line x1={3} y1={8} x2={17} y2={8} stroke={c} strokeWidth={1}/>
            <line x1={3} y1={10.5} x2={17} y2={10.5} stroke={c} strokeWidth={1}/>
            <line x1={9} y1={7} x2={9} y2={12} stroke={c} strokeWidth={1}/>
          </svg>
        ),
      },
      {
        type: 'subProcess',
        label: 'Sub Process',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.subProcess.fill} stroke={c} strokeWidth={1.5} />
            <rect x={7.5} y={8.5} width={5} height={3.5} rx={0.5} fill="none" stroke={c} strokeWidth={1}/>
            <line x1={10} y1={9.5} x2={10} y2={11.5} stroke={c} strokeWidth={1}/>
            <line x1={8.8} y1={10.5} x2={11.2} y2={10.5} stroke={c} strokeWidth={1}/>
          </svg>
        ),
      },
      {
        type: 'sendTask',
        label: 'Send Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.sendTask.fill} stroke={c} strokeWidth={1.5} />
            <rect x={4} y={3.5} width={9} height={6} rx={0.5} fill={c} stroke={c} strokeWidth={1}/>
            <path d="M4 3.5l4.5 3.5 4.5-3.5" fill="none" stroke={ELEMENT_COLORS.sendTask.fill} strokeWidth={1} strokeLinejoin="round"/>
            <path d="M15 5.5l3-2v7l-3-2" fill={c} stroke="none"/>
          </svg>
        ),
      },
      {
        type: 'receiveTask',
        label: 'Receive Task',
        icon: (c) => (
          <svg width={20} height={14} viewBox="0 0 20 14">
            <rect x={1} y={1} width={18} height={12} rx={2} fill={ELEMENT_COLORS.receiveTask.fill} stroke={c} strokeWidth={1.5} />
            <rect x={4} y={3.5} width={12} height={7} rx={0.5} fill="none" stroke={c} strokeWidth={1}/>
            <path d="M4 3.5l6 4.5 6-4.5" fill="none" stroke={c} strokeWidth={1} strokeLinejoin="round"/>
          </svg>
        ),
      },
    ],
  },
  {
    heading: 'Gateways',
    items: [
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
    ],
  },
  {
    heading: 'Boundary',
    items: [
      {
        type: 'boundaryTimerEvent',
        label: 'Timer Boundary',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.boundaryTimerEvent.fill} stroke={c} strokeWidth={1.5} strokeDasharray="3 2" />
            <circle cx={10} cy={10} r={5} fill="none" stroke={c} strokeWidth={1.1}/>
            <path d="M10 7v3l2 1.5" stroke={c} strokeWidth={1.1} strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'boundarySignalEvent',
        label: 'Signal Boundary',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.boundarySignalEvent.fill} stroke={c} strokeWidth={1.5} strokeDasharray="3 2" />
            <path d="M10 5.5l3.5 7H6.5L10 5.5z" fill="none" stroke={c} strokeWidth={1.1} strokeLinejoin="round"/>
          </svg>
        ),
      },
      {
        type: 'boundaryErrorEvent',
        label: 'Error Boundary',
        icon: (c) => (
          <svg width={20} height={20}>
            <circle cx={10} cy={10} r={8} fill={ELEMENT_COLORS.boundaryErrorEvent.fill} stroke={c} strokeWidth={1.5} strokeDasharray="3 2" />
            <path d="M12 5.5l-3 4h2.5l-3 5" stroke={c} strokeWidth={1.4} strokeLinecap="round" strokeLinejoin="round" fill="none"/>
          </svg>
        ),
      },
    ],
  },
];

function Twisty({ open, size = 8, minimize = false }: { open: boolean; size?: number; minimize?: boolean }) {
  // minimize mode: up when expanded (click to collapse), down when collapsed (click to expand)
  // default mode:  down when open, right-pointing when closed
  const angle = minimize
    ? (open ? 180 : 0)
    : (open ? 0 : -90);
  return (
    <svg
      width={size} height={size}
      viewBox="0 0 8 8"
      style={{
        transform: `rotate(${angle}deg)`,
        transition: 'transform 0.15s ease',
        flexShrink: 0,
      }}
    >
      <polygon points="1,2 7,2 4,6" fill="currentColor" />
    </svg>
  );
}

export default function BpmnPalette() {
  const [minimized, setMinimized] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [position, setPosition] = useState({ x: 12, y: 12 });
  const positionRef = useRef({ x: 12, y: 12 });

  const toggleSection = (heading: string) => {
    setCollapsed(prev => {
      const next = new Set(prev);
      next.has(heading) ? next.delete(heading) : next.add(heading);
      return next;
    });
  };

  const onDragStart = (e: React.DragEvent, type: BpmnElementType) => {
    e.dataTransfer.setData('application/bpmn-type', type);
    e.dataTransfer.effectAllowed = 'move';
  };

  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('button')) return;
    e.preventDefault();

    const startX = e.clientX - positionRef.current.x;
    const startY = e.clientY - positionRef.current.y;
    document.body.style.cursor = 'grabbing';

    const onMouseMove = (ev: MouseEvent) => {
      const next = { x: ev.clientX - startX, y: ev.clientY - startY };
      positionRef.current = next;
      setPosition(next);
    };

    const onMouseUp = () => {
      document.body.style.cursor = '';
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
    };

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  };

  return (
    <div style={{
      position: 'absolute',
      top: position.y,
      left: position.x,
      zIndex: 10,
      width: 164,
      background: 'rgba(255,255,255,0.97)',
      border: '1px solid #e2e8f0',
      borderRadius: 8,
      boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
      overflow: 'hidden',
      pointerEvents: 'all',
    }}>
      {/* Drag handle header */}
      <div
        onMouseDown={handleHeaderMouseDown}
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '7px 10px',
          background: '#f8fafc',
          borderBottom: minimized ? 'none' : '1px solid #e2e8f0',
          cursor: 'grab',
          userSelect: 'none',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {/* Drag grip dots */}
          <svg width={10} height={14} style={{ opacity: 0.35 }}>
            {[0, 4, 8].map(y => (
              [0, 4].map(x => (
                <circle key={`${x}-${y}`} cx={x + 1} cy={y + 3} r={1} fill="#64748b" />
              ))
            ))}
          </svg>
          <span style={{
            fontSize: 11,
            fontWeight: 600,
            color: '#64748b',
            textTransform: 'uppercase',
            letterSpacing: '0.05em',
          }}>
            Elements
          </span>
        </div>
        <button
          onClick={() => setMinimized(m => !m)}
          title={minimized ? 'Expand' : 'Minimize'}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: '1px 3px',
            color: '#94a3b8',
            lineHeight: 1,
            borderRadius: 3,
            display: 'flex',
            alignItems: 'center',
          }}
          onMouseEnter={e => (e.currentTarget.style.color = '#475569')}
          onMouseLeave={e => (e.currentTarget.style.color = '#94a3b8')}
        >
          {minimized ? (
            /* Restore: small window square */
            <svg width={12} height={12} viewBox="0 0 12 12">
              <rect x={1.5} y={1.5} width={9} height={9} rx={1.5} fill="none" stroke="currentColor" strokeWidth={1.5} />
              <line x1={1.5} y1={4} x2={10.5} y2={4} stroke="currentColor" strokeWidth={1.5} />
            </svg>
          ) : (
            /* Minimize: dash */
            <svg width={12} height={12} viewBox="0 0 12 12">
              <line x1={2} y1={9} x2={10} y2={9} stroke="currentColor" strokeWidth={2} strokeLinecap="round" />
            </svg>
          )}
        </button>
      </div>

      {!minimized && (
        <div style={{ padding: '6px 8px', display: 'flex', flexDirection: 'column', gap: 0 }}>
          {PALETTE.map(({ heading, items }) => {
            const isCollapsed = collapsed.has(heading);
            return (
              <div key={heading}>
                {/* Section heading — click to toggle */}
                <div
                  onClick={() => toggleSection(heading)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 5,
                    padding: '5px 4px 3px',
                    cursor: 'pointer',
                    userSelect: 'none',
                    color: '#94a3b8',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.color = '#475569')}
                  onMouseLeave={e => (e.currentTarget.style.color = '#94a3b8')}
                >
                  <Twisty open={!isCollapsed} size={10} />
                  <span style={{
                    fontSize: 9,
                    fontWeight: 700,
                    textTransform: 'uppercase',
                    letterSpacing: '0.08em',
                  }}>
                    {heading}
                  </span>
                </div>

                {!isCollapsed && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 3, marginBottom: 4 }}>
                    {items.map(({ type, label, icon }) => {
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
                            padding: '5px 8px',
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
                          <span style={{ fontSize: 11, color: '#0f172a' }}>{label}</span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
