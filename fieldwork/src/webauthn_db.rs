//! Database operations for WebAuthn credentials and challenges.
//!
//! Pure CRUD operations against the canonical `webauthn_credentials` and
//! `webauthn_challenges` tables defined in [`crate::db`]. Each function
//! dispatches on [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;

/// Store a WebAuthn credential.
pub async fn store_credential(
    pool: &Pool,
    id: i64,
    user_id: &str,
    credential_json: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO webauthn_credentials (id, user_id, credential_json, created_at) \
                 VALUES ($1, $2::uuid, $3, $4)",
            )
            .bind(id)
            .bind(user_id)
            .bind(credential_json)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO webauthn_credentials (id, user_id, credential_json, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(id)
            .bind(user_id)
            .bind(credential_json)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all credentials for a user. Returns (id, credential_json) pairs.
pub async fn get_credentials(
    pool: &Pool,
    user_id: &str,
) -> Result<Vec<(i64, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, credential_json FROM webauthn_credentials \
                 WHERE user_id = $1::uuid ORDER BY id",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, credential_json FROM webauthn_credentials \
                 WHERE user_id = ?1 ORDER BY id",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await
        }
    }
}

/// Delete a credential by ID.
pub async fn delete_credential(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM webauthn_credentials WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM webauthn_credentials WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Store a WebAuthn challenge.
pub async fn store_challenge(
    pool: &Pool,
    challenge_id: &str,
    state_json: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO webauthn_challenges (challenge_id, state_json, created_at) \
                 VALUES ($1, $2, $3)",
            )
            .bind(challenge_id)
            .bind(state_json)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO webauthn_challenges (challenge_id, state_json, created_at) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(challenge_id)
            .bind(state_json)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a challenge's state JSON by ID.
pub async fn get_challenge(
    pool: &Pool,
    challenge_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT state_json FROM webauthn_challenges WHERE challenge_id = $1",
            )
            .bind(challenge_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(|r| r.0))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT state_json FROM webauthn_challenges WHERE challenge_id = ?1",
            )
            .bind(challenge_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(|r| r.0))
        }
    }
}

/// Delete a challenge by ID.
pub async fn delete_challenge(pool: &Pool, challenge_id: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM webauthn_challenges WHERE challenge_id = $1")
                .bind(challenge_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM webauthn_challenges WHERE challenge_id = ?1")
                .bind(challenge_id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Purge challenges older than `before` timestamp. Returns count of deleted rows.
pub async fn purge_old_challenges(pool: &Pool, before: i64) -> Result<u64, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let result = sqlx::query("DELETE FROM webauthn_challenges WHERE created_at < $1")
                .bind(before)
                .execute(pg)
                .await?;
            Ok(result.rows_affected())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let result = sqlx::query("DELETE FROM webauthn_challenges WHERE created_at < ?1")
                .bind(before)
                .execute(sq)
                .await?;
            Ok(result.rows_affected())
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
             VALUES ('u1', 'test@example.com', 'Test', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn store_and_get_credential() {
        let pool = setup().await;
        store_credential(&pool, 1, "u1", r#"{"cred":"data"}"#, 2000)
            .await
            .unwrap();

        let creds = get_credentials(&pool, "u1").await.unwrap();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].0, 1);
        assert_eq!(creds[0].1, r#"{"cred":"data"}"#);
    }

    #[tokio::test]
    async fn delete_credential_works() {
        let pool = setup().await;
        store_credential(&pool, 1, "u1", r#"{"a":1}"#, 2000).await.unwrap();
        delete_credential(&pool, 1).await.unwrap();
        let creds = get_credentials(&pool, "u1").await.unwrap();
        assert!(creds.is_empty());
    }

    #[tokio::test]
    async fn no_credentials_returns_empty() {
        let pool = setup().await;
        let creds = get_credentials(&pool, "u1").await.unwrap();
        assert!(creds.is_empty());
    }

    #[tokio::test]
    async fn store_and_get_challenge() {
        let pool = setup().await;
        store_challenge(&pool, "ch-1", r#"{"state":"pending"}"#, 3000)
            .await
            .unwrap();

        let state = get_challenge(&pool, "ch-1").await.unwrap();
        assert_eq!(state.as_deref(), Some(r#"{"state":"pending"}"#));
    }

    #[tokio::test]
    async fn delete_challenge_works() {
        let pool = setup().await;
        store_challenge(&pool, "ch-1", "{}", 3000).await.unwrap();
        delete_challenge(&pool, "ch-1").await.unwrap();
        assert!(get_challenge(&pool, "ch-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_nonexistent_challenge_returns_none() {
        let pool = setup().await;
        assert!(get_challenge(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn purge_old_challenges_removes_expired() {
        let pool = setup().await;
        store_challenge(&pool, "old", "{}", 1000).await.unwrap();
        store_challenge(&pool, "new", "{}", 5000).await.unwrap();

        let purged = purge_old_challenges(&pool, 3000).await.unwrap();
        assert_eq!(purged, 1);

        assert!(get_challenge(&pool, "old").await.unwrap().is_none());
        assert!(get_challenge(&pool, "new").await.unwrap().is_some());
    }
}
