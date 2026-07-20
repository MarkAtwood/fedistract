//! Database operations for idempotency keys (Mastodon Client API).
//!
//! Pure CRUD against the canonical `idempotency_keys` table.

use crate::db::Pool;

/// Check if an idempotency key has already been used. Returns the post_id if so.
pub async fn check_key(
    pool: &Pool,
    key_hash: &str,
    user_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let row: Option<(i64,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT post_id FROM idempotency_keys \
                 WHERE key_hash = $1 AND user_id = $2::uuid",
            )
            .bind(key_hash)
            .bind(user_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT post_id FROM idempotency_keys \
                 WHERE key_hash = ?1 AND user_id = ?2",
            )
            .bind(key_hash)
            .bind(user_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.map(|(pid,)| pid))
}

/// Store an idempotency key. Silently ignores duplicates.
pub async fn store_key(
    pool: &Pool,
    key_hash: &str,
    user_id: &str,
    post_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO idempotency_keys (key_hash, user_id, post_id, created_at) \
                 VALUES ($1, $2::uuid, $3, $4) ON CONFLICT DO NOTHING",
            )
            .bind(key_hash)
            .bind(user_id)
            .bind(post_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO idempotency_keys (key_hash, user_id, post_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(key_hash)
            .bind(user_id)
            .bind(post_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete idempotency keys created before the given timestamp.
/// Returns the number of rows deleted.
pub async fn purge_old(pool: &Pool, before: i64) -> Result<u64, sqlx::Error> {
    let rows = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM idempotency_keys WHERE created_at < $1")
                .bind(before)
                .execute(pg)
                .await?
                .rows_affected()
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM idempotency_keys WHERE created_at < ?1")
                .bind(before)
                .execute(sq)
                .await?
                .rows_affected()
        }
    };
    Ok(rows)
}

#[cfg(test)]
#[cfg(feature = "db-sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES ('u1', 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES ('p1', 'u1', 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 'u1', 'p1', 'https://example/posts/1', 'hello', '<p>hello</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 'u1', 'p1', 'https://example/posts/2', 'world', '<p>world</p>', 'public', 1001);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn check_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(check_key(&pool, "hash1", "u1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn store_and_check() {
        let pool = setup().await;
        store_key(&pool, "hash1", "u1", 1, 2000).await.unwrap();

        let post_id = check_key(&pool, "hash1", "u1").await.unwrap().unwrap();
        assert_eq!(post_id, 1);
    }

    #[tokio::test]
    async fn store_is_idempotent() {
        let pool = setup().await;
        store_key(&pool, "hash1", "u1", 1, 2000).await.unwrap();
        store_key(&pool, "hash1", "u1", 2, 3000).await.unwrap();

        // First insert wins
        let post_id = check_key(&pool, "hash1", "u1").await.unwrap().unwrap();
        assert_eq!(post_id, 1);
    }

    #[tokio::test]
    async fn purge_old_deletes_expired() {
        let pool = setup().await;
        store_key(&pool, "old1", "u1", 1, 1000).await.unwrap();
        store_key(&pool, "old2", "u1", 2, 1500).await.unwrap();
        store_key(&pool, "new1", "u1", 1, 3000).await.unwrap();

        let deleted = purge_old(&pool, 2000).await.unwrap();
        assert_eq!(deleted, 2);

        assert!(check_key(&pool, "old1", "u1").await.unwrap().is_none());
        assert!(check_key(&pool, "new1", "u1").await.unwrap().is_some());
    }
}
