# BPMN Validation Rules

**This document is the single source of truth for all BPMN element rules.**  
Consult it before adding or changing any element type, wiring logic, or validation code.  
When you add a new element type, update every table in this doc first, then update the code.

---

## Severity Levels

| Level | Meaning | Enforced in |
|---|---|---|
| **Error** | Process cannot deploy or will not execute correctly | `src/parser/mod.rs` `validate()` + UI (red badge) |
| **Warning** | Process deploys but will likely stall or misbehave | `ui/…/BpmnProperties.tsx` `computeNodeWarnings` (yellow badge) |

---

## Process-Level Invariants

| Rule | Severity | Status |
|---|---|---|
| At least 1 start event (any type) | Error | ✓ backend |
| At most 1 plain `startEvent` (message/timer/signal starts are additive) | Error | ✓ backend |
| At least 1 `endEvent` | Error | ✓ backend |
| All sequence flow `sourceRef`/`targetRef` must exist in the process | Error | ✓ backend |
| `ExclusiveGateway` default flow ref must exist | Error | ✓ backend |
| `BoundaryEvent.attachedToRef` must exist in the process | Error | ✓ backend |
| Start events must have 0 incoming sequence flows | Error | ✓ backend + UI |
| End events must have 0 outgoing sequence flows | Error | ✓ backend + UI |
| `ExclusiveGateway` used as a split must have ≥ 2 outgoing flows | Warning | ✓ UI |
| `InclusiveGateway` used as a split must have ≥ 2 outgoing flows | Warning | ✓ UI |

---

## Source → Target Connection Matrix

Valid **sequence flow** connections. Rows = source element, columns = target category.  
`✓` = allowed, `✗` = never valid.

| Source \ Target | Start event | Task | Gateway | Intermediate event | End event |
|---|---|---|---|---|---|
| startEvent / messageStartEvent / timerStartEvent | ✗ | ✓ | ✓ | ✓ | ✓ |
| endEvent | ✗ | ✗ | ✗ | ✗ | ✗ |
| userTask / serviceTask / businessRuleTask / subProcess / sendTask / receiveTask | ✗ | ✓ | ✓ | ✓ | ✓ |
| exclusiveGateway / parallelGateway / inclusiveGateway | ✗ | ✓ | ✓ | ✓ | ✓ |
| intermediateCatchTimerEvent / intermediateCatchMessageEvent / intermediateCatchSignalEvent | ✗ | ✓ | ✓ | ✓ | ✓ |
| boundaryTimerEvent / boundarySignalEvent / boundaryErrorEvent (outgoing sequence flow) | ✗ | ✓ | ✓ | ✓ | ✓ |

### Attachment Edges (separate from sequence flows)

Attachment edges (`data.kind === 'attachment'`) wire a boundary event to its host. They are **not** sequence flows and are not serialised as `<sequenceFlow>` in XML; they become `attachedToRef` on the `<boundaryEvent>` element.

| Source (boundary event) | Valid host targets |
|---|---|
| boundaryTimerEvent / boundarySignalEvent / boundaryErrorEvent | userTask, serviceTask, businessRuleTask, subProcess, sendTask, receiveTask |

Boundary events **cannot** attach to: gateways, event nodes, endEvent.

---

## Cardinality Rules Per Element

| Type | Incoming seq flows | Outgoing seq flows | As attachment source | As attachment target |
|---|---|---|---|---|
| startEvent | **0** | ≥ 1 | 0 | 0 |
| messageStartEvent | **0** | ≥ 1 | 0 | 0 |
| timerStartEvent | **0** | ≥ 1 | 0 | 0 |
| endEvent | ≥ 1 | **0** | 0 | 0 |
| userTask | ≥ 1 | ≥ 1 | 0 | unlimited |
| serviceTask | ≥ 1 | ≥ 1 | 0 | unlimited |
| businessRuleTask | ≥ 1 | ≥ 1 | 0 | unlimited |
| subProcess | ≥ 1 | ≥ 1 | 0 | unlimited |
| sendTask | ≥ 1 | ≥ 1 | 0 | unlimited |
| receiveTask | ≥ 1 | ≥ 1 | 0 | unlimited |
| exclusiveGateway | ≥ 1 | ≥ 1 (split: ≥ 2) | 0 | 0 |
| parallelGateway | ≥ 1 | ≥ 1 (fork: ≥ 2) | 0 | 0 |
| inclusiveGateway | ≥ 1 | ≥ 1 (split: ≥ 2) | 0 | 0 |
| intermediateCatchTimerEvent | ≥ 1 | ≥ 1 | 0 | 0 |
| intermediateCatchMessageEvent | ≥ 1 | ≥ 1 | 0 | 0 |
| intermediateCatchSignalEvent | ≥ 1 | ≥ 1 | 0 | 0 |
| boundaryTimerEvent | **0** | ≥ 1 | exactly **1** | 0 |
| boundarySignalEvent | **0** | ≥ 1 | exactly **1** | 0 |
| boundaryErrorEvent | **0** | ≥ 1 | exactly **1** | 0 |

---

## Required Configuration Per Type

Missing a required field is a **Warning** (shown in the properties panel).

| Type | Required field(s) | Notes |
|---|---|---|
| timerStartEvent | `timerExpression` | ISO 8601 cycle/duration/date |
| messageStartEvent | `messageName` | Must match `message_name` in POST /messages |
| intermediateCatchTimerEvent | `timerExpression` | ISO 8601 duration or date |
| intermediateCatchMessageEvent | `messageName` | |
| intermediateCatchSignalEvent | `signalName` | Must match signal broadcast name |
| boundaryTimerEvent | `timerExpression` | ISO 8601 duration (e.g. `PT30M`) |
| boundarySignalEvent | `signalName` | |
| boundaryErrorEvent | `errorCode` optional — blank = catch-all | Not a hard requirement |
| businessRuleTask | `decisionRef` | Must match deployed DMN decision id |
| sendTask | `messageName` | |
| receiveTask | `messageName` | |
| serviceTask | `topic` OR `url` (at least one) | topic = worker poll, url = HTTP push |

---

## Future-Proofing: Adding a New Element Type

When adding any new element type, update **all** of the following before touching code:

1. Add a row to the **Cardinality Rules** table.
2. Add a row to the **Required Configuration** table (or note "no required fields").
3. Update the **Source → Target Connection Matrix** — add a row if the new type can be a source, add a column category note if it's a new target category.
4. Add a case to `computeNodeWarnings` in `ui/src/components/bpmn/BpmnProperties.tsx`.
5. Add a check to `validate()` in `src/parser/mod.rs` if the rule is error-level.
6. Update `ELEMENT_COLORS`, `NODE_DIMENSIONS`, `ELEMENT_LABELS` in `bpmnTypes.ts`.
