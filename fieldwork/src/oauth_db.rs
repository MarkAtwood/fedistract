//! Database operations for OAuth applications and bearer tokens.
//!
//! Pure CRUD operations against the canonical `oauth_apps` and `oauth_tokens`
//! tables defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// A row from the `oauth_apps` table.
#[derive(Debug, Clone)]
pub struct OAuthAppRow {
    pub id: i64,
    pub client_id: String,
    pub client_secret: String,
    pub name: String,
    pub website: Option<String>,
    pub redirect_uri: String,
    pub scopes: String,
    pub created_at: i64,
}

type AppTuple = (i64, String, String, String, Option<String>, String, String, i64);

fn tuple_to_app(t: AppTuple) -> OAuthAppRow {
    OAuthAppRow {
        id: t.0,
        client_id: t.1,
        client_secret: t.2,
        name: t.3,
        website: t.4,
        redirect_uri: t.5,
        scopes: t.6,
        created_at: t.7,
    }
}

/// Register an OAuth application.
pub async fn create_app(pool: &Pool, row: &OAuthAppRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO oauth_apps \
                 (id, client_id, client_secret, name, website, redirect_uri, scopes, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(row.id)
            .bind(&row.client_id)
            .bind(&row.client_secret)
            .bind(&row.name)
            .bind(&row.website)
            .bind(&row.redirect_uri)
            .bind(&row.scopes)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO oauth_apps \
                 (id, client_id, client_secret, name, website, redirect_uri, scopes, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(row.id)
            .bind(&row.client_id)
            .bind(&row.client_secret)
            .bind(&row.name)
            .bind(&row.website)
            .bind(&row.redirect_uri)
            .bind(&row.scopes)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Look up an OAuth app by client_id.
pub async fn get_app_by_client_id(
    pool: &Pool,
    client_id: &str,
) -> Result<Option<OAuthAppRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: Option<AppTuple> = sqlx::query_as(
                "SELECT id, client_id, client_secret, name, website, \
                 redirect_uri, scopes, created_at \
                 FROM oauth_apps WHERE client_id = $1",
            )
            .bind(client_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_app))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<AppTuple> = sqlx::query_as(
                "SELECT id, client_id, client_secret, name, website, \
                 redirect_uri, scopes, created_at \
                 FROM oauth_apps WHERE client_id = ?1",
            )
            .bind(client_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_app))
        }
    }
}

/// Create a bearer token.
pub async fn create_token(
    pool: &Pool,
    id: i64,
    token_hash: &str,
    app_id: i64,
    user_id: i64,
    persona_id: i64,
    scopes: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO oauth_tokens \
                 (id, token_hash, app_id, user_id, persona_id, scopes, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(id)
            .bind(token_hash)
            .bind(app_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(scopes)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO oauth_tokens \
                 (id, token_hash, app_id, user_id, persona_id, scopes, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(id)
            .bind(token_hash)
            .bind(app_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(scopes)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Verify a token by its hash. Returns `(persona_id, username, scopes)` if valid.
///
/// A token is valid if it exists, is not revoked, and has an associated persona.
pub async fn verify_token(
    pool: &Pool,
    token_hash: &str,
) -> Result<Option<(i64, String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: Option<(i64, String, String)> = sqlx::query_as(
                "SELECT t.persona_id, p.username, t.scopes \
                 FROM oauth_tokens t \
                 JOIN personas p ON p.id = t.persona_id \
                 WHERE t.token_hash = $1 AND t.revoked_at IS NULL",
            )
            .bind(token_hash)
            .fetch_optional(pg)
            .await?;
            Ok(row)
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<(i64, String, String)> = sqlx::query_as(
                "SELECT t.persona_id, p.username, t.scopes \
                 FROM oauth_tokens t \
                 JOIN personas p ON p.id = t.persona_id \
                 WHERE t.token_hash = ?1 AND t.revoked_at IS NULL",
            )
            .bind(token_hash)
            .fetch_optional(sq)
            .await?;
            Ok(row)
        }
    }
}

/// Insert an OAuth authorization code.
pub async fn insert_authz_code(
    pool: &Pool,
    code_hash: &str,
    app_id: i64,
    user_id: i64,
    persona_id: i64,
    scopes: &str,
    redirect_uri: &str,
    now: i64,
    expires_at: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO oauth_authz_codes \
                 (code_hash, app_id, user_id, persona_id, scopes, redirect_uri, created_at, expires_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(code_hash)
            .bind(app_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(scopes)
            .bind(redirect_uri)
            .bind(now)
            .bind(expires_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO oauth_authz_codes \
                 (code_hash, app_id, user_id, persona_id, scopes, redirect_uri, created_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(code_hash)
            .bind(app_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(scopes)
            .bind(redirect_uri)
            .bind(now)
            .bind(expires_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Consume (atomically fetch and delete) an OAuth authorization code.
///
/// Returns `(app_id, persona_id, scopes, redirect_uri)` if the code exists
/// and has not expired. Deletes the code row atomically.
pub async fn consume_authz_code(
    pool: &Pool,
    code_hash: &str,
    now: i64,
) -> Result<Option<(i64, i64, String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "DELETE FROM oauth_authz_codes \
                 WHERE code_hash = $1 AND expires_at > $2 \
                 RETURNING app_id, persona_id, scopes, redirect_uri",
            )
            .bind(code_hash)
            .bind(now)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "DELETE FROM oauth_authz_codes \
                 WHERE code_hash = ?1 AND expires_at > ?2 \
                 RETURNING app_id, persona_id, scopes, redirect_uri",
            )
            .bind(code_hash)
            .bind(now)
            .fetch_optional(sq)
            .await
        }
    }
}

/// Update the `last_used_at` timestamp for a token by its hash. Best-effort.
pub async fn touch_token(
    pool: &Pool,
    token_hash: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE oauth_tokens SET last_used_at = $1 WHERE token_hash = $2",
            )
            .bind(now)
            .bind(token_hash)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE oauth_tokens SET last_used_at = ?1 WHERE token_hash = ?2",
            )
            .bind(now)
            .bind(token_hash)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Revoke a token by its hash.
pub async fn revoke_by_hash(
    pool: &Pool,
    token_hash: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = $1 \
                 WHERE token_hash = $2 AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(token_hash)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = ?1 \
                 WHERE token_hash = ?2 AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(token_hash)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Revoke all tokens for a persona. Returns the number of tokens revoked.
pub async fn revoke_all_for_persona(
    pool: &Pool,
    persona_id: i64,
    now: i64,
) -> Result<u64, sqlx::Error> {
    let rows_affected = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = $1 \
                 WHERE persona_id = $2 AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(persona_id)
            .execute(pg)
            .await?
            .rows_affected()
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = ?1 \
                 WHERE persona_id = ?2 AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(persona_id)
            .execute(sq)
            .await?
            .rows_affected()
        }
    };
    Ok(rows_affected)
}

/// Revoke a token by its ID.
pub async fn revoke_token(pool: &Pool, token_id: i64, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = $1 WHERE id = $2",
            )
            .bind(now)
            .bind(token_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE oauth_tokens SET revoked_at = ?1 WHERE id = ?2",
            )
            .bind(now)
            .bind(token_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
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
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_app() -> OAuthAppRow {
        OAuthAppRow {
            id: 1,
            client_id: "client-abc".into(),
            client_secret: "secret-xyz".into(),
            name: "Test App".into(),
            website: Some("https://example.com".into()),
            redirect_uri: "urn:ietf:wg:oauth:2.0:oob".into(),
            scopes: "read write".into(),
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn create_and_get_app() {
        let pool = setup().await;
        let app = sample_app();
        create_app(&pool, &app).await.unwrap();

        let fetched = get_app_by_client_id(&pool, "client-abc")
            .await
            .unwrap()
            .expect("app should exist");
        assert_eq!(fetched.id, 1);
        assert_eq!(fetched.name, "Test App");
        assert_eq!(fetched.scopes, "read write");
        assert_eq!(fetched.website.as_deref(), Some("https://example.com"));
    }

    #[tokio::test]
    async fn get_nonexistent_app_returns_none() {
        let pool = setup().await;
        let result = get_app_by_client_id(&pool, "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn create_and_verify_token() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();
        create_token(&pool, 10, "hash-abc", 1, 1, 101, "read write", 2000)
            .await
            .unwrap();

        let result = verify_token(&pool, "hash-abc").await.unwrap();
        let (persona_id, username, scopes) = result.expect("token should be valid");
        assert_eq!(persona_id, 101);
        assert_eq!(username, "alice");
        assert_eq!(scopes, "read write");
    }

    #[tokio::test]
    async fn verify_nonexistent_token_returns_none() {
        let pool = setup().await;
        let result = verify_token(&pool, "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn revoked_token_not_verified() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();
        create_token(&pool, 10, "hash-abc", 1, 1, 101, "read", 2000)
            .await
            .unwrap();

        revoke_token(&pool, 10, 3000).await.unwrap();

        let result = verify_token(&pool, "hash-abc").await.unwrap();
        assert!(result.is_none(), "revoked token should not verify");
    }

    #[tokio::test]
    async fn revoke_nonexistent_is_ok() {
        let pool = setup().await;
        revoke_token(&pool, 999, 3000).await.unwrap();
    }

    #[tokio::test]
    async fn insert_and_consume_authz_code() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();

        insert_authz_code(&pool, "code-hash", 1, 1, 101, "read", "urn:ietf:wg:oauth:2.0:oob", 1000, 2000)
            .await
            .unwrap();

        let result = consume_authz_code(&pool, "code-hash", 1500).await.unwrap();
        let (app_id, persona_id, scopes, redirect) = result.expect("code should exist");
        assert_eq!(app_id, 1);
        assert_eq!(persona_id, 101);
        assert_eq!(scopes, "read");
        assert_eq!(redirect, "urn:ietf:wg:oauth:2.0:oob");

        // Second consume should return None (code was deleted)
        assert!(consume_authz_code(&pool, "code-hash", 1500).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn consume_expired_code_returns_none() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();

        insert_authz_code(&pool, "code-exp", 1, 1, 101, "read", "http://redir", 1000, 1500)
            .await
            .unwrap();

        assert!(consume_authz_code(&pool, "code-exp", 2000).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn touch_token_updates_last_used() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();
        create_token(&pool, 10, "hash-touch", 1, 1, 101, "read", 2000)
            .await
            .unwrap();

        touch_token(&pool, "hash-touch", 5000).await.unwrap();
        // Verify the token still works (touch doesn't break it)
        let result = verify_token(&pool, "hash-touch").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn revoke_by_hash_works() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();
        create_token(&pool, 10, "hash-revoke", 1, 1, 101, "read", 2000)
            .await
            .unwrap();

        revoke_by_hash(&pool, "hash-revoke", 5000).await.unwrap();
        assert!(verify_token(&pool, "hash-revoke").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoke_all_for_persona_works() {
        let pool = setup().await;
        create_app(&pool, &sample_app()).await.unwrap();
        create_token(&pool, 10, "hash-a", 1, 1, 101, "read", 2000).await.unwrap();
        create_token(&pool, 11, "hash-b", 1, 1, 101, "read", 2000).await.unwrap();

        let count = revoke_all_for_persona(&pool, 101, 5000).await.unwrap();
        assert_eq!(count, 2);

        assert!(verify_token(&pool, "hash-a").await.unwrap().is_none());
        assert!(verify_token(&pool, "hash-b").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoke_all_for_persona_returns_zero_when_none() {
        let pool = setup().await;
        let count = revoke_all_for_persona(&pool, 101, 5000).await.unwrap();
        assert_eq!(count, 0);
    }
}
