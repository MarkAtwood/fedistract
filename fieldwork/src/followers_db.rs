//! Database operations for inbound followers (remote actors following local personas).
//!
//! Pure CRUD operations against the canonical `followers` and `remote_accounts`
//! tables defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// Add an inbound follower. Silently ignores duplicates.
pub async fn add_follower(
    pool: &Pool,
    persona_id: &str,
    user_id: &str,
    remote_account_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO followers (persona_id, user_id, remote_account_id, accepted_at) \
                 VALUES ($1::uuid, $2::uuid, $3, $4) ON CONFLICT DO NOTHING",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(remote_account_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO followers (persona_id, user_id, remote_account_id, accepted_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(remote_account_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove an inbound follower.
pub async fn remove_follower(
    pool: &Pool,
    persona_id: &str,
    remote_account_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM followers WHERE persona_id = $1::uuid AND remote_account_id = $2",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM followers WHERE persona_id = ?1 AND remote_account_id = ?2",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check whether a remote account is following a local persona.
pub async fn is_following(
    pool: &Pool,
    persona_id: &str,
    remote_account_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM followers \
                 WHERE persona_id = $1::uuid AND remote_account_id = $2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM followers \
                 WHERE persona_id = ?1 AND remote_account_id = ?2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
}

/// Count inbound followers for a persona.
pub async fn follower_count(pool: &Pool, persona_id: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::bigint FROM followers WHERE persona_id = $1::uuid",
            )
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM followers WHERE persona_id = ?1",
            )
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0)
}

/// Get inbox URLs for all followers of a persona (for delivery fan-out).
///
/// Prefers `shared_inbox_url` when available, falling back to `inbox_url`.
/// Deduplicates shared inboxes so each URL appears at most once.
pub async fn follower_inboxes(
    pool: &Pool,
    persona_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT DISTINCT COALESCE(ra.shared_inbox_url, ra.inbox_url) \
                 FROM followers f \
                 JOIN remote_accounts ra ON ra.id = f.remote_account_id \
                 WHERE f.persona_id = $1::uuid",
            )
            .bind(persona_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(|r| r.0).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT DISTINCT COALESCE(ra.shared_inbox_url, ra.inbox_url) \
                 FROM followers f \
                 JOIN remote_accounts ra ON ra.id = f.remote_account_id \
                 WHERE f.persona_id = ?1",
            )
            .bind(persona_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(|r| r.0).collect())
        }
    }
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
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, shared_inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/bob', 'bob', 'remote', 'Bob', \
                     'pk', 'https://remote/users/bob#key', 'https://remote/inbox', \
                     'https://remote/shared-inbox', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (2, 'https://other/users/carol', 'carol', 'other', 'Carol', \
                     'pk2', 'https://other/users/carol#key', 'https://other/inbox', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn add_and_check_follower() {
        let pool = setup().await;
        add_follower(&pool, "p1", "u1", 1, 2000).await.unwrap();

        assert!(is_following(&pool, "p1", 1).await.unwrap());
        assert!(!is_following(&pool, "p1", 2).await.unwrap());
    }

    #[tokio::test]
    async fn add_follower_is_idempotent() {
        let pool = setup().await;
        add_follower(&pool, "p1", "u1", 1, 2000).await.unwrap();
        add_follower(&pool, "p1", "u1", 1, 3000).await.unwrap();
        assert_eq!(follower_count(&pool, "p1").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn remove_follower_deletes() {
        let pool = setup().await;
        add_follower(&pool, "p1", "u1", 1, 2000).await.unwrap();
        remove_follower(&pool, "p1", 1).await.unwrap();
        assert!(!is_following(&pool, "p1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn remove_nonexistent_is_ok() {
        let pool = setup().await;
        remove_follower(&pool, "p1", 999).await.unwrap();
    }

    #[tokio::test]
    async fn follower_count_works() {
        let pool = setup().await;
        assert_eq!(follower_count(&pool, "p1").await.unwrap(), 0);
        add_follower(&pool, "p1", "u1", 1, 2000).await.unwrap();
        add_follower(&pool, "p1", "u1", 2, 2000).await.unwrap();
        assert_eq!(follower_count(&pool, "p1").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn follower_inboxes_deduplicates_shared() {
        let pool = setup().await;
        add_follower(&pool, "p1", "u1", 1, 2000).await.unwrap();
        add_follower(&pool, "p1", "u1", 2, 2000).await.unwrap();

        let inboxes = follower_inboxes(&pool, "p1").await.unwrap();
        // Account 1 has shared_inbox_url, account 2 does not
        assert!(inboxes.contains(&"https://remote/shared-inbox".to_string()));
        assert!(inboxes.contains(&"https://other/inbox".to_string()));
        assert_eq!(inboxes.len(), 2);
    }

    #[tokio::test]
    async fn follower_inboxes_empty() {
        let pool = setup().await;
        let inboxes = follower_inboxes(&pool, "p1").await.unwrap();
        assert!(inboxes.is_empty());
    }
}
