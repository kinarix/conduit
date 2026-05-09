#[derive(Debug, Clone, PartialEq)]
pub enum AuthProvider {
    Internal,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantIsolation {
    /// Many orgs share one deployment. Cross-org access is a hard boundary.
    Multi,
    /// One org per deployment. Bootstrap-admin env vars are required to
    /// seed the single root org and admin user on first boot.
    Single,
}

/// Application configuration loaded from environment variables.
/// See .env.example for all available settings.
#[derive(Debug, Clone)]
pub struct Config {
    // Required
    pub database_url: String,

    // Auth
    pub auth_provider: AuthProvider,

    // Server
    pub server_host: String,
    pub server_port: u16,

    // Logging
    pub log_level: String,

    // DB pool
    pub db_min_connections: u32,
    pub db_max_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub db_statement_timeout_ms: u64,

    // Job executor (added in Phase 8)
    pub job_executor_poll_ms: u64,
    pub job_executor_batch_size: i64,
    pub job_lock_duration_secs: i64,

    // Secrets encryption (Phase 16: HTTP connector). 32 raw bytes used as the
    // ChaCha20-Poly1305 master key. Loaded eagerly so a misconfigured
    // deployment fails at startup, not on first secret read.
    pub secrets_key: [u8; 32],

    // Auth (Phase 22). HS256 signing key for Conduit-issued JWTs; the
    // authenticator both signs (login) and verifies (extractor) with this key.
    pub jwt_signing_key: String,
    pub jwt_ttl_seconds: i64,
    pub jwt_issuer: String,

    pub tenant_isolation: TenantIsolation,

    // First-boot bootstrap. When the users table is empty and these are set,
    // an org + internal-auth admin user are created so the deployment is
    // immediately usable. Required for `tenant_isolation = Single`.
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
    pub bootstrap_admin_org_slug: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env file if present (ignored if missing)
        dotenvy::dotenv().ok();

        // `DATABASE_URL` is unprefixed by convention (sqlx/Postgres ecosystem
        // defaults). All other Conduit settings use the `CONDUIT_` prefix.
        let auth_provider = match optional_env("CONDUIT_AUTH_PROVIDER", "internal").as_str() {
            "external" => AuthProvider::External,
            _ => AuthProvider::Internal,
        };

        let tenant_isolation = match optional_env("CONDUIT_TENANT_ISOLATION", "multi")
            .to_lowercase()
            .as_str()
        {
            "single" => TenantIsolation::Single,
            "multi" => TenantIsolation::Multi,
            other => {
                return Err(anyhow::anyhow!(
                    "CONDUIT_TENANT_ISOLATION must be 'multi' or 'single', got '{other}'"
                ));
            }
        };

        Ok(Config {
            database_url: require_env("DATABASE_URL")?,

            auth_provider,

            server_host: optional_env("CONDUIT_SERVER_HOST", "0.0.0.0"),
            server_port: optional_env("CONDUIT_SERVER_PORT", "8080")
                .parse()
                .map_err(|_| anyhow::anyhow!("CONDUIT_SERVER_PORT must be a valid port number"))?,

            log_level: optional_env("CONDUIT_LOG_LEVEL", "info"),

            db_min_connections: optional_env("CONDUIT_DB_MIN_CONNECTIONS", "2")
                .parse()
                .map_err(|_| anyhow::anyhow!("CONDUIT_DB_MIN_CONNECTIONS must be an integer"))?,
            db_max_connections: optional_env("CONDUIT_DB_MAX_CONNECTIONS", "10")
                .parse()
                .map_err(|_| anyhow::anyhow!("CONDUIT_DB_MAX_CONNECTIONS must be an integer"))?,
            db_acquire_timeout_secs: optional_env("CONDUIT_DB_ACQUIRE_TIMEOUT_SECS", "30")
                .parse()
                .map_err(|_| {
                    anyhow::anyhow!("CONDUIT_DB_ACQUIRE_TIMEOUT_SECS must be an integer")
                })?,
            db_statement_timeout_ms: optional_env("CONDUIT_DB_STATEMENT_TIMEOUT_MS", "30000")
                .parse()
                .map_err(|_| {
                    anyhow::anyhow!("CONDUIT_DB_STATEMENT_TIMEOUT_MS must be an integer")
                })?,

            job_executor_poll_ms: optional_env("CONDUIT_JOB_EXECUTOR_POLL_MS", "200")
                .parse()
                .map_err(|_| anyhow::anyhow!("CONDUIT_JOB_EXECUTOR_POLL_MS must be an integer"))?,
            job_executor_batch_size: optional_env("CONDUIT_JOB_EXECUTOR_BATCH_SIZE", "10")
                .parse()
                .map_err(|_| {
                    anyhow::anyhow!("CONDUIT_JOB_EXECUTOR_BATCH_SIZE must be an integer")
                })?,
            job_lock_duration_secs: optional_env("CONDUIT_JOB_LOCK_DURATION_SECS", "30")
                .parse()
                .map_err(|_| {
                    anyhow::anyhow!("CONDUIT_JOB_LOCK_DURATION_SECS must be an integer")
                })?,

            secrets_key: load_secrets_key()?,

            jwt_signing_key: require_env("CONDUIT_JWT_SIGNING_KEY")?,
            jwt_ttl_seconds: optional_env("CONDUIT_JWT_TTL_SECONDS", "3600")
                .parse()
                .map_err(|_| anyhow::anyhow!("CONDUIT_JWT_TTL_SECONDS must be an integer"))?,
            jwt_issuer: optional_env("CONDUIT_JWT_ISSUER", "conduit"),

            tenant_isolation,

            bootstrap_admin_email: std::env::var("CONDUIT_BOOTSTRAP_ADMIN_EMAIL").ok(),
            bootstrap_admin_password: std::env::var("CONDUIT_BOOTSTRAP_ADMIN_PASSWORD").ok(),
            bootstrap_admin_org_slug: std::env::var("CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG").ok(),
        })
    }
}

/// Decode the master encryption key for the `secrets` table from
/// `CONDUIT_SECRETS_KEY` (base64, 32 raw bytes). Generate one in dev with:
///     `openssl rand -base64 32`
fn load_secrets_key() -> anyhow::Result<[u8; 32]> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = require_env("CONDUIT_SECRETS_KEY")?;
    let bytes = STANDARD
        .decode(raw.trim())
        .map_err(|e| anyhow::anyhow!("CONDUIT_SECRETS_KEY is not valid base64: {e}"))?;
    if bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "CONDUIT_SECRETS_KEY must decode to 32 bytes, got {}",
            bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn require_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key)
        .map_err(|_| anyhow::anyhow!("Required environment variable '{}' is not set", key))
}

fn optional_env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// 32 zero bytes, base64-encoded. Suitable for tests; never use in prod.
    const TEST_KEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const TEST_JWT_KEY: &str = "test-jwt-signing-key-do-not-use-in-prod";

    fn clear_required_env() {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("CONDUIT_SECRETS_KEY");
        std::env::remove_var("CONDUIT_JWT_SIGNING_KEY");
        std::env::remove_var("CONDUIT_TENANT_ISOLATION");
    }

    #[test]
    fn config_fails_without_database_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("CONDUIT_SECRETS_KEY", TEST_KEY_B64);
        std::env::set_var("CONDUIT_JWT_SIGNING_KEY", TEST_JWT_KEY);
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DATABASE_URL"));
        clear_required_env();
    }

    #[test]
    fn config_fails_without_secrets_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::set_var("CONDUIT_JWT_SIGNING_KEY", TEST_JWT_KEY);
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CONDUIT_SECRETS_KEY"));
        clear_required_env();
    }

    #[test]
    fn config_fails_without_jwt_signing_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::set_var("CONDUIT_SECRETS_KEY", TEST_KEY_B64);
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CONDUIT_JWT_SIGNING_KEY"));
        clear_required_env();
    }

    #[test]
    fn config_rejects_short_secrets_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::set_var("CONDUIT_SECRETS_KEY", "dG9vc2hvcnQ="); // "tooshort"
        std::env::set_var("CONDUIT_JWT_SIGNING_KEY", TEST_JWT_KEY);
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("32 bytes"));
        clear_required_env();
    }

    #[test]
    fn config_rejects_invalid_tenant_isolation() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::set_var("CONDUIT_SECRETS_KEY", TEST_KEY_B64);
        std::env::set_var("CONDUIT_JWT_SIGNING_KEY", TEST_JWT_KEY);
        std::env::set_var("CONDUIT_TENANT_ISOLATION", "neither");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CONDUIT_TENANT_ISOLATION"));
        clear_required_env();
    }

    #[test]
    fn config_uses_defaults_for_optional_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_required_env();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::set_var("CONDUIT_SECRETS_KEY", TEST_KEY_B64);
        std::env::set_var("CONDUIT_JWT_SIGNING_KEY", TEST_JWT_KEY);
        std::env::remove_var("CONDUIT_SERVER_PORT");
        std::env::remove_var("CONDUIT_LOG_LEVEL");

        let config = Config::from_env().unwrap();
        assert_eq!(config.server_port, 8080);
        assert_eq!(config.log_level, "info");
        assert_eq!(config.tenant_isolation, TenantIsolation::Multi);
        assert_eq!(config.jwt_ttl_seconds, 3600);
        assert_eq!(config.jwt_issuer, "conduit");

        clear_required_env();
    }
}
