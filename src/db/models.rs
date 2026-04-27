use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Org {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub org_id: Uuid,
    pub auth_provider: String,
    pub external_id: Option<String>,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessDefinition {
    pub id: Uuid,
    pub org_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub process_key: String,
    pub version: i32,
    pub name: Option<String>,
    pub bpmn_xml: String,
    pub labels: JsonValue,
    pub deployed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessInstance {
    pub id: Uuid,
    pub org_id: Uuid,
    pub definition_id: Uuid,
    pub state: String,
    pub labels: JsonValue,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Execution {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub element_id: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Variable {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub name: String,
    pub value_type: String,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub element_id: String,
    pub name: Option<String>,
    pub task_type: String,
    pub assignee: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub job_type: String,
    pub topic: Option<String>,
    pub due_date: DateTime<Utc>,
    pub locked_by: Option<String>,
    pub locked_until: Option<DateTime<Utc>>,
    pub retries: i32,
    pub retry_count: i32,
    pub error_message: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExecutionHistory {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub element_id: String,
    pub element_type: String,
    pub entered_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ParallelJoinState {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub fork_execution_id: Uuid,
    pub expected_count: i32,
    pub arrived_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventSubscription {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub event_type: String,
    pub event_name: String,
    pub correlation_key: Option<String>,
    pub element_id: String,
    pub created_at: DateTime<Utc>,
}
