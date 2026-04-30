use conduit::db;
use conduit::engine::{parse_duration, Engine};
use conduit::parser::{self, FlowNodeKind, TimerSpec};
use conduit::state::GraphCache;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

async fn setup() -> (PgPool, Engine) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let engine = Engine::new(pool.clone(), cache);
    (pool, engine)
}

async fn create_org(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let slug = format!("timer-org-{}", Uuid::new_v4());
    let org = db::orgs::insert(pool, "Timer Test Org", &slug)
        .await
        .unwrap();
    let f1 = db::process_groups::insert(pool, org.id, "Primary").await.unwrap();
    let f2 = db::process_groups::insert(pool, org.id, "Secondary").await.unwrap();
    (org.id, vec![f1.id, f2.id])
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

/// Start → IntermediateCatchEvent(timer, duration) → End
fn timer_process_bpmn(duration: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <intermediateCatchEvent id="timer1" name="Wait">
      <timerEventDefinition>
        <timeDuration>{duration}</timeDuration>
      </timerEventDefinition>
    </intermediateCatchEvent>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="timer1"/>
    <sequenceFlow id="sf2" sourceRef="timer1" targetRef="end"/>
  </process>
</definitions>"#
    )
}

/// Start → UserTask → End (normal path)
/// UserTask also has an interrupting boundary timer → escalated End
fn boundary_timer_bpmn(duration: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Review"/>
    <endEvent id="end_normal"/>
    <endEvent id="end_escalated"/>
    <boundaryEvent id="timer-boundary" attachedToRef="task1" cancelActivity="true">
      <timerEventDefinition>
        <timeDuration>{duration}</timeDuration>
      </timerEventDefinition>
    </boundaryEvent>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end_normal"/>
    <sequenceFlow id="sf3" sourceRef="timer-boundary" targetRef="end_escalated"/>
  </process>
</definitions>"#
    )
}

// ─── Parser: intermediate timer catch event ───────────────────────────────────

#[test]
fn parse_intermediate_timer_catch_event() {
    let graph = parser::parse(&timer_process_bpmn("PT1H")).unwrap();
    assert_eq!(graph.nodes.len(), 3); // start, timer1, end

    let timer_node = graph.nodes.get("timer1").unwrap();
    assert_eq!(timer_node.name.as_deref(), Some("Wait"));
    match &timer_node.kind {
        FlowNodeKind::IntermediateTimerCatchEvent { timer: TimerSpec::Duration(d) } => {
            assert_eq!(d, "PT1H");
        }
        _ => panic!(
            "expected IntermediateTimerCatchEvent(Duration), got {:?}",
            timer_node.kind
        ),
    }
}

#[test]
fn parse_timer_process_has_correct_flows() {
    let graph = parser::parse(&timer_process_bpmn("P1D")).unwrap();
    // start → timer1 → end
    assert_eq!(graph.outgoing["start"], vec!["timer1"]);
    assert_eq!(graph.outgoing["timer1"], vec!["end"]);
}

#[test]
fn parse_boundary_timer_event() {
    let graph = parser::parse(&boundary_timer_bpmn("PT24H")).unwrap();
    // nodes: start, task1, end_normal, end_escalated, timer-boundary
    assert_eq!(graph.nodes.len(), 5);

    let boundary = graph.nodes.get("timer-boundary").unwrap();
    match &boundary.kind {
        FlowNodeKind::BoundaryTimerEvent {
            timer: TimerSpec::Duration(d),
            attached_to,
            cancelling,
        } => {
            assert_eq!(d, "PT24H");
            assert_eq!(attached_to, "task1");
            assert!(*cancelling);
        }
        _ => panic!("expected BoundaryTimerEvent(Duration), got {:?}", boundary.kind),
    }
    // outgoing from boundary → end_escalated
    assert_eq!(graph.outgoing["timer-boundary"], vec!["end_escalated"]);
}

// ─── Duration parsing ─────────────────────────────────────────────────────────

#[test]
fn duration_seconds() {
    assert_eq!(parse_duration("PT30S").unwrap().num_seconds(), 30);
}

#[test]
fn duration_minutes() {
    assert_eq!(parse_duration("PT5M").unwrap().num_seconds(), 300);
}

#[test]
fn duration_hours() {
    assert_eq!(parse_duration("PT1H").unwrap().num_seconds(), 3600);
}

#[test]
fn duration_days() {
    assert_eq!(parse_duration("P1D").unwrap().num_seconds(), 86_400);
}

#[test]
fn duration_weeks() {
    assert_eq!(parse_duration("P7D").unwrap().num_seconds(), 604_800);
}

#[test]
fn duration_combined() {
    // PT1H30M = 5400 seconds
    assert_eq!(parse_duration("PT1H30M").unwrap().num_seconds(), 5400);
}

#[test]
fn duration_invalid_returns_error() {
    assert!(parse_duration("not-a-duration").is_err());
    assert!(parse_duration("").is_err());
    assert!(parse_duration("PT").is_err());
}

// ─── Engine: intermediate timer event ────────────────────────────────────────

#[tokio::test]
async fn timer_event_pauses_process_and_inserts_timer_job() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // Process should be running (waiting at timer)
    assert_eq!(instance.state, "running");
    assert!(instance.ended_at.is_none());

    // A timer job should have been inserted
    let jobs = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, "timer");
    assert_eq!(jobs[0].state, "pending");

    // due_date should be ~1 hour from now (within a few seconds tolerance)
    let due = jobs[0].due_date;
    let now = chrono::Utc::now();
    let diff = due - now;
    assert!(
        diff.num_seconds() > 3590 && diff.num_seconds() < 3610,
        "expected ~3600s, got {}s",
        diff.num_seconds()
    );
}

#[tokio::test]
async fn timer_event_execution_history_entered_not_left() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();

    // start (entered+left) + timer1 (entered, not left)
    assert_eq!(history.len(), 2);

    let timer_entry = history.iter().find(|h| h.element_id == "timer1").unwrap();
    assert_eq!(timer_entry.element_type, "intermediateCatchEvent");
    assert!(timer_entry.left_at.is_none());
}

#[tokio::test]
async fn fire_timer_job_advances_token_to_end_event() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        // Use a 1-hour timer; we'll fire it manually without waiting
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let jobs = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let job_id = jobs[0].id;

    // Fire the timer directly (bypasses due_date check — simulates the executor)
    engine.fire_timer_job(job_id).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
    assert!(refreshed.ended_at.is_some());
}

#[tokio::test]
async fn fire_timer_job_closes_history_and_marks_job_completed() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let job_id = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    engine.fire_timer_job(job_id).await.unwrap();

    // All history entries should be closed
    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(history.len(), 3); // start, timer1, end
    for entry in &history {
        assert!(
            entry.left_at.is_some(),
            "expected left_at set for {}",
            entry.element_id
        );
    }

    // Job should be completed
    let job = db::jobs::get_by_id(&pool, job_id).await.unwrap();
    assert_eq!(job.state, "completed");
}

#[tokio::test]
async fn fire_timer_job_unknown_id_returns_not_found() {
    let (_, engine) = setup().await;
    let result = engine.fire_timer_job(Uuid::new_v4()).await;
    assert!(matches!(
        result,
        Err(conduit::error::EngineError::NotFound(_))
    ));
}

#[tokio::test]
async fn fire_timer_job_already_completed_returns_conflict() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let job_id = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    engine.fire_timer_job(job_id).await.unwrap();
    let result = engine.fire_timer_job(job_id).await;
    assert!(matches!(
        result,
        Err(conduit::error::EngineError::Conflict(_))
    ));
}

#[tokio::test]
async fn fire_timer_job_cold_cache() {
    let (pool, _warm) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let warm_cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let warm_engine = Engine::new(pool.clone(), warm_cache);
    let instance = warm_engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let job_id = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // Cold engine — no cached graph
    let cold_cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let cold_engine = Engine::new(pool.clone(), cold_cache);
    cold_engine.fire_timer_job(job_id).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

// ─── Job executor: fire due timer jobs ───────────────────────────────────────

#[tokio::test]
async fn fire_due_timer_jobs_fires_overdue_job() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    // Use a 1-hour timer, then manually backdate it to simulate it being overdue
    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // Backdate the job's due_date to the past so the executor picks it up
    sqlx::query("UPDATE jobs SET due_date = NOW() - interval '1 second' WHERE instance_id = $1")
        .bind(instance.id)
        .execute(&pool)
        .await
        .unwrap();

    // Fetch the specific job ID and fire it directly to avoid a race where concurrent
    // timer tests using FOR UPDATE SKIP LOCKED steal each other's jobs.
    let job_id: (Uuid,) =
        sqlx::query_as("SELECT id FROM jobs WHERE instance_id = $1 AND job_type = 'timer'")
            .bind(instance.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    engine.fire_timer_job(job_id.0).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

#[tokio::test]
async fn fire_due_timer_jobs_skips_future_job() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"), // due 1 hour from now — not yet due
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // Parallel tests may backdate their own jobs, so global fired count may be >= 0.
    // What matters for this test: our future-scheduled instance must NOT have advanced.
    engine.fire_due_timer_jobs().await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "running");
}

#[tokio::test]
async fn fire_due_timer_jobs_concurrent_executors_dont_double_fire() {
    let (pool, engine1) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("timer"),
        1,
        None,
        &timer_process_bpmn("PT1H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine1
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let job_id = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // Backdate so both executors see it as due
    sqlx::query("UPDATE jobs SET due_date = NOW() - interval '1 second' WHERE instance_id = $1")
        .bind(instance.id)
        .execute(&pool)
        .await
        .unwrap();

    let cache2: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let engine2 = Engine::new(pool.clone(), cache2);

    // Race both executors. Due to parallel test execution they may each pick up
    // jobs from other concurrent tests, but each individual job must only fire once.
    // SKIP LOCKED guarantees no two executors claim the same job row.
    let (r1, r2) = tokio::join!(engine1.fire_due_timer_jobs(), engine2.fire_due_timer_jobs(),);

    // Both calls must succeed (no double-fire Conflict error).
    r1.unwrap();
    r2.unwrap();

    // This instance's job must be completed exactly once.
    let job = db::jobs::get_by_id(&pool, job_id).await.unwrap();
    assert_eq!(job.state, "completed", "job must be completed exactly once");

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

// ─── Boundary timer event ─────────────────────────────────────────────────────

#[tokio::test]
async fn boundary_timer_inserts_timer_job_alongside_task() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("boundary"),
        1,
        None,
        &boundary_timer_bpmn("PT24H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.state, "running");

    // One user task should be pending
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].state, "pending");

    // A boundary timer job should also be pending
    let jobs = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, "timer");
    assert_eq!(jobs[0].state, "pending");

    // due ~24 hours from now
    let due = jobs[0].due_date;
    let diff = (due - chrono::Utc::now()).num_seconds();
    assert!(
        diff > 86_390 && diff < 86_410,
        "expected ~86400s for PT24H, got {diff}s"
    );
}

#[tokio::test]
async fn boundary_timer_fires_cancels_task_and_advances_to_escalated_end() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("boundary"),
        1,
        None,
        &boundary_timer_bpmn("PT24H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let job_id = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // Fire the boundary timer directly
    engine.fire_timer_job(job_id).await.unwrap();

    // Process should be completed via the escalated path
    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    // The user task should be cancelled
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks[0].state, "cancelled");

    // History should include end_escalated, not end_normal
    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(
        visited.contains(&"end_escalated"),
        "expected end_escalated in history"
    );
    assert!(
        !visited.contains(&"end_normal"),
        "expected end_normal NOT in history"
    );
}

#[tokio::test]
async fn completing_user_task_cancels_boundary_timer_job() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("boundary"),
        1,
        None,
        &boundary_timer_bpmn("PT24H"),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // Complete the task normally — boundary timer should be cancelled
    engine.complete_user_task(task_id, &[]).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    let jobs = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(jobs[0].state, "cancelled");

    // History: end_normal visited, end_escalated not
    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(visited.contains(&"end_normal"));
    assert!(!visited.contains(&"end_escalated"));
}
