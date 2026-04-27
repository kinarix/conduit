# Phase 12 — Embedded Subprocess

## Status
✅ Phase 12a complete. 12b (Boundary Message Event) and 12c (Event Subprocess) deferred.

## Summary

An embedded subprocess is a container element that groups a sub-flow.
The token enters the subprocess, executes the entire inner flow, then exits via the subprocess's outgoing sequence flow — all within the same process instance.

Variables are instance-scoped (already the case in the engine — reads use `instance_id`), so the subprocess sees and mutates the same variable pool as its parent.

## What to Build

### Phase 12a — Embedded Subprocess (this phase)

#### Parser (`src/parser/mod.rs`)

New `FlowNodeKind` variant:

```rust
SubProcess {
    sub_graph: ProcessGraph,
}
```

The parser, when it encounters a `subProcess` element:
1. Recursively parses child elements (`startEvent`, `endEvent`, `userTask`, `serviceTask`, `sequenceFlow`, `exclusiveGateway`, `parallelGateway`, etc.) into a nested `ProcessGraph`
2. Stores the nested graph inside the `SubProcess` variant
3. Registers the `subProcess` node in the parent graph as usual (so outgoing sequence flows wire up correctly)

The recursive parse can share the same `parse_node` / `parse_flow` helpers since the inner grammar is identical.

#### Engine (`src/engine/mod.rs`)

**New private struct** (for passing context into subprocess calls):

```rust
struct SubprocessCtx<'a> {
    outer_graph: &'a ProcessGraph,
    subprocess_element_id: &'a str,
}
```

**`run_to_wait` signature change**:

```rust
async fn run_to_wait(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    instance_id: Uuid,
    start_element_id: &str,
    graph: &ProcessGraph,
    scope: Option<Uuid>,
    subprocess_ctx: Option<SubprocessCtx<'_>>,
) -> Result<()>
```

All existing call sites pass `subprocess_ctx: None`. The `SubProcess` arm passes `Some(...)`.

**New arm in `run_to_wait` match**:

```
FlowNodeKind::SubProcess { sub_graph } => {
    // The execution just created above becomes the subprocess scope.
    let subprocess_exec = execution;  // already INSERT'd above
    // Mark it active in history
    // Find the inner start event (exactly one StartEvent in sub_graph)
    // Call run_to_wait(tx, instance_id, inner_start_id, sub_graph,
    //                  Some(subprocess_exec.id),
    //                  Some(SubprocessCtx { outer_graph: graph, subprocess_element_id: &node.id }))
    // After inner run_to_wait returns:
    //   - If subprocess is already completed (its state = 'completed'), push outgoing
    //   - Otherwise, nothing to push — the subprocess is waiting on a task/event inside
}
```

**`EndEvent` arm changes**:

When `subprocess_ctx` is `Some(ctx)` and the current scope (`scope`) is `Some(subprocess_exec_id)`:
- This EndEvent is the inner terminus of a subprocess.
- Count active executions with `parent_id = subprocess_exec_id`:
  ```sql
  SELECT COUNT(*) FROM executions
  WHERE parent_id = $1 AND state = 'active'
  ```
- If `active_inner == 0`: mark the subprocess execution as completed, then push outgoing from `ctx.outer_graph.outgoing[ctx.subprocess_element_id]` with the subprocess execution's `parent_id` as the new scope.
- If `active_inner > 0`: non-interrupting paths are still running — don't advance yet.

For top-level EndEvent (`subprocess_ctx = None` or `scope = None`): existing instance-completion logic unchanged.

**Task resumption** (`complete_user_task`, `complete_service_task`, `fire_timer_job`, etc.):

When a wait state inside a subprocess is resumed, the execution's `parent_id` is the subprocess execution ID. This is already stored as `scope` when the token continues. The existing `scope` passing logic is therefore unchanged. The EndEvent will eventually fire with the `scope` set to the subprocess execution ID — but these resumption paths don't have `subprocess_ctx`. 

Solution: at the EndEvent, re-derive `subprocess_ctx` from the scope:
- If `scope` is Some(x), query `SELECT element_id FROM executions WHERE id = x`
- Look up element_id in the graph passed to the continuation function
- If it resolves to a `SubProcess`, build a `SubprocessCtx` on the fly

This means the continuation path (`complete_user_task` etc.) needs to pass the graph down to `run_to_wait`, which it already does. The `subprocess_ctx` can be derived inside `run_to_wait` itself by checking the scope execution's element type.

**Revised approach (no `subprocess_ctx` parameter)**:

Instead of passing `subprocess_ctx`, check at EndEvent time:

```rust
// In the EndEvent arm:
if let Some(scope_exec_id) = scope {
    // Is this scope a subprocess?
    let scope_element: (String,) = sqlx::query_as(
        "SELECT element_id FROM executions WHERE id = $1"
    ).bind(scope_exec_id).fetch_one(tx).await?;
    
    if let Some(scope_node) = graph.nodes.get(&scope_element.0) {
        if let FlowNodeKind::SubProcess { .. } = &scope_node.kind {
            // Inner subprocess EndEvent
            // Count active siblings
            // If 0: complete subprocess, push outgoing
            // Return early
        }
    }
}
// Fall through to instance-level completion check
```

This approach needs the `graph` passed to `run_to_wait` to be the graph containing the subprocess node. But when `run_to_wait` is called for inner elements, the `graph` is the *inner* sub_graph — so `graph.nodes` won't contain the subprocess element.

**Final design**: pass the outer graph explicitly.

`run_to_wait` keeps its current signature with one addition:

```rust
async fn run_to_wait(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    instance_id: Uuid,
    start_element_id: &str,
    graph: &ProcessGraph,
    scope: Option<Uuid>,
    outer_graph: Option<&ProcessGraph>, // Some when inside a subprocess
) -> Result<()>
```

- All existing callers pass `outer_graph: None`
- The SubProcess arm passes `outer_graph: Some(graph)` (current graph is now outer)
- The EndEvent arm: if `scope.is_some() && outer_graph.is_some()`, look up `scope` execution element_id in `outer_graph` to determine if it's a subprocess

#### No schema changes required

`executions.parent_id` already exists and supports subprocess scoping. Variables are already instance-scoped for reads.

### Phase 12b — Boundary Message Event (follow-on)

Parser:
```rust
BoundaryMessageEvent {
    message_name: String,
    correlation_key_expr: Option<String>,
    attached_to: String,
    cancelling: bool,
}
```

Engine: mirrors `BoundarySignalEvent` but uses message correlation (matching logic from `IntermediateMessageCatchEvent`).

### Phase 12c — Event Subprocess (follow-on)

An event subprocess is triggered by a start event (message, signal, timer) inside a subprocess scope. Non-interrupting event subprocesses spawn a parallel path inside the parent; interrupting ones cancel the parent scope. Deferred to after 12a/12b are solid.

## Tests (`tests/subprocess_test.rs`)

| Test | What it verifies |
|---|---|
| `subprocess_executes_inner_flow_before_parent_continues` | Token enters subprocess, runs inner steps, exits to parent's next element |
| `subprocess_with_user_task_pauses_and_resumes` | Subprocess containing a UserTask: instance waits on inner task, completion advances inner then exits subprocess |
| `subprocess_variables_visible_to_parent` | Variable written inside subprocess is readable after subprocess exits |
| `parent_variables_visible_inside_subprocess` | Variable written before entering subprocess is readable by inner ExclusiveGateway condition |
| `subprocess_with_exclusive_gateway` | Inner exclusive gateway routes correctly |
| `subprocess_completes_instance_after_exit` | Instance reaches completed state after subprocess |
| `nested_subprocess` | Subprocess containing another subprocess works |

## Checklist
- [ ] Failing tests written
- [ ] Implementation complete
- [ ] All tests passing (this phase + all previous)
- [ ] cargo clippy clean
- [ ] cargo fmt clean
- [ ] Phase marked complete in PLAN.md
