# ADR-004: XML Parser — roxmltree

## Status
Proposed (pending Phase 0 spike)

## Context
BPMN process definitions are XML files. Need to parse them into an in-memory graph.
Parsing happens at deployment time only (not on every request), so raw throughput
is less critical than API ergonomics and correctness.

## Decision
Use **roxmltree 0.19.x** for initial implementation.

## Candidates Evaluated

| Library | Pros | Cons |
|---|---|---|
| roxmltree | Simple DOM API, namespace aware, read-only (safe), great ergonomics | Not streaming (loads whole document) |
| quick-xml | Very fast streaming parser, low memory | More verbose API, manual state management |
| minidom | Namespace-focused DOM | Small community, less documentation |

## Rationale
- BPMN files are typically small-medium (rarely > 1MB) — DOM approach is fine
- roxmltree's API is simple and safe (read-only document tree)
- Namespace handling is built in (critical for BPMN namespaced attributes)
- Parsed once at deployment and cached — performance not critical
- If files grow very large, can switch to quick-xml later

## Spike Validation
- [ ] Parse a real BPMN 2.0 file
- [ ] Extract: all flowElements, sequenceFlows, gateway conditions
- [ ] Extract namespaced attributes from the Conduit extension namespace: `conduit:topic`, `conduit:assignee`
- [ ] Handle nested elements: subprocesses, boundary events
- [ ] Parse time for 1000-element BPMN file (should be < 10ms)
