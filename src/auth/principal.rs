use std::collections::HashSet;
use uuid::Uuid;

use super::permission::Permission;
use crate::error::{EngineError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    Jwt,
    ApiKey,
}

/// The authenticated caller, derived once per request by the `Principal`
/// extractor. `org_id` is authoritative — handlers MUST scope queries to
/// `principal.org_id` and ignore any client-supplied org reference.
/// `permissions` is loaded fresh on every request so role revocations
/// take effect without waiting for token expiry.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub kind: PrincipalKind,
    pub permissions: HashSet<Permission>,
}

impl Principal {
    /// Returns `Ok(())` if the principal holds `perm`, `Err(EngineError::Forbidden)` otherwise.
    pub fn require(&self, perm: Permission) -> Result<()> {
        if self.permissions.contains(&perm) {
            Ok(())
        } else {
            Err(EngineError::Forbidden(format!(
                "permission required: {perm}"
            )))
        }
    }
}
