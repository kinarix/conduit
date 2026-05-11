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
/// extractor.
///
/// `current_org_id` is set when the request path scopes the caller to an
/// org (via `/api/v1/orgs/{org_id}/...`). Handlers in those routes call
/// [`Principal::current_org`] to obtain the scoped org. On global routes
/// (e.g. `/api/v1/auth/me`, `/api/v1/orgs`) `current_org_id` is `None`.
///
/// `permissions` is loaded fresh on every request so role revocations take
/// effect immediately. It is the union of:
///   - permissions from every `global_role_assignments` row for the user
///   - permissions from `org_role_assignments` rows in `current_org_id`
///     (only loaded when `current_org_id` is `Some`)
///
/// `is_global_admin` is `true` iff the user has any `global_role_assignments`
/// row. Global admins are allowed to access any org's scoped routes even
/// without an explicit `org_members` row (membership check is bypassed).
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub email: String,
    pub kind: PrincipalKind,
    pub current_org_id: Option<Uuid>,
    /// Back-compat mirror of `current_org_id.unwrap_or(Uuid::nil())`. Most
    /// existing handlers say `principal.org_id` (no parens) — that keeps
    /// working after the routing refactor because the extractor populates
    /// this field from the path.
    pub org_id: Uuid,
    pub is_global_admin: bool,
    pub permissions: HashSet<Permission>,
}

impl Principal {
    /// `Ok(())` if the principal holds `perm`, `Err(Forbidden)` otherwise.
    pub fn require(&self, perm: Permission) -> Result<()> {
        if self.permissions.contains(&perm) {
            Ok(())
        } else {
            Err(EngineError::Forbidden(format!(
                "permission required: {perm}"
            )))
        }
    }

    /// Non-failing check. Use when branching on a permission rather than
    /// gating an endpoint.
    pub fn has(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    /// The org the caller is currently operating in. Errors with `Forbidden`
    /// when called on a global (non-org-scoped) route — callers should be
    /// using the org-scoped routes for anything tenant-bound.
    pub fn current_org(&self) -> Result<Uuid> {
        self.current_org_id.ok_or_else(|| {
            EngineError::Forbidden(
                "this endpoint requires an org-scoped path (/api/v1/orgs/{org_id}/...)".to_string(),
            )
        })
    }
}
