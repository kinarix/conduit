# Graph Report - .  (2026-04-26)

## Corpus Check
- Corpus is ~28,339 words - fits in a single context window. You may not need a graph.

## Summary
- 457 nodes · 715 edges · 51 communities detected
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 38 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_BPMN Gateway Architecture|BPMN Gateway Architecture]]
- [[_COMMUNITY_Process Engine Core|Process Engine Core]]
- [[_COMMUNITY_Event Processing & Parser|Event Processing & Parser]]
- [[_COMMUNITY_Timer & Boundary Tests|Timer & Boundary Tests]]
- [[_COMMUNITY_Infrastructure & Config|Infrastructure & Config]]
- [[_COMMUNITY_Task Completion Tests|Task Completion Tests]]
- [[_COMMUNITY_Schema & DB Tests|Schema & DB Tests]]
- [[_COMMUNITY_API Integration Tests|API Integration Tests]]
- [[_COMMUNITY_Parser Tests|Parser Tests]]
- [[_COMMUNITY_Deployment Validation Tests|Deployment Validation Tests]]
- [[_COMMUNITY_Engine Methods|Engine Methods]]
- [[_COMMUNITY_External Task Tests|External Task Tests]]
- [[_COMMUNITY_External Task API Types|External Task API Types]]
- [[_COMMUNITY_BPMN Parser Types|BPMN Parser Types]]
- [[_COMMUNITY_Domain Model Types|Domain Model Types]]
- [[_COMMUNITY_External Task DB Queries|External Task DB Queries]]
- [[_COMMUNITY_App Config|App Config]]
- [[_COMMUNITY_Event Subscription Queries|Event Subscription Queries]]
- [[_COMMUNITY_Task DB Queries|Task DB Queries]]
- [[_COMMUNITY_Task API Handlers|Task API Handlers]]
- [[_COMMUNITY_Error Handling|Error Handling]]
- [[_COMMUNITY_Variable DB Queries|Variable DB Queries]]
- [[_COMMUNITY_Process Definition Queries|Process Definition Queries]]
- [[_COMMUNITY_Execution DB Queries|Execution DB Queries]]
- [[_COMMUNITY_Process Instance Queries|Process Instance Queries]]
- [[_COMMUNITY_Instance API Handlers|Instance API Handlers]]
- [[_COMMUNITY_Deployment API Handlers|Deployment API Handlers]]
- [[_COMMUNITY_Message & Signal Events|Message & Signal Events]]
- [[_COMMUNITY_Test Infrastructure|Test Infrastructure]]
- [[_COMMUNITY_Org API Handlers|Org API Handlers]]
- [[_COMMUNITY_User API Handlers|User API Handlers]]
- [[_COMMUNITY_Clustering & Concurrency|Clustering & Concurrency]]
- [[_COMMUNITY_Health Check Tests|Health Check Tests]]
- [[_COMMUNITY_App State|App State]]
- [[_COMMUNITY_Health API Handler|Health API Handler]]
- [[_COMMUNITY_Application Entry Point|Application Entry Point]]
- [[_COMMUNITY_Org DB Queries|Org DB Queries]]
- [[_COMMUNITY_User DB Queries|User DB Queries]]
- [[_COMMUNITY_DB Connection|DB Connection]]
- [[_COMMUNITY_Execution History Queries|Execution History Queries]]
- [[_COMMUNITY_Condition Evaluator|Condition Evaluator]]
- [[_COMMUNITY_Dev Methodology|Dev Methodology]]
- [[_COMMUNITY_Library Root|Library Root]]
- [[_COMMUNITY_Module Root|Module Root]]
- [[_COMMUNITY_Event Gateway|Event Gateway]]
- [[_COMMUNITY_Start Event|Start Event]]
- [[_COMMUNITY_End Event|End Event]]
- [[_COMMUNITY_BPMN Pool|BPMN Pool]]
- [[_COMMUNITY_BPMN Lane|BPMN Lane]]
- [[_COMMUNITY_Ownership & Labels|Ownership & Labels]]
- [[_COMMUNITY_DMN Decision Rules|DMN Decision Rules]]

## God Nodes (most connected - your core abstractions)
1. `setup()` - 19 edges
2. `create_org()` - 17 edges
3. `seed_definition()` - 17 edges
4. `unique_key()` - 16 edges
5. `setup()` - 14 edges
6. `seed_instance()` - 14 edges
7. `create_org()` - 13 edges
8. `unique_key()` - 13 edges
9. `timer_process_bpmn()` - 12 edges
10. `seed_execution()` - 12 edges

## Surprising Connections (you probably didn't know these)
- `Conduit Process Orchestration Engine` --implements--> `Tokio Async Runtime`  [EXTRACTED]
  README.md → docs/adr/ADR-001-async-runtime.md
- `Conduit Process Orchestration Engine` --implements--> `SQLx Migrations`  [EXTRACTED]
  README.md → docs/adr/ADR-006-migrations.md
- `Conduit Process Orchestration Engine` --shares_data_with--> `PostgreSQL (State Store)`  [EXTRACTED]
  README.md → docs/ARCHITECTURE.md
- `Concurrent execution tracking` --shares_data_with--> `executions Table`  [INFERRED]
  docs/phases/PHASE-9-parallel-gateway.md → docs/phases/PHASE-2-schema.md
- `Multiple conditional paths routing` --semantically_similar_to--> `Condition Expression Evaluation`  [INFERRED] [semantically similar]
  docs/phases/PHASE-13-inclusive-gateway.md → docs/phases/PHASE-6-exclusive-gateway.md

## Hyperedges (group relationships)
- **Core Technology Stack** — tokio_runtime, axum_framework, sqlx_driver, postgres_database [EXTRACTED 1.00]
- **Gateway Routing Mechanism** — exclusive_gateway, rhai_evaluator, process_variables, execution_token [INFERRED 0.85]
- **Concurrent Multi-Instance Safety** — for_update_skip_locked, optimistic_locking, clustering_model, job_executor [EXTRACTED 1.00]
- **Core Process Execution Schema** — process_definitions_table, process_instances_table, executions_table, variables_table, tasks_table [EXTRACTED 1.00]
- **REST API Endpoints Layer** — deployment_endpoint, instances_endpoint, tasks_endpoint, task_complete_endpoint, orgs_endpoint, users_endpoint [EXTRACTED 0.95]
- **Technology Stack Foundation** — tokio_runtime, axum_web_framework, sqlx_database_driver, sqlx_migrate_tool [EXTRACTED 1.00]
- **Gateway Conditional Routing Pattern** — exclusive_gateway, inclusive_gateway, parallel_gateway, condition_expression, token_routing [INFERRED 0.85]
- **External Event Coordination** — message_events, boundary_events, event_subscriptions_table, message_api_endpoint [INFERRED 0.82]

## Communities

### Community 0 - "BPMN Gateway Architecture"
Cohesion: 0.07
Nodes (38): API Layer (Axum REST), Axum Web Framework, Axum-Tokio Native Integration, Boundary Event (Task-Watching), BPMN 2.0 Process Specification, Cloud Native Stack Replacement, Conduit Process Orchestration Engine, Database as Source of Truth (+30 more)

### Community 1 - "Process Engine Core"
Cohesion: 0.07
Nodes (37): AUTH_PROVIDER environment config, POST /api/v1/external-tasks/:id/complete, Condition Expression Evaluation, Default flow support, POST /api/v1/deployments, executions Table, POST /api/v1/external-tasks/:id/extend-lock, POST /api/v1/external-tasks/:id/failure (+29 more)

### Community 2 - "Event Processing & Parser"
Cohesion: 0.08
Nodes (33): Boundary Events (timer, message), Boundary timer events (interrupting), BPMN Parser Module, Concurrent execution tracking, Message correlation key matching, Event subprocess support, event_subscriptions Table, Token/Execution (Position Marker) (+25 more)

### Community 3 - "Timer & Boundary Tests"
Cohesion: 0.2
Nodes (21): boundary_timer_bpmn(), boundary_timer_fires_cancels_task_and_advances_to_escalated_end(), boundary_timer_inserts_timer_job_alongside_task(), completing_user_task_cancels_boundary_timer_job(), create_org(), fire_due_timer_jobs_concurrent_executors_dont_double_fire(), fire_due_timer_jobs_fires_overdue_job(), fire_due_timer_jobs_skips_future_job() (+13 more)

### Community 4 - "Infrastructure & Config"
Cohesion: 0.08
Nodes (29): Axum Web Framework, Config Module, PostgreSQL Connection Pool, Docker Compose PostgreSQL Setup, Error Handling Strategy (Transient/Business/Engine), Error Module (EngineError), Fetch-and-Lock Worker API, FOR UPDATE SKIP LOCKED Pattern (+21 more)

### Community 5 - "Task Completion Tests"
Cohesion: 0.27
Nodes (26): complete_already_completed_task_returns_conflict(), complete_task_not_found_returns_error(), complete_task_with_variables_writes_to_db(), complete_user_task_advances_token_to_end(), complete_user_task_closes_history_entry(), create_org(), engine_cold_cache_can_start_instance(), gateway_bpmn() (+18 more)

### Community 6 - "Schema & DB Tests"
Cohesion: 0.24
Nodes (23): create_org(), event_subscription_insert_and_find_by_message(), event_subscription_signal_broadcast(), execution_cascade_delete_with_instance(), execution_insert_and_read(), execution_parent_child_relationship(), job_failure_increments_retry_count(), job_insert_and_fetch_and_lock() (+15 more)

### Community 7 - "API Integration Tests"
Cohesion: 0.24
Nodes (13): complete_already_completed_task_returns_409(), complete_task_advances_instance_to_completed(), complete_task_returns_204(), deploy_definition(), get_instance_returns_200(), get_task_returns_200(), linear_bpmn(), list_tasks_returns_pending_task() (+5 more)

### Community 8 - "Parser Tests"
Cohesion: 0.23
Nodes (11): fixture(), intermediate_timer_catch_event_is_supported(), parse_camunda_dialect(), parse_complex_subset(), parse_minimal(), parse_service_and_user(), parse_simple_user_task(), reject_dangling_flow() (+3 more)

### Community 9 - "Deployment Validation Tests"
Cohesion: 0.3
Nodes (12): deploy_does_not_persist_on_parse_failure(), deploy_empty_key_returns_400(), deploy_invalid_xml_returns_400(), deploy_missing_key_field_returns_422(), deploy_missing_start_event_returns_400(), deploy_same_key_twice_increments_version(), deploy_stores_bpmn_xml_in_db(), deploy_unsupported_gateway_returns_400() (+4 more)

### Community 10 - "Engine Methods"
Cohesion: 0.29
Nodes (3): Engine, parse_duration(), VariableInput

### Community 11 - "External Task Tests"
Cohesion: 0.28
Nodes (11): complete_advances_token_to_end(), complete_with_output_variables(), complete_wrong_worker_returns_409(), deploy_and_start(), extend_lock_updates_deadline(), failure_decrements_retries(), failure_max_retries_marks_instance_error(), fetch_and_lock_by_topic_filters() (+3 more)

### Community 12 - "External Task API Types"
Cohesion: 0.17
Nodes (6): CompleteExternalTaskRequest, ExtendLockRequest, ExternalTaskDto, FailExternalTaskRequest, FetchAndLockRequest, VariableDto

### Community 13 - "BPMN Parser Types"
Cohesion: 0.27
Nodes (10): extract_condition(), extract_timer_duration(), extract_topic(), FlowNode, FlowNodeKind, parse(), ProcessGraph, require_id() (+2 more)

### Community 14 - "Domain Model Types"
Cohesion: 0.18
Nodes (10): EventSubscription, Execution, ExecutionHistory, Job, Org, ProcessDefinition, ProcessInstance, Task (+2 more)

### Community 15 - "External Task DB Queries"
Cohesion: 0.22
Nodes (0): 

### Community 16 - "App Config"
Cohesion: 0.39
Nodes (6): AuthProvider, Config, config_fails_without_database_url(), config_uses_defaults_for_optional_vars(), optional_env(), require_env()

### Community 17 - "Event Subscription Queries"
Cohesion: 0.29
Nodes (0): 

### Community 18 - "Task DB Queries"
Cohesion: 0.29
Nodes (0): 

### Community 19 - "Task API Handlers"
Cohesion: 0.29
Nodes (2): CompleteTaskRequest, TaskListResponse

### Community 20 - "Error Handling"
Cohesion: 0.53
Nodes (4): EngineError, internal_returns_500(), not_found_returns_404(), validation_returns_400()

### Community 21 - "Variable DB Queries"
Cohesion: 0.33
Nodes (0): 

### Community 22 - "Process Definition Queries"
Cohesion: 0.33
Nodes (0): 

### Community 23 - "Execution DB Queries"
Cohesion: 0.4
Nodes (0): 

### Community 24 - "Process Instance Queries"
Cohesion: 0.4
Nodes (0): 

### Community 25 - "Instance API Handlers"
Cohesion: 0.4
Nodes (1): StartInstanceRequest

### Community 26 - "Deployment API Handlers"
Cohesion: 0.4
Nodes (2): DeployRequest, DeployResponse

### Community 27 - "Message & Signal Events"
Cohesion: 0.4
Nodes (5): Message Event (Targeted), Phase 10: Message Events, Phase 11: Signal Events, Signal Event (Broadcast), Signal vs Message Distinction

### Community 28 - "Test Infrastructure"
Cohesion: 0.5
Nodes (1): TestApp

### Community 29 - "Org API Handlers"
Cohesion: 0.5
Nodes (1): CreateOrgRequest

### Community 30 - "User API Handlers"
Cohesion: 0.5
Nodes (1): CreateUserRequest

### Community 31 - "Clustering & Concurrency"
Cohesion: 0.5
Nodes (4): Multi-Instance Clustering Model, Concurrent Execution Safety, Optimistic Locking Pattern, Phase 15: Clustering + Observability

### Community 32 - "Health Check Tests"
Cohesion: 0.67
Nodes (0): 

### Community 33 - "App State"
Cohesion: 0.67
Nodes (1): AppState

### Community 34 - "Health API Handler"
Cohesion: 0.67
Nodes (0): 

### Community 35 - "Application Entry Point"
Cohesion: 1.0
Nodes (0): 

### Community 36 - "Org DB Queries"
Cohesion: 1.0
Nodes (0): 

### Community 37 - "User DB Queries"
Cohesion: 1.0
Nodes (0): 

### Community 38 - "DB Connection"
Cohesion: 1.0
Nodes (0): 

### Community 39 - "Execution History Queries"
Cohesion: 1.0
Nodes (0): 

### Community 40 - "Condition Evaluator"
Cohesion: 1.0
Nodes (0): 

### Community 41 - "Dev Methodology"
Cohesion: 1.0
Nodes (2): Incremental Phase Methodology, Test-First Development

### Community 42 - "Library Root"
Cohesion: 1.0
Nodes (0): 

### Community 43 - "Module Root"
Cohesion: 1.0
Nodes (0): 

### Community 44 - "Event Gateway"
Cohesion: 1.0
Nodes (1): Event Based Gateway (Race)

### Community 45 - "Start Event"
Cohesion: 1.0
Nodes (1): Start Event

### Community 46 - "End Event"
Cohesion: 1.0
Nodes (1): End Event

### Community 47 - "BPMN Pool"
Cohesion: 1.0
Nodes (1): Pool (Participant Boundary)

### Community 48 - "BPMN Lane"
Cohesion: 1.0
Nodes (1): Lane (Role/Department)

### Community 49 - "Ownership & Labels"
Cohesion: 1.0
Nodes (1): Phase 5.5: Ownership + Labels

### Community 50 - "DMN Decision Rules"
Cohesion: 1.0
Nodes (1): DMN vs Code Decision Criteria

## Knowledge Gaps
- **97 isolated node(s):** `TestApp`, `AuthProvider`, `FlowNodeKind`, `FlowNode`, `SequenceFlow` (+92 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Application Entry Point`** (2 nodes): `main()`, `main.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Org DB Queries`** (2 nodes): `insert()`, `orgs.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `User DB Queries`** (2 nodes): `users.rs`, `insert()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `DB Connection`** (2 nodes): `connect()`, `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Execution History Queries`** (2 nodes): `list_by_instance()`, `execution_history.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Condition Evaluator`** (2 nodes): `evaluate_condition()`, `evaluator.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Dev Methodology`** (2 nodes): `Incremental Phase Methodology`, `Test-First Development`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Library Root`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Module Root`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Event Gateway`** (1 nodes): `Event Based Gateway (Race)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Start Event`** (1 nodes): `Start Event`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `End Event`** (1 nodes): `End Event`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `BPMN Pool`** (1 nodes): `Pool (Participant Boundary)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `BPMN Lane`** (1 nodes): `Lane (Role/Department)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Ownership & Labels`** (1 nodes): `Phase 5.5: Ownership + Labels`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `DMN Decision Rules`** (1 nodes): `DMN vs Code Decision Criteria`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Phase 3 — BPMN Parser` connect `BPMN Gateway Architecture` to `Process Engine Core`, `Event Processing & Parser`?**
  _High betweenness centrality (0.022) - this node is a cross-community bridge._
- **Why does `Job Executor (Tokio Background)` connect `Infrastructure & Config` to `Process Engine Core`, `Event Processing & Parser`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **Why does `Conduit Process Orchestration Engine` connect `BPMN Gateway Architecture` to `Infrastructure & Config`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **What connects `TestApp`, `AuthProvider`, `FlowNodeKind` to the rest of the system?**
  _97 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `BPMN Gateway Architecture` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._
- **Should `Process Engine Core` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._
- **Should `Event Processing & Parser` be split into smaller, more focused modules?**
  _Cohesion score 0.08 - nodes in this community are weakly interconnected._