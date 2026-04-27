# Phase 11 — Signal Events

## Status
✅ Complete — all tests passing.

## Summary

Signals broadcast to ALL waiting instances simultaneously (unlike messages which target one).
A signal with no listeners returns success — signals are fire-and-forget.

## What Was Built

### Parser (`src/parser/mod.rs`)
Three new `FlowNodeKind` variants:
- `SignalStartEvent { signal_name: String }` — starts a new instance when the signal is broadcast
- `IntermediateSignalCatchEvent { signal_name: String }` — pauses the token until the signal arrives
- `BoundarySignalEvent { signal_name: String, attached_to: String, cancelling: bool }` — boundary event on a UserTask; interrupting or non-interrupting

### Engine (`src/engine/mod.rs`)
- `run_to_wait` handles `SignalStartEvent` (pass-through) and `IntermediateSignalCatchEvent` (creates `event_subscriptions` row, stops)
- `UserTask` setup creates a boundary execution + event_subscription for each `BoundarySignalEvent` attached to the task
- `complete_user_task` cancels boundary signal subscriptions and their execution rows when the task completes normally
- `broadcast_signal(signal_name, variables, org_id)` — Phase 1: drains all matching subscriptions (LIMIT 1 FOR UPDATE SKIP LOCKED per iteration, one tx per subscription); Phase 2: starts new instances for all matching `SignalStartEvent` definitions
- `EndEvent` arm checks active execution count before completing the instance (needed for non-interrupting boundary paths)

### API (`src/api/signals.rs`)
```
POST /api/v1/signals/broadcast
Body: { "org_id": "...", "signal_name": "...", "variables": [...] }
Response: 204 No Content
```

## Tests (`tests/signals_test.rs`)

| Test | What it verifies |
|---|---|
| `signal_catch_pauses_instance_and_creates_subscription` | Token stops at IntermediateSignalCatchEvent |
| `broadcast_signal_advances_waiting_instance_to_end` | Signal resumes and completes the instance |
| `broadcast_signal_passes_variables` | Variables from broadcast are written to the instance |
| `broadcast_signal_reaches_all_waiting_instances` | Broadcast hits every matching subscription |
| `broadcast_signal_with_no_listeners_succeeds` | No error when nobody is listening |
| `signal_start_event_creates_new_instance` | New instance created per matching SignalStartEvent |
| `boundary_signal_interrupting_cancels_task` | Interrupting boundary cancels host task, follows boundary path |
| `boundary_signal_non_interrupting_keeps_task` | Non-interrupting boundary spawns parallel path, task stays active |
| `normal_task_completion_cleans_up_signal_subscription` | Normal task completion removes boundary subscription + execution |

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (this phase + all previous)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [x] Phase marked complete in PLAN.md
