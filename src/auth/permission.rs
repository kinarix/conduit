//! Permission catalog — Phase 23.1.
//!
//! Naming rule:
//!   - CRUD verbs (`create` / `read` / `update` / `delete`) for reference data.
//!   - Domain verbs only when authorization meaningfully differs from CRUD
//!     (state transitions, designed-vs-promoted artifacts, read-metadata vs
//!     read-plaintext).
//!
//! Single source of truth: this file. `migrations/031_permission_catalog.sql`
//! must list exactly the same strings in its CHECK constraint. The
//! `permission_catalog_in_sync_with_migration` test asserts this.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    // org
    OrgCreate,
    OrgRead,
    OrgUpdate,
    OrgDelete,
    // org membership
    OrgMemberCreate,
    OrgMemberRead,
    OrgMemberDelete,
    // users (global identity)
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,
    // role definitions
    RoleCreate,
    RoleRead,
    RoleUpdate,
    RoleDelete,
    // role assignments (grants)
    RoleAssignmentCreate,
    RoleAssignmentRead,
    RoleAssignmentDelete,
    // per-org auth provider config
    AuthConfigRead,
    AuthConfigUpdate,
    // process definitions (BPMN)
    ProcessCreate,
    ProcessRead,
    ProcessUpdate,
    ProcessDelete,
    ProcessDeploy,
    ProcessDisable,
    // process groups
    ProcessGroupCreate,
    ProcessGroupRead,
    ProcessGroupUpdate,
    ProcessGroupDelete,
    // process instances
    InstanceRead,
    InstanceStart,
    InstanceCancel,
    InstancePause,
    InstanceResume,
    InstanceDelete,
    // user tasks
    TaskRead,
    TaskComplete,
    TaskUpdate,
    // external (worker) tasks
    ExternalTaskExecute,
    // decisions (DMN)
    DecisionCreate,
    DecisionRead,
    DecisionUpdate,
    DecisionDelete,
    DecisionDeploy,
    // secrets — split read for compliance
    SecretCreate,
    SecretReadMetadata,
    SecretReadPlaintext,
    SecretUpdate,
    SecretDelete,
    // api keys (admin-managed)
    ApiKeyManage,
    // process layout
    ProcessLayoutRead,
    ProcessLayoutUpdate,
    // business events
    MessageCorrelate,
    SignalBroadcast,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrgCreate => "org.create",
            Self::OrgRead => "org.read",
            Self::OrgUpdate => "org.update",
            Self::OrgDelete => "org.delete",
            Self::OrgMemberCreate => "org_member.create",
            Self::OrgMemberRead => "org_member.read",
            Self::OrgMemberDelete => "org_member.delete",
            Self::UserCreate => "user.create",
            Self::UserRead => "user.read",
            Self::UserUpdate => "user.update",
            Self::UserDelete => "user.delete",
            Self::RoleCreate => "role.create",
            Self::RoleRead => "role.read",
            Self::RoleUpdate => "role.update",
            Self::RoleDelete => "role.delete",
            Self::RoleAssignmentCreate => "role_assignment.create",
            Self::RoleAssignmentRead => "role_assignment.read",
            Self::RoleAssignmentDelete => "role_assignment.delete",
            Self::AuthConfigRead => "auth_config.read",
            Self::AuthConfigUpdate => "auth_config.update",
            Self::ProcessCreate => "process.create",
            Self::ProcessRead => "process.read",
            Self::ProcessUpdate => "process.update",
            Self::ProcessDelete => "process.delete",
            Self::ProcessDeploy => "process.deploy",
            Self::ProcessDisable => "process.disable",
            Self::ProcessGroupCreate => "process_group.create",
            Self::ProcessGroupRead => "process_group.read",
            Self::ProcessGroupUpdate => "process_group.update",
            Self::ProcessGroupDelete => "process_group.delete",
            Self::InstanceRead => "instance.read",
            Self::InstanceStart => "instance.start",
            Self::InstanceCancel => "instance.cancel",
            Self::InstancePause => "instance.pause",
            Self::InstanceResume => "instance.resume",
            Self::InstanceDelete => "instance.delete",
            Self::TaskRead => "task.read",
            Self::TaskComplete => "task.complete",
            Self::TaskUpdate => "task.update",
            Self::ExternalTaskExecute => "external_task.execute",
            Self::DecisionCreate => "decision.create",
            Self::DecisionRead => "decision.read",
            Self::DecisionUpdate => "decision.update",
            Self::DecisionDelete => "decision.delete",
            Self::DecisionDeploy => "decision.deploy",
            Self::SecretCreate => "secret.create",
            Self::SecretReadMetadata => "secret.read_metadata",
            Self::SecretReadPlaintext => "secret.read_plaintext",
            Self::SecretUpdate => "secret.update",
            Self::SecretDelete => "secret.delete",
            Self::ApiKeyManage => "api_key.manage",
            Self::ProcessLayoutRead => "process_layout.read",
            Self::ProcessLayoutUpdate => "process_layout.update",
            Self::MessageCorrelate => "message.correlate",
            Self::SignalBroadcast => "signal.broadcast",
        }
    }

    /// `true` iff this permission can ONLY be held via a global role
    /// assignment (it has no meaningful per-org scope). Currently just
    /// `org.create`.
    pub fn is_global_only(self) -> bool {
        matches!(self, Self::OrgCreate)
    }

    /// Full catalog. Order matters only for tests / printable docs — runtime
    /// callers should use the enum directly.
    pub const ALL: &'static [Permission] = &[
        Self::OrgCreate,
        Self::OrgRead,
        Self::OrgUpdate,
        Self::OrgDelete,
        Self::OrgMemberCreate,
        Self::OrgMemberRead,
        Self::OrgMemberDelete,
        Self::UserCreate,
        Self::UserRead,
        Self::UserUpdate,
        Self::UserDelete,
        Self::RoleCreate,
        Self::RoleRead,
        Self::RoleUpdate,
        Self::RoleDelete,
        Self::RoleAssignmentCreate,
        Self::RoleAssignmentRead,
        Self::RoleAssignmentDelete,
        Self::AuthConfigRead,
        Self::AuthConfigUpdate,
        Self::ProcessCreate,
        Self::ProcessRead,
        Self::ProcessUpdate,
        Self::ProcessDelete,
        Self::ProcessDeploy,
        Self::ProcessDisable,
        Self::ProcessGroupCreate,
        Self::ProcessGroupRead,
        Self::ProcessGroupUpdate,
        Self::ProcessGroupDelete,
        Self::InstanceRead,
        Self::InstanceStart,
        Self::InstanceCancel,
        Self::InstancePause,
        Self::InstanceResume,
        Self::InstanceDelete,
        Self::TaskRead,
        Self::TaskComplete,
        Self::TaskUpdate,
        Self::ExternalTaskExecute,
        Self::DecisionCreate,
        Self::DecisionRead,
        Self::DecisionUpdate,
        Self::DecisionDelete,
        Self::DecisionDeploy,
        Self::SecretCreate,
        Self::SecretReadMetadata,
        Self::SecretReadPlaintext,
        Self::SecretUpdate,
        Self::SecretDelete,
        Self::ApiKeyManage,
        Self::ProcessLayoutRead,
        Self::ProcessLayoutUpdate,
        Self::MessageCorrelate,
        Self::SignalBroadcast,
    ];
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Permission {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Hot path lives in token-load code; the `ALL` scan is fine for ~55
        // entries but if this becomes hot we can switch to phf.
        for &p in Self::ALL {
            if p.as_str() == s {
                return Ok(p);
            }
        }
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enum ↔ migration parity. Parses the CHECK constraint out of
    /// migrations/031_permission_catalog.sql and asserts the set of strings
    /// matches `Permission::ALL`.
    #[test]
    fn permission_catalog_in_sync_with_migration() {
        let migration = include_str!("../../migrations/031_permission_catalog.sql");
        // Grab the first CHECK block — the one that defines the catalog.
        let check_start = migration
            .find("CHECK (permission IN (")
            .expect("CHECK block not found in 031_permission_catalog.sql");
        let after = &migration[check_start..];
        let close = after.find("));").expect("close of CHECK block not found");
        let block = &after[..close];

        let mut from_sql: std::collections::HashSet<String> = std::collections::HashSet::new();
        for token in block.split(',') {
            if let Some(start) = token.find('\'') {
                let rest = &token[start + 1..];
                if let Some(end) = rest.find('\'') {
                    from_sql.insert(rest[..end].to_string());
                }
            }
        }

        let from_enum: std::collections::HashSet<String> =
            Permission::ALL.iter().map(|p| p.as_str().to_string()).collect();

        let only_in_enum: Vec<_> = from_enum.difference(&from_sql).collect();
        let only_in_sql: Vec<_> = from_sql.difference(&from_enum).collect();
        assert!(
            only_in_enum.is_empty() && only_in_sql.is_empty(),
            "Permission enum drifted from migration 031.\n  only in enum: {only_in_enum:?}\n  only in SQL : {only_in_sql:?}"
        );
    }

    #[test]
    fn round_trip_all_permissions() {
        for &p in Permission::ALL {
            let s = p.as_str();
            assert_eq!(Permission::from_str(s).unwrap(), p);
        }
    }
}
