/// Application configuration loaded from environment variables.
/// See .env.example for all available settings.
#[derive(Debug, Clone)]
pub struct Config {
    // Required
    pub database_url: String,

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
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env file if present (ignored if missing)
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: require_env("DATABASE_URL")?,

            server_host: optional_env("SERVER_HOST", "0.0.0.0"),
            server_port: optional_env("SERVER_PORT", "8080")
                .parse()
                .map_err(|_| anyhow::anyhow!("SERVER_PORT must be a valid port number"))?,

            log_level: optional_env("LOG_LEVEL", "info"),

            db_min_connections: optional_env("DB_MIN_CONNECTIONS", "2")
                .parse()
                .map_err(|_| anyhow::anyhow!("DB_MIN_CONNECTIONS must be an integer"))?,
            db_max_connections: optional_env("DB_MAX_CONNECTIONS", "10")
                .parse()
                .map_err(|_| anyhow::anyhow!("DB_MAX_CONNECTIONS must be an integer"))?,
            db_acquire_timeout_secs: optional_env("DB_ACQUIRE_TIMEOUT_SECS", "30")
                .parse()
                .map_err(|_| anyhow::anyhow!("DB_ACQUIRE_TIMEOUT_SECS must be an integer"))?,
            db_statement_timeout_ms: optional_env("DB_STATEMENT_TIMEOUT_MS", "30000")
                .parse()
                .map_err(|_| anyhow::anyhow!("DB_STATEMENT_TIMEOUT_MS must be an integer"))?,

            job_executor_poll_ms: optional_env("JOB_EXECUTOR_POLL_MS", "200")
                .parse()
                .map_err(|_| anyhow::anyhow!("JOB_EXECUTOR_POLL_MS must be an integer"))?,
            job_executor_batch_size: optional_env("JOB_EXECUTOR_BATCH_SIZE", "10")
                .parse()
                .map_err(|_| anyhow::anyhow!("JOB_EXECUTOR_BATCH_SIZE must be an integer"))?,
            job_lock_duration_secs: optional_env("JOB_LOCK_DURATION_SECS", "30")
                .parse()
                .map_err(|_| anyhow::anyhow!("JOB_LOCK_DURATION_SECS must be an integer"))?,
        })
    }
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

    #[test]
    fn config_fails_without_database_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("DATABASE_URL");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn config_uses_defaults_for_optional_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("DATABASE_URL", "postgres://test");
        std::env::remove_var("SERVER_PORT");
        std::env::remove_var("LOG_LEVEL");

        let config = Config::from_env().unwrap();
        assert_eq!(config.server_port, 8080);
        assert_eq!(config.log_level, "info");

        std::env::remove_var("DATABASE_URL");
    }
}
