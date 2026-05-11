//! First-boot bootstrap of the platform admin user.
//!
//! Runs after migrations and before the HTTP listener is bound. Idempotent:
//! once any user exists in the DB, this is a no-op.
//!
//! The bootstrap user is a *global* (platform) admin: a global identity
//! with a `global_role_assignments` row for the built-in `PlatformAdmin`
//! role. They are not a member of any org until they choose to create or
//! join one. They can manage every org and every user via the platform
//! admin APIs.

use sqlx::PgPool;

use crate::config::{Config, TenantIsolation};
use crate::db;

pub async fn run_if_needed(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    let existing = db::users::count(pool).await?;
    if existing > 0 {
        return Ok(());
    }

    let (Some(email), Some(password)) = (
        config.bootstrap_admin_email.as_deref(),
        config.bootstrap_admin_password.as_deref(),
    ) else {
        if config.tenant_isolation == TenantIsolation::Single {
            anyhow::bail!(
                "CONDUIT_TENANT_ISOLATION=single requires CONDUIT_BOOTSTRAP_ADMIN_EMAIL \
                 and CONDUIT_BOOTSTRAP_ADMIN_PASSWORD on first boot — refusing to start \
                 with no users in the DB."
            );
        }
        tracing::warn!(
            "No users in the database and no bootstrap admin env vars set — \
             create a user out-of-band before any API call will succeed."
        );
        return Ok(());
    };

    if config.bootstrap_admin_org_slug.is_some() {
        tracing::warn!(
            "CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG is set but ignored — the bootstrap \
             admin is a global platform admin and is not a member of any org. \
             They can create orgs through /api/v1/orgs."
        );
    }

    let hash = crate::auth::password::hash(password)?;
    let user = db::users::insert(pool, "internal", None, email, Some(&hash)).await?;

    let granted =
        db::role_assignments::grant_global_by_name(pool, user.id, "PlatformAdmin", Some(user.id))
            .await?;
    if !granted {
        anyhow::bail!(
            "Built-in `PlatformAdmin` role not found — migration 031 did not seed the \
             permission catalog. Try `make db-reset && make migrate`."
        );
    }

    tracing::warn!(
        user_id = %user.id,
        email = %email,
        "Bootstrap platform admin created. Log in with email + password (no org slug)."
    );
    Ok(())
}
