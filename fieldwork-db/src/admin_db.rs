//! Database operations for admin tokens and authentication.
//!
//! Pure CRUD operations against the canonical `admin_tokens` table
//! defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// Create an admin token.
pub async fn create_admin_token(
    pool: &Pool,
    id: i64,
    token_hash: &str,
    name: &str,
    now: i64,
    expires_at: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO admin_tokens (id, token_hash, name, created_at, expires_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(token_hash)
            .bind(name)
            .bind(now)
            .bind(expires_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO admin_tokens (id, token_hash, name, created_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(id)
            .bind(token_hash)
            .bind(name)
            .bind(now)
            .bind(expires_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Verify an admin token by hash. Returns (id, name) if the token is valid
/// (not expired, not revoked).
pub async fn verify_admin_token(
    pool: &Pool,
    token_hash: &str,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, name FROM admin_tokens \
                 WHERE token_hash = $1 AND revoked_at IS NULL \
                 AND (expires_at IS NULL OR expires_at > $2)",
            )
            .bind(token_hash)
            .bind(now_epoch())
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, name FROM admin_tokens \
                 WHERE token_hash = ?1 AND revoked_at IS NULL \
                 AND (expires_at IS NULL OR expires_at > ?2)",
            )
            .bind(token_hash)
            .bind(now_epoch())
            .fetch_optional(sq)
            .await
        }
    }
}

/// Revoke an admin token by ID.
pub async fn revoke_admin_token(pool: &Pool, id: i64, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE admin_tokens SET revoked_at = $1 WHERE id = $2")
                .bind(now)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE admin_tokens SET revoked_at = ?1 WHERE id = ?2")
                .bind(now)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// List all admin tokens. Returns (id, name, created_at, expires_at, revoked_at).
pub async fn list_admin_tokens(
    pool: &Pool,
) -> Result<Vec<(i64, String, i64, Option<i64>, Option<i64>)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, name, created_at, expires_at, revoked_at \
                 FROM admin_tokens ORDER BY created_at",
            )
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, name, created_at, expires_at, revoked_at \
                 FROM admin_tokens ORDER BY created_at",
            )
            .fetch_all(sq)
            .await
        }
    }
}

/// Current Unix epoch in seconds (for expiry comparison).
fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
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
        pool
    }

    #[tokio::test]
    async fn create_and_verify() {
        let pool = setup().await;
        create_admin_token(&pool, 1, "hash-abc", "deploy-key", 1000, None)
            .await
            .unwrap();

        let result = verify_admin_token(&pool, "hash-abc").await.unwrap();
        let (id, name) = result.expect("token should be valid");
        assert_eq!(id, 1);
        assert_eq!(name, "deploy-key");
    }

    #[tokio::test]
    async fn verify_nonexistent_returns_none() {
        let pool = setup().await;
        let result = verify_admin_token(&pool, "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn revoked_token_not_verified() {
        let pool = setup().await;
        create_admin_token(&pool, 1, "hash-abc", "key", 1000, None)
            .await
            .unwrap();
        revoke_admin_token(&pool, 1, 2000).await.unwrap();

        let result = verify_admin_token(&pool, "hash-abc").await.unwrap();
        assert!(result.is_none(), "revoked token should not verify");
    }

    #[tokio::test]
    async fn expired_token_not_verified() {
        let pool = setup().await;
        // expires_at = 1 second (well in the past)
        create_admin_token(&pool, 1, "hash-abc", "key", 1000, Some(1))
            .await
            .unwrap();

        let result = verify_admin_token(&pool, "hash-abc").await.unwrap();
        assert!(result.is_none(), "expired token should not verify");
    }

    #[tokio::test]
    async fn list_tokens() {
        let pool = setup().await;
        create_admin_token(&pool, 1, "h1", "first", 1000, None).await.unwrap();
        create_admin_token(&pool, 2, "h2", "second", 2000, Some(9999))
            .await
            .unwrap();

        let tokens = list_admin_tokens(&pool).await.unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].1, "first");
        assert!(tokens[0].3.is_none()); // no expires_at
        assert_eq!(tokens[1].3, Some(9999));
    }

    #[tokio::test]
    async fn revoke_nonexistent_is_ok() {
        let pool = setup().await;
        revoke_admin_token(&pool, 999, 3000).await.unwrap();
    }
}
