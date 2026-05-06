use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Fixed advisory lock key identifying the Conduit leader slot.
const LEADER_LOCK_KEY: i64 = 0x1337_C0AD_0001_i64;

/// PostgreSQL session-level advisory lock-based leader election.
///
/// One replica holds the advisory lock at a time; the others poll and take
/// over if the holder disappears. `is_leader()` returns the cached result and
/// is safe to call from any thread.
pub struct LeaderElector {
    is_leader: Arc<AtomicBool>,
}

impl LeaderElector {
    /// Spawn the election loop and return a handle. The loop runs until
    /// `token` is cancelled; it holds a dedicated connection (not from the
    /// pool) so the session-level lock stays alive.
    pub async fn start(pool: PgPool, token: CancellationToken) -> Self {
        let is_leader = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&is_leader);

        tokio::spawn(async move {
            // Dedicated connection — session advisory locks are tied to the
            // connection, so we must not return it to the pool between polls.
            let conn = match pool.acquire().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "LeaderElector: failed to acquire connection");
                    return;
                }
            };
            // Detach the connection so it doesn't go back to the pool on drop.
            let mut conn = conn.detach();

            loop {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => break,
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {}
                }

                let result: Result<(bool,), _> = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
                    .bind(LEADER_LOCK_KEY)
                    .fetch_one(&mut conn)
                    .await;

                match result {
                    Ok((acquired,)) => {
                        let was_leader = flag.swap(acquired, Ordering::Relaxed);
                        if acquired && !was_leader {
                            tracing::info!("This instance became the leader");
                        } else if !acquired && was_leader {
                            tracing::warn!("This instance lost the leader role");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "LeaderElector: advisory lock query failed");
                        flag.store(false, Ordering::Relaxed);
                    }
                }
            }

            // Release the lock on clean shutdown so another replica can take
            // over immediately instead of waiting for the connection to close.
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(LEADER_LOCK_KEY)
                .execute(&mut conn)
                .await;

            flag.store(false, Ordering::Relaxed);
            tracing::info!("LeaderElector stopped");
        });

        Self { is_leader }
    }

    /// Returns `true` if this instance currently holds the leader lock.
    pub fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::Relaxed)
    }
}
