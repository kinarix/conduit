use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessDefinition {
    pub id: Uuid,
    pub process_key: String,
    pub version: i32,
    pub name: Option<String>,
    pub bpmn_xml: String,
    pub deployed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessInstance {
    pub id: Uuid,
    pub definition_id: Uuid,
    pub state: String,
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
