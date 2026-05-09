//! First-boot bootstrap of the initial org + admin user.
//!
//! Runs after migrations and before the HTTP listener is bound. Idempotent:
//! once any user exists in the DB, this is a no-op. Required when
//! `tenant_isolation = Single` so the deployment is reachable on first boot.

use sqlx::PgPool;

use crate::config::{Config, TenantIsolation};
use crate::db;

pub async fn run_if_needed(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    let existing = db::users::count(pool).await?;
    if existing > 0 {
        return Ok(());
    }

    let (Some(email), Some(password), Some(slug)) = (
        config.bootstrap_admin_email.as_deref(),
        config.bootstrap_admin_password.as_deref(),
        config.bootstrap_admin_org_slug.as_deref(),
    ) else {
        if config.tenant_isolation == TenantIsolation::Single {
            anyhow::bail!(
                "CONDUIT_TENANT_ISOLATION=single requires CONDUIT_BOOTSTRAP_ADMIN_EMAIL, \
                 CONDUIT_BOOTSTRAP_ADMIN_PASSWORD, and CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG \
                 on first boot — refusing to start with no users in the DB."
            );
        }
        tracing::warn!(
            "No users in the database and no bootstrap admin env vars set — \
             create a user out-of-band before any API call will succeed."
        );
        return Ok(());
    };

    let org = db::orgs::insert(pool, "Default", slug).await?;
    let hash = crate::auth::password::hash(password)?;
    let user = db::users::insert(pool, org.id, "internal", None, email, Some(&hash)).await?;
    db::roles::assign_admin(pool, user.id).await?;
    tracing::warn!(
        org_id = %org.id,
        user_id = %user.id,
        email = %email,
        "Bootstrap admin created from CONDUIT_BOOTSTRAP_ADMIN_* env vars"
    );
    Ok(())
}
