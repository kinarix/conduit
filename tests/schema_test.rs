mod common;

use chrono::Utc;
use conduit::db::{
    event_subscriptions, executions, jobs, process_definitions, process_instances, tasks, variables,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

// Each test gets a unique process key so concurrent tests don't collide on the
// UNIQUE(process_key, version) constraint or pick up leftovers from prior runs.
fn unique_key(prefix: &str) -> String {
    format!("{}.{}", prefix, uuid::Uuid::new_v4())
}

async fn seed_definition(pool: &sqlx::PgPool) -> conduit::db::models::ProcessDefinition {
    let key = unique_key("test");
    process_definitions::insert(pool, &key, 1, Some("Test Process"), "<definitions/>")
        .await
        .unwrap()
}

async fn seed_instance(
    pool: &sqlx::PgPool,
    definition_id: uuid::Uuid,
) -> conduit::db::models::ProcessInstance {
    process_instances::insert(pool, definition_id)
        .await
        .unwrap()
}

async fn seed_execution(
    pool: &sqlx::PgPool,
    instance_id: uuid::Uuid,
) -> conduit::db::models::Execution {
    executions::insert(pool, instance_id, None, "startEvent_1")
        .await
        .unwrap()
}

// ── process_definitions ───────────────────────────────────────────────────────

#[tokio::test]
async fn process_definition_insert_and_read() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    assert!(def.process_key.starts_with("test."));
    assert_eq!(def.version, 1);
    assert_eq!(def.name.as_deref(), Some("Test Process"));

    let fetched = process_definitions::get_by_id(pool, def.id).await.unwrap();
    assert_eq!(fetched.id, def.id);
    assert_eq!(fetched.bpmn_xml, "<definitions/>");
}

#[tokio::test]
async fn process_definition_unique_key_version() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let key = unique_key("unique");
    process_definitions::insert(pool, &key, 1, None, "<definitions/>")
        .await
        .unwrap();

    // Inserting same key+version again must fail
    let result = process_definitions::insert(pool, &key, 1, None, "<definitions/>").await;
    assert!(result.is_err(), "duplicate key+version should fail");
}

#[tokio::test]
async fn process_definition_next_version_increments() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let key = unique_key("versioned");
    let v1 = process_definitions::next_version(pool, &key).await.unwrap();
    process_definitions::insert(pool, &key, v1, None, "<definitions/>")
        .await
        .unwrap();

    let v2 = process_definitions::next_version(pool, &key).await.unwrap();
    assert_eq!(v2, v1 + 1);
}

#[tokio::test]
async fn process_definition_get_latest() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let key = unique_key("latest");
    process_definitions::insert(pool, &key, 1, Some("v1"), "<definitions/>")
        .await
        .unwrap();
    let def2 = process_definitions::insert(pool, &key, 2, Some("v2"), "<definitions/>")
        .await
        .unwrap();

    let latest = process_definitions::get_latest_by_key(pool, &key)
        .await
        .unwrap();
    assert_eq!(latest.id, def2.id);
    assert_eq!(latest.version, 2);
}

// ── process_instances ─────────────────────────────────────────────────────────

#[tokio::test]
async fn process_instance_insert_and_read() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    assert_eq!(inst.definition_id, def.id);
    assert_eq!(inst.state, "running");
    assert!(inst.ended_at.is_none());

    let fetched = process_instances::get_by_id(pool, inst.id).await.unwrap();
    assert_eq!(fetched.id, inst.id);
}

#[tokio::test]
async fn process_instance_fk_rejects_unknown_definition() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let fake_id = uuid::Uuid::new_v4();
    let result = process_instances::insert(pool, fake_id).await;
    assert!(result.is_err(), "FK violation should be rejected");
}

#[tokio::test]
async fn process_instance_state_update() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;

    let updated = process_instances::update_state(pool, inst.id, "completed")
        .await
        .unwrap();
    assert_eq!(updated.state, "completed");
    assert!(updated.ended_at.is_some());
}

// ── executions ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn execution_insert_and_read() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    assert_eq!(exec.instance_id, inst.id);
    assert_eq!(exec.element_id, "startEvent_1");
    assert_eq!(exec.state, "active");
    assert!(exec.parent_id.is_none());

    let fetched = executions::get_by_id(pool, exec.id).await.unwrap();
    assert_eq!(fetched.id, exec.id);
}

#[tokio::test]
async fn execution_parent_child_relationship() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let parent = seed_execution(pool, inst.id).await;

    let child = executions::insert(pool, inst.id, Some(parent.id), "subprocess_1")
        .await
        .unwrap();
    assert_eq!(child.parent_id, Some(parent.id));
}

#[tokio::test]
async fn execution_cascade_delete_with_instance() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    sqlx::query("DELETE FROM process_instances WHERE id = $1")
        .bind(inst.id)
        .execute(pool)
        .await
        .unwrap();

    let result = executions::get_by_id(pool, exec.id).await;
    assert!(
        result.is_err(),
        "execution should cascade-delete with instance"
    );
}

// ── variables ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn variable_upsert_and_read() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    let val = serde_json::json!("hello");
    let var = variables::upsert(pool, inst.id, exec.id, "greeting", "string", &val)
        .await
        .unwrap();

    assert_eq!(var.name, "greeting");
    assert_eq!(var.value_type, "string");
    assert_eq!(var.value, val);
}

#[tokio::test]
async fn variable_upsert_overwrites() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    variables::upsert(
        pool,
        inst.id,
        exec.id,
        "amount",
        "integer",
        &serde_json::json!(100),
    )
    .await
    .unwrap();
    let v2 = variables::upsert(
        pool,
        inst.id,
        exec.id,
        "amount",
        "integer",
        &serde_json::json!(200),
    )
    .await
    .unwrap();

    assert_eq!(v2.value, serde_json::json!(200));

    let all = variables::list_by_execution(pool, exec.id).await.unwrap();
    assert_eq!(all.len(), 1, "upsert should not duplicate");
}

// ── tasks ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn task_insert_and_complete() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    let task = tasks::insert(
        pool,
        inst.id,
        exec.id,
        "userTask_1",
        Some("Review Order"),
        "user_task",
        Some("alice"),
        None,
    )
    .await
    .unwrap();

    assert_eq!(task.state, "pending");
    assert_eq!(task.assignee.as_deref(), Some("alice"));

    let fetched = tasks::get_by_id(pool, task.id).await.unwrap();
    assert_eq!(fetched.task_type, "user_task");

    let completed = tasks::complete(pool, task.id).await.unwrap();
    assert_eq!(completed.state, "completed");
    assert!(completed.completed_at.is_some());
}

// ── jobs ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn job_insert_and_fetch_and_lock() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    let due = Utc::now() - chrono::Duration::seconds(1); // already due
    let job = jobs::insert(
        pool,
        inst.id,
        exec.id,
        "external_task",
        Some("email-sender"),
        due,
        3,
    )
    .await
    .unwrap();

    assert_eq!(job.state, "pending");

    let locked = jobs::fetch_and_lock(pool, "worker-1", 30, Some("email-sender"))
        .await
        .unwrap()
        .expect("should lock a due job");

    assert_eq!(locked.id, job.id);
    assert_eq!(locked.state, "locked");
    assert_eq!(locked.locked_by.as_deref(), Some("worker-1"));
    assert!(locked.locked_until.is_some());
}

#[tokio::test]
async fn job_second_worker_cannot_lock_same_job() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    let due = Utc::now() - chrono::Duration::seconds(1);
    jobs::insert(
        pool,
        inst.id,
        exec.id,
        "external_task",
        Some("payment"),
        due,
        3,
    )
    .await
    .unwrap();

    jobs::fetch_and_lock(pool, "worker-a", 30, Some("payment"))
        .await
        .unwrap()
        .expect("worker-a should lock the job");

    let second = jobs::fetch_and_lock(pool, "worker-b", 30, Some("payment"))
        .await
        .unwrap();
    assert!(
        second.is_none(),
        "worker-b should not get an already-locked job"
    );
}

#[tokio::test]
async fn job_failure_increments_retry_count() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    let due = Utc::now() - chrono::Duration::seconds(1);
    let job = jobs::insert(
        pool,
        inst.id,
        exec.id,
        "external_task",
        Some("risky"),
        due,
        3,
    )
    .await
    .unwrap();

    let failed = jobs::record_failure(pool, job.id, "connection refused")
        .await
        .unwrap();
    assert_eq!(failed.retry_count, 1);
    assert_eq!(failed.state, "pending"); // not yet exhausted
    assert_eq!(failed.error_message.as_deref(), Some("connection refused"));
}

// ── event_subscriptions ───────────────────────────────────────────────────────

#[tokio::test]
async fn event_subscription_insert_and_find_by_message() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst = seed_instance(pool, def.id).await;
    let exec = seed_execution(pool, inst.id).await;

    // Use a unique event name + correlation key to avoid collisions across runs.
    let event_name = unique_key("OrderApproved");
    let corr_key = unique_key("order");

    event_subscriptions::insert(
        pool,
        inst.id,
        exec.id,
        "message",
        &event_name,
        Some(corr_key.as_str()),
        "catch_1",
    )
    .await
    .unwrap();

    let found = event_subscriptions::find_by_message(pool, &event_name, Some(corr_key.as_str()))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].instance_id, inst.id);

    // Wrong correlation key → no match
    let not_found = event_subscriptions::find_by_message(pool, &event_name, Some("order-wrong"))
        .await
        .unwrap();
    assert!(not_found.is_empty());
}

#[tokio::test]
async fn event_subscription_signal_broadcast() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let def = seed_definition(pool).await;
    let inst1 = seed_instance(pool, def.id).await;
    let exec1 = seed_execution(pool, inst1.id).await;
    let inst2 = seed_instance(pool, def.id).await;
    let exec2 = seed_execution(pool, inst2.id).await;

    event_subscriptions::insert(
        pool,
        inst1.id,
        exec1.id,
        "signal",
        "GlobalAlert",
        None,
        "catch_1",
    )
    .await
    .unwrap();
    event_subscriptions::insert(
        pool,
        inst2.id,
        exec2.id,
        "signal",
        "GlobalAlert",
        None,
        "catch_2",
    )
    .await
    .unwrap();

    let all = event_subscriptions::find_by_signal(pool, "GlobalAlert")
        .await
        .unwrap();
    assert!(
        all.len() >= 2,
        "broadcast should find all waiting instances"
    );
}

// ── index existence ───────────────────────────────────────────────────────────

#[tokio::test]
async fn partial_index_on_jobs_exists() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM pg_indexes
        WHERE tablename = 'jobs'
          AND indexname  = 'idx_jobs_due_date_unlocked'
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(
        count, 1,
        "partial index idx_jobs_due_date_unlocked must exist"
    );
}
