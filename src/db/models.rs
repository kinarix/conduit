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

/// Internal-auth row including the argon2 password hash. Never serialized
/// over the wire — `User` is the public projection.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserCredentials {
    pub id: Uuid,
    pub org_id: Uuid,
    pub auth_provider: String,
    pub email: String,
    pub password_hash: Option<String>,
}

/// Public projection of `api_keys`. Never includes the plaintext or hash.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiKeyMetadata {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessGroup {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
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
    pub status: String,
    pub process_group_id: Uuid,
    pub disabled_at: Option<DateTime<Utc>>,
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
    /// Sequential per-(org_id, process_key) human-friendly identifier.
    /// Assigned by a BEFORE INSERT trigger; never null for inserted rows.
    pub counter: i64,
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
    pub timer_expression: Option<String>,
    pub repetitions_remaining: Option<i32>,
    /// Phase 16: HTTP connector config snapshot. `None` for non-http_task jobs
    /// and for legacy http_task jobs deployed before the connector landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<JsonValue>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TimerStartTrigger {
    pub id: Uuid,
    pub definition_id: Uuid,
    pub element_id: String,
    pub timer_expression: String,
    pub repetitions_remaining: Option<i32>,
    pub due_at: DateTime<Utc>,
    pub state: String,
    pub locked_by: Option<String>,
    pub locked_until: Option<DateTime<Utc>>,
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
pub struct ProcessEvent {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub execution_id: Option<Uuid>,
    pub event_type: String,
    pub element_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub payload: JsonValue,
    pub metadata: JsonValue,
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
pub struct DecisionDefinition {
    pub id: Uuid,
    pub org_id: Uuid,
    pub process_group_id: Option<Uuid>,
    pub decision_key: String,
    pub version: i32,
    pub name: Option<String>,
    pub dmn_xml: String,
    pub deployed_at: DateTime<Utc>,
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

/// Internal model for the `secrets` table. The encrypted value never leaves
/// the DB layer — API responses use [`SecretMetadata`] instead.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SecretRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub value_encrypted: Vec<u8>,
    pub nonce: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Public-facing secret representation. Excludes ciphertext and plaintext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SecretRow> for SecretMetadata {
    fn from(r: SecretRow) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            name: r.name,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessLayout {
    pub org_id: Uuid,
    pub process_key: String,
    pub layout_data: JsonValue,
    pub updated_at: DateTime<Utc>,
}
