//! First-boot bootstrap of the platform admin user.
//!
//! Runs after migrations and before the HTTP listener is bound. Idempotent:
//! once any user exists in the DB, this is a no-op.
//!
//! The bootstrap user is a *platform* admin. They live in the hidden
//! `_platform` org (seeded by migration 025), hold only the `org.create`
//! permission, and cannot themselves manage processes or instances. Their
//! job is to provision real orgs and seed each org's first Org Admin via
//! the instance-setup wizard.

use sqlx::PgPool;

use crate::config::{Config, TenantIsolation};
use crate::db;

const PLATFORM_ORG_SLUG: &str = "conduit";

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
            "CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG is set but ignored: the bootstrap \
             admin is a platform admin living in the hidden `_platform` org. \
             Real orgs are created through the instance-setup wizard."
        );
    }

    // The `conduit` system org is seeded by migration 025_platform_admin.sql
    // (then renamed by 026_rename_system_org.sql). If it is missing, the
    // deployment is in an inconsistent state — fail loudly rather than
    // silently re-creating it.
    let platform_org = db::orgs::get_by_slug(pool, PLATFORM_ORG_SLUG)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "`conduit` system org not found — migrations 025/026 did not run. \
                 Try `make db-reset && make migrate`."
            )
        })?;

    let hash = crate::auth::password::hash(password)?;
    let user =
        db::users::insert(pool, platform_org.id, "internal", None, email, Some(&hash)).await?;
    db::roles::assign_admin(pool, user.id).await?;

    tracing::warn!(
        org_id = %platform_org.id,
        user_id = %user.id,
        email = %email,
        org_slug = PLATFORM_ORG_SLUG,
        "Bootstrap platform admin created. Log in with org slug `{}`.",
        PLATFORM_ORG_SLUG
    );
    Ok(())
}
