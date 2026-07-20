//! Database operations for outbound follows (local personas following others).
//!
//! Pure CRUD operations against the canonical `follows` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.

use crate::db::Pool;

/// Follow a remote account. Silently ignores duplicates.
pub async fn follow_remote(
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
                "INSERT INTO follows (persona_id, user_id, followee_remote_id, created_at) \
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
                "INSERT OR IGNORE INTO follows \
                 (persona_id, user_id, followee_remote_id, created_at) \
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

/// Follow a local persona. Silently ignores duplicates.
pub async fn follow_local(
    pool: &Pool,
    persona_id: &str,
    user_id: &str,
    target_persona_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO follows (persona_id, user_id, followee_persona_id, created_at) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4) ON CONFLICT DO NOTHING",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(target_persona_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO follows \
                 (persona_id, user_id, followee_persona_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(target_persona_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unfollow a remote account.
pub async fn unfollow_remote(
    pool: &Pool,
    persona_id: &str,
    remote_account_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM follows \
                 WHERE persona_id = $1::uuid AND followee_remote_id = $2",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM follows \
                 WHERE persona_id = ?1 AND followee_remote_id = ?2",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unfollow a local persona.
pub async fn unfollow_local(
    pool: &Pool,
    persona_id: &str,
    target_persona_id: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM follows \
                 WHERE persona_id = $1::uuid AND followee_persona_id = $2::uuid",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM follows \
                 WHERE persona_id = ?1 AND followee_persona_id = ?2",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check whether a persona is following a remote account.
pub async fn is_following_remote(
    pool: &Pool,
    persona_id: &str,
    remote_account_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM follows \
                 WHERE persona_id = $1::uuid AND followee_remote_id = $2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM follows \
                 WHERE persona_id = ?1 AND followee_remote_id = ?2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(remote_account_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
}

/// Count outbound follows for a persona (both remote and local).
pub async fn following_count(pool: &Pool, persona_id: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::bigint FROM follows WHERE persona_id = $1::uuid",
            )
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM follows WHERE persona_id = ?1",
            )
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0)
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
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES ('p2', 'u1', 'bob', 'pk2', 'pub2', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/carol', 'carol', 'remote', 'Carol', \
                     'pk', 'https://remote/users/carol#key', 'https://remote/inbox', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn follow_and_check_remote() {
        let pool = setup().await;
        follow_remote(&pool, "p1", "u1", 1, 2000).await.unwrap();
        assert!(is_following_remote(&pool, "p1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn follow_remote_is_idempotent() {
        let pool = setup().await;
        follow_remote(&pool, "p1", "u1", 1, 2000).await.unwrap();
        follow_remote(&pool, "p1", "u1", 1, 3000).await.unwrap();
        assert_eq!(following_count(&pool, "p1").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn follow_and_check_local() {
        let pool = setup().await;
        follow_local(&pool, "p1", "u1", "p2", 2000).await.unwrap();
        assert_eq!(following_count(&pool, "p1").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn follow_local_is_idempotent() {
        let pool = setup().await;
        follow_local(&pool, "p1", "u1", "p2", 2000).await.unwrap();
        follow_local(&pool, "p1", "u1", "p2", 3000).await.unwrap();
        assert_eq!(following_count(&pool, "p1").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn unfollow_remote_deletes() {
        let pool = setup().await;
        follow_remote(&pool, "p1", "u1", 1, 2000).await.unwrap();
        unfollow_remote(&pool, "p1", 1).await.unwrap();
        assert!(!is_following_remote(&pool, "p1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn unfollow_local_deletes() {
        let pool = setup().await;
        follow_local(&pool, "p1", "u1", "p2", 2000).await.unwrap();
        unfollow_local(&pool, "p1", "p2").await.unwrap();
        assert_eq!(following_count(&pool, "p1").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn unfollow_nonexistent_is_ok() {
        let pool = setup().await;
        unfollow_remote(&pool, "p1", 999).await.unwrap();
        unfollow_local(&pool, "p1", "p2").await.unwrap();
    }

    #[tokio::test]
    async fn following_count_mixed() {
        let pool = setup().await;
        follow_remote(&pool, "p1", "u1", 1, 2000).await.unwrap();
        follow_local(&pool, "p1", "u1", "p2", 2000).await.unwrap();
        assert_eq!(following_count(&pool, "p1").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn not_following_returns_false() {
        let pool = setup().await;
        assert!(!is_following_remote(&pool, "p1", 1).await.unwrap());
    }
}
