//! Delivery queue database operations for outbound ActivityPub activities.
//!
//! Pure data-layer module: no HTTP, no signing, no background worker.
//! Consumer projects compose these with [`crate::retry`] and
//! [`crate::circuit_breaker`] to build their delivery workers.

use crate::db::Pool;
use crate::retry;

/// A row from the `delivery_queue` table.
#[derive(Debug, Clone)]
pub struct DeliveryJob {
    pub id: i64,
    pub target_inbox: String,
    pub sender_persona_id: i64,
    pub activity_json: String,
    pub attempts: i32,
    pub next_attempt_at: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
    pub dead_at: Option<i64>,
}

/// Insert a new delivery job. Returns the generated row ID.
///
/// Sets `next_attempt_at` and `created_at` to `now` (unix epoch seconds).
pub async fn enqueue(
    pool: &Pool,
    target_inbox: &str,
    sender_persona_id: i64,
    activity_json: &str,
    now: i64,
) -> Result<i64, sqlx::Error> {
    let id = crate::id::generate_id();
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO delivery_queue \
                 (id, target_inbox, sender_persona_id, activity_json, attempts, \
                  next_attempt_at, created_at) \
                 VALUES ($1, $2, $3, $4, 0, $5, $6)",
            )
            .bind(id)
            .bind(target_inbox)
            .bind(sender_persona_id)
            .bind(activity_json)
            .bind(now)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO delivery_queue \
                 (id, target_inbox, sender_persona_id, activity_json, attempts, \
                  next_attempt_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
            )
            .bind(id)
            .bind(target_inbox)
            .bind(sender_persona_id)
            .bind(activity_json)
            .bind(now)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(id)
}

/// Fetch pending jobs whose `next_attempt_at` is at or before `now`.
///
/// Returns at most `limit` rows, ordered by `next_attempt_at` ascending.
pub async fn fetch_pending(
    pool: &Pool,
    limit: i32,
    now: i64,
) -> Result<Vec<DeliveryJob>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<JobRow> =
                sqlx::query_as(
                    "SELECT id, target_inbox, sender_persona_id, activity_json, attempts, \
                     next_attempt_at, last_error, created_at, delivered_at, dead_at \
                     FROM delivery_queue \
                     WHERE delivered_at IS NULL AND dead_at IS NULL AND next_attempt_at <= $1 \
                     ORDER BY next_attempt_at \
                     LIMIT $2",
                )
                .bind(now)
                .bind(limit)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(row_to_job).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<JobRow> =
                sqlx::query_as(
                    "SELECT id, target_inbox, sender_persona_id, activity_json, attempts, \
                     next_attempt_at, last_error, created_at, delivered_at, dead_at \
                     FROM delivery_queue \
                     WHERE delivered_at IS NULL AND dead_at IS NULL AND next_attempt_at <= ?1 \
                     ORDER BY next_attempt_at \
                     LIMIT ?2",
                )
                .bind(now)
                .bind(limit)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(row_to_job).collect())
        }
    }
}

/// Mark a job as successfully delivered.
pub async fn mark_delivered(pool: &Pool, id: i64, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE delivery_queue SET delivered_at = $1 WHERE id = $2")
                .bind(now)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE delivery_queue SET delivered_at = ?1 WHERE id = ?2")
                .bind(now)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Mark a job as permanently failed.
pub async fn mark_dead(pool: &Pool, id: i64, error: &str, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE delivery_queue SET dead_at = $1, last_error = $2 WHERE id = $3",
            )
            .bind(now)
            .bind(error)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE delivery_queue SET dead_at = ?1, last_error = ?2 WHERE id = ?3",
            )
            .bind(now)
            .bind(error)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Schedule a retry: increment attempts, set next_attempt_at using the
/// [`retry`] backoff schedule, and record the error.
///
/// If attempts have reached [`retry::MAX_ATTEMPTS`], calls [`mark_dead`] instead.
pub async fn schedule_retry(
    pool: &Pool,
    id: i64,
    error: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    // Read current attempt count to compute the next delay.
    let attempts = current_attempts(pool, id).await?;
    let next_attempt = attempts + 1;

    if retry::exhausted(next_attempt) {
        return mark_dead(pool, id, error, now).await;
    }

    let next_at = now + retry::delay_ms(attempts) / 1000;

    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE delivery_queue \
                 SET attempts = $1, next_attempt_at = $2, last_error = $3 \
                 WHERE id = $4",
            )
            .bind(next_attempt)
            .bind(next_at)
            .bind(error)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE delivery_queue \
                 SET attempts = ?1, next_attempt_at = ?2, last_error = ?3 \
                 WHERE id = ?4",
            )
            .bind(next_attempt)
            .bind(next_at)
            .bind(error)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete completed or dead jobs created before `before` (unix epoch seconds).
///
/// Returns the number of rows deleted.
pub async fn purge_old(pool: &Pool, before: i64) -> Result<u64, sqlx::Error> {
    let rows_affected = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM delivery_queue \
                 WHERE (delivered_at IS NOT NULL OR dead_at IS NOT NULL) \
                 AND created_at < $1",
            )
            .bind(before)
            .execute(pg)
            .await?
            .rows_affected()
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM delivery_queue \
                 WHERE (delivered_at IS NOT NULL OR dead_at IS NOT NULL) \
                 AND created_at < ?1",
            )
            .bind(before)
            .execute(sq)
            .await?
            .rows_affected()
        }
    };
    Ok(rows_affected)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type JobRow = (i64, String, i64, String, i32, i64, Option<String>, i64, Option<i64>, Option<i64>);

fn row_to_job(row: JobRow) -> DeliveryJob {
    DeliveryJob {
        id: row.0,
        target_inbox: row.1,
        sender_persona_id: row.2,
        activity_json: row.3,
        attempts: row.4,
        next_attempt_at: row.5,
        last_error: row.6,
        created_at: row.7,
        delivered_at: row.8,
        dead_at: row.9,
    }
}

async fn current_attempts(pool: &Pool, id: i64) -> Result<i32, sqlx::Error> {
    let attempts: (i32,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT attempts FROM delivery_queue WHERE id = $1")
                .bind(id)
                .fetch_one(pg)
                .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT attempts FROM delivery_queue WHERE id = ?1")
                .bind(id)
                .fetch_one(sq)
                .await?
        }
    };
    Ok(attempts.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const NOW: i64 = 1_700_000_000;

    #[cfg(feature = "db-sqlite")]
    async fn test_pool() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();
        // Insert prerequisite user + persona so FK on sender_persona_id is satisfied.
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'test@example.com', 'Test', 'user', 1700000000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1700000000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn enqueue_and_fetch() {
        let pool = test_pool().await;

        let id = enqueue(&pool, "https://remote/inbox", 101, r#"{"type":"Create"}"#, NOW)
            .await
            .unwrap();
        assert!(id > 0);

        let jobs = fetch_pending(&pool, 10, NOW).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, id);
        assert_eq!(jobs[0].target_inbox, "https://remote/inbox");
        assert_eq!(jobs[0].sender_persona_id, 101);
        assert_eq!(jobs[0].activity_json, r#"{"type":"Create"}"#);
        assert_eq!(jobs[0].attempts, 0);
        assert_eq!(jobs[0].created_at, NOW);
        assert!(jobs[0].delivered_at.is_none());
        assert!(jobs[0].dead_at.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn fetch_pending_respects_time() {
        let pool = test_pool().await;

        enqueue(&pool, "https://remote/inbox", 101, "{}", NOW + 100)
            .await
            .unwrap();

        // Before next_attempt_at: no results
        let jobs = fetch_pending(&pool, 10, NOW).await.unwrap();
        assert!(jobs.is_empty());

        // At next_attempt_at: returns the job
        let jobs = fetch_pending(&pool, 10, NOW + 100).await.unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn mark_delivered_removes_from_pending() {
        let pool = test_pool().await;

        let id = enqueue(&pool, "https://remote/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        mark_delivered(&pool, id, NOW + 10).await.unwrap();

        let jobs = fetch_pending(&pool, 10, NOW + 100).await.unwrap();
        assert!(jobs.is_empty());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn mark_dead_removes_from_pending() {
        let pool = test_pool().await;

        let id = enqueue(&pool, "https://remote/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        mark_dead(&pool, id, "410 Gone", NOW + 10).await.unwrap();

        let jobs = fetch_pending(&pool, 10, NOW + 100).await.unwrap();
        assert!(jobs.is_empty());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn schedule_retry_increments_attempts() {
        let pool = test_pool().await;

        let id = enqueue(&pool, "https://remote/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        schedule_retry(&pool, id, "connection refused", NOW + 1).await.unwrap();

        // Job should not be fetchable at NOW+1 because next_attempt_at moved forward
        let jobs = fetch_pending(&pool, 10, NOW + 1).await.unwrap();
        assert!(jobs.is_empty());

        // Fetch at a time after the first retry delay (60s)
        let jobs = fetch_pending(&pool, 10, NOW + 1 + 61).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].attempts, 1);
        assert_eq!(jobs[0].last_error.as_deref(), Some("connection refused"));
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn schedule_retry_marks_dead_at_max() {
        let pool = test_pool().await;

        let id = enqueue(&pool, "https://remote/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        // Exhaust all retries
        for i in 0..retry::MAX_ATTEMPTS {
            schedule_retry(&pool, id, "server error", NOW + (i as i64) * 100_000)
                .await
                .unwrap();
        }

        // Should be dead now, not pending
        let jobs = fetch_pending(&pool, 10, NOW + 10_000_000).await.unwrap();
        assert!(jobs.is_empty());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn purge_old_deletes_completed() {
        let pool = test_pool().await;

        let id1 = enqueue(&pool, "https://a/inbox", 101, "{}", NOW)
            .await
            .unwrap();
        let id2 = enqueue(&pool, "https://b/inbox", 101, "{}", NOW)
            .await
            .unwrap();
        let _id3 = enqueue(&pool, "https://c/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        mark_delivered(&pool, id1, NOW + 1).await.unwrap();
        mark_dead(&pool, id2, "gone", NOW + 1).await.unwrap();

        // Purge jobs created before NOW + 1 (all three were created at NOW)
        let deleted = purge_old(&pool, NOW + 1).await.unwrap();
        // Only id1 (delivered) and id2 (dead) should be purged; id3 is still pending
        assert_eq!(deleted, 2);

        // id3 should still be fetchable
        let jobs = fetch_pending(&pool, 10, NOW + 100).await.unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn purge_old_skips_pending() {
        let pool = test_pool().await;

        enqueue(&pool, "https://a/inbox", 101, "{}", NOW)
            .await
            .unwrap();

        // Nothing is delivered or dead, so nothing to purge
        let deleted = purge_old(&pool, NOW + 1000).await.unwrap();
        assert_eq!(deleted, 0);
    }
}
