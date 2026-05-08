use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// One external task, as delivered by `/external-tasks/fetch-and-lock`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalTask {
    pub id: Uuid,
    pub topic: Option<String>,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub locked_until: Option<DateTime<Utc>>,
    pub retries: i32,
    pub retry_count: i32,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

impl ExternalTask {
    /// Variables collapsed into a name → value map for handler convenience.
    pub fn variable_map(&self) -> HashMap<String, serde_json::Value> {
        self.variables
            .iter()
            .map(|v| (v.name.clone(), v.value.clone()))
            .collect()
    }

    /// Look up a single variable by name.
    pub fn variable(&self, name: &str) -> Option<&serde_json::Value> {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .map(|v| &v.value)
    }
}

/// Wire shape of a process variable (matches the engine's `VariableInput` /
/// `VariableDto`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    /// String, Long, Double, Boolean, Json, Null.
    pub value_type: String,
    pub value: serde_json::Value,
}

impl Variable {
    pub fn string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value_type: "String".into(),
            value: serde_json::Value::String(value.into()),
        }
    }

    pub fn long(name: impl Into<String>, value: i64) -> Self {
        Self {
            name: name.into(),
            value_type: "Long".into(),
            value: serde_json::json!(value),
        }
    }

    pub fn double(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            value_type: "Double".into(),
            value: serde_json::json!(value),
        }
    }

    pub fn boolean(name: impl Into<String>, value: bool) -> Self {
        Self {
            name: name.into(),
            value_type: "Boolean".into(),
            value: serde_json::Value::Bool(value),
        }
    }

    pub fn json(name: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            value_type: "Json".into(),
            value,
        }
    }

    pub fn null(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value_type: "Null".into(),
            value: serde_json::Value::Null,
        }
    }
}

/// Convenience enum for the most common variable shapes — kept for
/// callers that prefer it over the wire `Variable` directly.
#[derive(Debug, Clone)]
pub enum VariableValue {
    String(String),
    Long(i64),
    Double(f64),
    Boolean(bool),
    Json(serde_json::Value),
    Null,
}

impl VariableValue {
    pub fn into_variable(self, name: impl Into<String>) -> Variable {
        match self {
            VariableValue::String(s) => Variable::string(name, s),
            VariableValue::Long(n) => Variable::long(name, n),
            VariableValue::Double(f) => Variable::double(name, f),
            VariableValue::Boolean(b) => Variable::boolean(name, b),
            VariableValue::Json(v) => Variable::json(name, v),
            VariableValue::Null => Variable::null(name),
        }
    }
}
