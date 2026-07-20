//! Database operations for invite codes.
//!
//! Pure CRUD operations against the canonical `invite_codes` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.

use crate::db::Pool;

/// A row from the `invite_codes` table.
#[derive(Debug, Clone)]
pub struct InviteCodeRow {
    pub code: String,
    pub created_by: Option<i64>,
    pub used_by: Option<i64>,
    pub max_uses: i32,
    pub uses: i32,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

type InviteCodeTuple = (String, Option<i64>, Option<i64>, i32, i32, Option<i64>, i64);

fn tuple_to_row(t: InviteCodeTuple) -> InviteCodeRow {
    InviteCodeRow {
        code: t.0,
        created_by: t.1,
        used_by: t.2,
        max_uses: t.3,
        uses: t.4,
        expires_at: t.5,
        created_at: t.6,
    }
}

/// Insert a new invite code.
pub async fn create_code(pool: &Pool, row: &InviteCodeRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO invite_codes (code, created_by, used_by, max_uses, uses, expires_at, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&row.code)
            .bind(&row.created_by)
            .bind(&row.used_by)
            .bind(row.max_uses)
            .bind(row.uses)
            .bind(row.expires_at)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO invite_codes (code, created_by, used_by, max_uses, uses, expires_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(&row.code)
            .bind(&row.created_by)
            .bind(&row.used_by)
            .bind(row.max_uses)
            .bind(row.uses)
            .bind(row.expires_at)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Look up an invite code by its code string.
pub async fn get_code(pool: &Pool, code: &str) -> Result<Option<InviteCodeRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query_as::<_, InviteCodeTuple>(
                "SELECT code, created_by, used_by, max_uses, uses, expires_at, created_at \
                 FROM invite_codes WHERE code = $1",
            )
            .bind(code)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_row))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query_as::<_, InviteCodeTuple>(
                "SELECT code, created_by, used_by, max_uses, uses, expires_at, created_at \
                 FROM invite_codes WHERE code = ?1",
            )
            .bind(code)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_row))
        }
    }
}

/// Attempt to use an invite code. Increments `uses`, sets `used_by` if
/// `max_uses == 1`. Returns `false` if the code is expired or exhausted.
pub async fn use_code(pool: &Pool, code: &str, user_id: i64, now: i64) -> Result<bool, sqlx::Error> {
    let row = match get_code(pool, code).await? {
        Some(r) => r,
        None => return Ok(false),
    };

    // Check expiry
    if let Some(expires) = row.expires_at {
        if now >= expires {
            return Ok(false);
        }
    }

    // Check exhaustion
    if row.uses >= row.max_uses {
        return Ok(false);
    }

    // Single-use codes record who used them
    if row.max_uses == 1 {
        match pool {
            #[cfg(feature = "postgres")]
            Pool::Postgres(pg) => {
                sqlx::query(
                    "UPDATE invite_codes SET uses = uses + 1, used_by = $1 WHERE code = $2",
                )
                .bind(user_id)
                .bind(code)
                .execute(pg)
                .await?;
            }
            #[cfg(feature = "sqlite")]
            Pool::Sqlite(sq) => {
                sqlx::query(
                    "UPDATE invite_codes SET uses = uses + 1, used_by = ?1 WHERE code = ?2",
                )
                .bind(user_id)
                .bind(code)
                .execute(sq)
                .await?;
            }
        }
    } else {
        match pool {
            #[cfg(feature = "postgres")]
            Pool::Postgres(pg) => {
                sqlx::query("UPDATE invite_codes SET uses = uses + 1 WHERE code = $1")
                    .bind(code)
                    .execute(pg)
                    .await?;
            }
            #[cfg(feature = "sqlite")]
            Pool::Sqlite(sq) => {
                sqlx::query("UPDATE invite_codes SET uses = uses + 1 WHERE code = ?1")
                    .bind(code)
                    .execute(sq)
                    .await?;
            }
        }
    }

    Ok(true)
}

/// List all invite codes.
pub async fn list_codes(pool: &Pool) -> Result<Vec<InviteCodeRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query_as::<_, InviteCodeTuple>(
                "SELECT code, created_by, used_by, max_uses, uses, expires_at, created_at \
                 FROM invite_codes ORDER BY created_at",
            )
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query_as::<_, InviteCodeTuple>(
                "SELECT code, created_by, used_by, max_uses, uses, expires_at, created_at \
                 FROM invite_codes ORDER BY created_at",
            )
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
    }
}

/// Delete an invite code by its code string.
pub async fn delete_code(pool: &Pool, code: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM invite_codes WHERE code = $1")
                .bind(code)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM invite_codes WHERE code = ?1")
                .bind(code)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
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
        // Create test users so FK constraints on used_by are satisfied.
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (10, 'user1@test.com', 'User1', 'user', 1000); \
             INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'u1@test.com', 'U1', 'user', 1000); \
             INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (2, 'u2@test.com', 'U2', 'user', 1000); \
             INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (3, 'u3@test.com', 'U3', 'user', 1000); \
             INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (4, 'u4@test.com', 'U4', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_code(code: &str) -> InviteCodeRow {
        InviteCodeRow {
            code: code.into(),
            created_by: None,
            used_by: None,
            max_uses: 1,
            uses: 0,
            expires_at: None,
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let pool = setup().await;
        let row = sample_code("ABC123");
        create_code(&pool, &row).await.unwrap();

        let fetched = get_code(&pool, "ABC123").await.unwrap().expect("should exist");
        assert_eq!(fetched.code, "ABC123");
        assert_eq!(fetched.max_uses, 1);
        assert_eq!(fetched.uses, 0);
        assert!(fetched.used_by.is_none());
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_code(&pool, "NOPE").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn use_single_use_code() {
        let pool = setup().await;
        create_code(&pool, &sample_code("SINGLE")).await.unwrap();

        let ok = use_code(&pool, "SINGLE", 10, 500).await.unwrap();
        assert!(ok);

        let fetched = get_code(&pool, "SINGLE").await.unwrap().unwrap();
        assert_eq!(fetched.uses, 1);
        assert_eq!(fetched.used_by, Some(10));

        // Second use should fail (exhausted)
        let ok = use_code(&pool, "SINGLE", 11, 500).await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn use_multi_use_code() {
        let pool = setup().await;
        let mut row = sample_code("MULTI");
        row.max_uses = 3;
        create_code(&pool, &row).await.unwrap();

        assert!(use_code(&pool, "MULTI", 1, 500).await.unwrap());
        assert!(use_code(&pool, "MULTI", 2, 500).await.unwrap());
        assert!(use_code(&pool, "MULTI", 3, 500).await.unwrap());
        // Fourth use should fail
        assert!(!use_code(&pool, "MULTI", 4, 500).await.unwrap());

        let fetched = get_code(&pool, "MULTI").await.unwrap().unwrap();
        assert_eq!(fetched.uses, 3);
        // Multi-use codes do not record used_by
        assert!(fetched.used_by.is_none());
    }

    #[tokio::test]
    async fn expired_code_rejected() {
        let pool = setup().await;
        let mut row = sample_code("EXPIRED");
        row.expires_at = Some(100);
        create_code(&pool, &row).await.unwrap();

        let ok = use_code(&pool, "EXPIRED", 10, 200).await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn nonexistent_code_use_returns_false() {
        let pool = setup().await;
        let ok = use_code(&pool, "NOPE", 10, 500).await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn list_codes_ordered() {
        let pool = setup().await;
        let mut a = sample_code("AAA");
        a.created_at = 2000;
        let mut b = sample_code("BBB");
        b.created_at = 1000;
        create_code(&pool, &a).await.unwrap();
        create_code(&pool, &b).await.unwrap();

        let all = list_codes(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].code, "BBB"); // earlier created_at first
        assert_eq!(all[1].code, "AAA");
    }

    #[tokio::test]
    async fn list_codes_empty() {
        let pool = setup().await;
        let all = list_codes(&pool).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn delete_code_removes() {
        let pool = setup().await;
        create_code(&pool, &sample_code("DEL")).await.unwrap();
        delete_code(&pool, "DEL").await.unwrap();
        assert!(get_code(&pool, "DEL").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_is_ok() {
        let pool = setup().await;
        delete_code(&pool, "NOPE").await.unwrap();
    }
}
