//! Multi-tenant user, persona, and quota management.
//!
//! Extracted from gaja's tenant module. Provides user CRUD, persona lifecycle,
//! and quota enforcement queries against the canonical schema defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.

use crate::db::Pool;
use crate::persona_db::PersonaRow;
use sqlx::Row;

// ---------------------------------------------------------------------------
// User model
// ---------------------------------------------------------------------------

/// A row from the `users` table.
#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub oidc_subject: Option<String>,
    pub oidc_issuer: Option<String>,
    pub display_name: String,
    pub role: String,
    pub created_at: i64,
    pub suspended_at: Option<i64>,
    pub suspended_reason: Option<String>,
    pub did_key: Option<String>,
    pub did_plc: Option<String>,
    pub recovery_pubkey: Option<String>,
    pub legal_hold: bool,
}

#[cfg(feature = "db-postgres")]
const USER_COLS_PG: &str = "id::text, email, password_hash, oidc_subject, oidc_issuer, \
    display_name, role, created_at, suspended_at, suspended_reason, \
    did_key, did_plc, recovery_pubkey, legal_hold";

#[cfg(feature = "db-sqlite")]
const USER_COLS_SQ: &str = "id, email, password_hash, oidc_subject, oidc_issuer, \
    display_name, role, created_at, suspended_at, suspended_reason, \
    did_key, did_plc, recovery_pubkey, legal_hold";

#[cfg(feature = "db-postgres")]
fn pg_row_to_user(row: sqlx::postgres::PgRow) -> UserRow {
    UserRow {
        id: row.get(0),
        email: row.get(1),
        password_hash: row.get(2),
        oidc_subject: row.get(3),
        oidc_issuer: row.get(4),
        display_name: row.get(5),
        role: row.get(6),
        created_at: row.get(7),
        suspended_at: row.get(8),
        suspended_reason: row.get(9),
        did_key: row.get(10),
        did_plc: row.get(11),
        recovery_pubkey: row.get(12),
        legal_hold: row.get(13),
    }
}

#[cfg(feature = "db-sqlite")]
fn sq_row_to_user(row: sqlx::sqlite::SqliteRow) -> UserRow {
    let legal_hold: i32 = row.get(13);
    UserRow {
        id: row.get(0),
        email: row.get(1),
        password_hash: row.get(2),
        oidc_subject: row.get(3),
        oidc_issuer: row.get(4),
        display_name: row.get(5),
        role: row.get(6),
        created_at: row.get(7),
        suspended_at: row.get(8),
        suspended_reason: row.get(9),
        did_key: row.get(10),
        did_plc: row.get(11),
        recovery_pubkey: row.get(12),
        legal_hold: legal_hold != 0,
    }
}

// ---------------------------------------------------------------------------
// Quota model
// ---------------------------------------------------------------------------

/// Quota limits for a user.
#[derive(Debug, Clone)]
pub struct QuotaInfo {
    pub media_bytes: Option<i64>,
    pub posts_per_day: Option<i32>,
    pub followers: Option<i32>,
    pub following: Option<i32>,
}

// ---------------------------------------------------------------------------
// User management
// ---------------------------------------------------------------------------

/// Create a new user.
pub async fn create_user(
    pool: &Pool,
    id: &str,
    email: &str,
    password_hash: Option<&str>,
    role: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO users (id, email, password_hash, role, created_at) \
                 VALUES ($1::uuid, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(email)
            .bind(password_hash)
            .bind(role)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO users (id, email, password_hash, role, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(id)
            .bind(email)
            .bind(password_hash)
            .bind(role)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Look up a user by email.
pub async fn get_user_by_email(
    pool: &Pool,
    email: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {USER_COLS_PG} FROM users WHERE email = $1"),
            )
            .bind(email)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_user))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {USER_COLS_SQ} FROM users WHERE email = ?1"),
            )
            .bind(email)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sq_row_to_user))
        }
    }
}

/// Look up a user by ID.
pub async fn get_user_by_id(
    pool: &Pool,
    id: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {USER_COLS_PG} FROM users WHERE id = $1::uuid"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_user))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {USER_COLS_SQ} FROM users WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sq_row_to_user))
        }
    }
}

/// List all users, ordered by creation time.
pub async fn list_users(pool: &Pool) -> Result<Vec<UserRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!("SELECT {USER_COLS_PG} FROM users ORDER BY created_at"),
            )
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_user).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!("SELECT {USER_COLS_SQ} FROM users ORDER BY created_at"),
            )
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sq_row_to_user).collect())
        }
    }
}

/// Suspend a user.
pub async fn suspend_user(
    pool: &Pool,
    id: &str,
    reason: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE users SET suspended_at = $1, suspended_reason = $2 WHERE id = $3::uuid",
            )
            .bind(now)
            .bind(reason)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE users SET suspended_at = ?1, suspended_reason = ?2 WHERE id = ?3",
            )
            .bind(now)
            .bind(reason)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unsuspend a user.
pub async fn unsuspend_user(pool: &Pool, id: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE users SET suspended_at = NULL, suspended_reason = NULL WHERE id = $1::uuid",
            )
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE users SET suspended_at = NULL, suspended_reason = NULL WHERE id = ?1",
            )
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Update a user's password hash.
pub async fn update_user_password(
    pool: &Pool,
    id: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2::uuid")
                .bind(password_hash)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
                .bind(password_hash)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Update a user's DID key and recovery public key.
pub async fn update_user_did(
    pool: &Pool,
    id: &str,
    did_key: &str,
    recovery_pubkey: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE users SET did_key = $1, recovery_pubkey = $2 WHERE id = $3::uuid",
            )
            .bind(did_key)
            .bind(recovery_pubkey)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE users SET did_key = ?1, recovery_pubkey = ?2 WHERE id = ?3",
            )
            .bind(did_key)
            .bind(recovery_pubkey)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Persona management (per user)
// ---------------------------------------------------------------------------

/// Create a persona for a user.
pub async fn create_persona_for_user(
    pool: &Pool,
    persona_id: &str,
    user_id: &str,
    username: &str,
    display_name: &str,
    private_key_pem: &str,
    public_key_pem: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO personas \
                 (id, user_id, username, display_name, private_key_pem, public_key_pem, created_at) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(username)
            .bind(display_name)
            .bind(private_key_pem)
            .bind(public_key_pem)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO personas \
                 (id, user_id, username, display_name, private_key_pem, public_key_pem, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(persona_id)
            .bind(user_id)
            .bind(username)
            .bind(display_name)
            .bind(private_key_pem)
            .bind(public_key_pem)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// List all personas for a user. Reuses [`PersonaRow`] from `persona_db`.
pub async fn list_personas_for_user(
    pool: &Pool,
    user_id: &str,
) -> Result<Vec<PersonaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            use crate::persona_db::SELECT_COLS_PG;
            let rows = sqlx::query(
                &format!("SELECT {SELECT_COLS_PG} FROM personas WHERE user_id = $1::uuid ORDER BY created_at"),
            )
            .bind(user_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(crate::persona_db::pg_row_to_persona).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            use crate::persona_db::SELECT_COLS_SQ;
            let rows = sqlx::query(
                &format!("SELECT {SELECT_COLS_SQ} FROM personas WHERE user_id = ?1 ORDER BY created_at"),
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(crate::persona_db::sq_row_to_persona).collect())
        }
    }
}

/// Delete a persona by ID (CASCADE).
pub async fn delete_persona(pool: &Pool, persona_id: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM personas WHERE id = $1::uuid")
                .bind(persona_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM personas WHERE id = ?1")
                .bind(persona_id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Quota management
// ---------------------------------------------------------------------------

/// Get quota limits for a user.
pub async fn get_quota(pool: &Pool, user_id: &str) -> Result<QuotaInfo, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                "SELECT quota_media_bytes, quota_posts_per_day, quota_followers, quota_following \
                 FROM users WHERE id = $1::uuid",
            )
            .bind(user_id)
            .fetch_one(pg)
            .await?;
            Ok(QuotaInfo {
                media_bytes: row.get(0),
                posts_per_day: row.get(1),
                followers: row.get(2),
                following: row.get(3),
            })
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                "SELECT quota_media_bytes, quota_posts_per_day, quota_followers, quota_following \
                 FROM users WHERE id = ?1",
            )
            .bind(user_id)
            .fetch_one(sq)
            .await?;
            Ok(QuotaInfo {
                media_bytes: row.get(0),
                posts_per_day: row.get(1),
                followers: row.get(2),
                following: row.get(3),
            })
        }
    }
}

/// Set a single quota field for a user. `field` must be one of:
/// `media_bytes`, `posts_per_day`, `followers`, `following`.
pub async fn set_quota(
    pool: &Pool,
    user_id: &str,
    field: &str,
    value: Option<i64>,
) -> Result<(), sqlx::Error> {
    let col = match field {
        "media_bytes" => "quota_media_bytes",
        "posts_per_day" => "quota_posts_per_day",
        "followers" => "quota_followers",
        "following" => "quota_following",
        _ => return Err(sqlx::Error::Protocol(format!("unknown quota field: {field}"))),
    };
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(&format!("UPDATE users SET {col} = $1 WHERE id = $2::uuid"))
                .bind(value)
                .bind(user_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(&format!("UPDATE users SET {col} = ?1 WHERE id = ?2"))
                .bind(value)
                .bind(user_id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Check whether a user is within their daily post quota. Returns `true` if
/// posting is allowed (no quota set, or under the limit).
pub async fn check_post_quota(
    pool: &Pool,
    user_id: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let quota = get_quota(pool, user_id).await?;
    let limit = match quota.posts_per_day {
        Some(l) => l,
        None => return Ok(true),
    };
    let day_ago = now - 86400;
    let count: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::bigint FROM posts \
                 WHERE user_id = $1::uuid AND created_at >= $2",
            )
            .bind(user_id)
            .bind(day_ago)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM posts \
                 WHERE user_id = ?1 AND created_at >= ?2",
            )
            .bind(user_id)
            .bind(day_ago)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(count.0 < limit as i64)
}

/// Check whether a user is within their media storage quota. Returns `true` if
/// uploading is allowed (no quota set, or under the limit).
pub async fn check_media_quota(
    pool: &Pool,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let quota = get_quota(pool, user_id).await?;
    let limit = match quota.media_bytes {
        Some(l) => l,
        None => return Ok(true),
    };
    let total: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COALESCE(SUM(file_size), 0)::bigint FROM media WHERE user_id = $1::uuid",
            )
            .bind(user_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COALESCE(SUM(file_size), 0) FROM media WHERE user_id = ?1",
            )
            .bind(user_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(total.0 < limit)
}

/// Check whether a persona is within their follower quota. Returns `true` if
/// accepting followers is allowed (no quota set, or under the limit).
/// Looks up the quota from the persona's owning user.
pub async fn check_follower_quota(
    pool: &Pool,
    persona_id: &str,
) -> Result<bool, sqlx::Error> {
    // Get the user_id for this persona, then check their quota
    let user_id: Option<(String,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT user_id::text FROM personas WHERE id = $1::uuid")
                .bind(persona_id)
                .fetch_optional(pg)
                .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT user_id FROM personas WHERE id = ?1")
                .bind(persona_id)
                .fetch_optional(sq)
                .await?
        }
    };
    let uid = match user_id {
        Some(u) => u.0,
        None => return Ok(true),
    };
    let quota = get_quota(pool, &uid).await?;
    let limit = match quota.followers {
        Some(l) => l,
        None => return Ok(true),
    };
    let count: (i64,) = match pool {
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
    Ok(count.0 < limit as i64)
}

/// Check whether a persona is within their following quota. Returns `true` if
/// following more accounts is allowed (no quota set, or under the limit).
/// Looks up the quota from the persona's owning user.
pub async fn check_following_quota(
    pool: &Pool,
    persona_id: &str,
) -> Result<bool, sqlx::Error> {
    let user_id: Option<(String,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT user_id::text FROM personas WHERE id = $1::uuid")
                .bind(persona_id)
                .fetch_optional(pg)
                .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT user_id FROM personas WHERE id = ?1")
                .bind(persona_id)
                .fetch_optional(sq)
                .await?
        }
    };
    let uid = match user_id {
        Some(u) => u.0,
        None => return Ok(true),
    };
    let quota = get_quota(pool, &uid).await?;
    let limit = match quota.following {
        Some(l) => l,
        None => return Ok(true),
    };
    let count: (i64,) = match pool {
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
    Ok(count.0 < limit as i64)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        let pool = Pool::Sqlite(sq);
        db::migrate(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_get_user_by_id() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", Some("hash"), "user", 1000)
            .await
            .unwrap();

        let user = get_user_by_id(&pool, "u1").await.unwrap().expect("should exist");
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.role, "user");
        assert!(user.suspended_at.is_none());
    }

    #[tokio::test]
    async fn get_user_by_email_works() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        let user = get_user_by_email(&pool, "alice@example.com")
            .await
            .unwrap()
            .expect("should exist");
        assert_eq!(user.id, "u1");
    }

    #[tokio::test]
    async fn get_nonexistent_user_returns_none() {
        let pool = setup().await;
        assert!(get_user_by_id(&pool, "nope").await.unwrap().is_none());
        assert!(get_user_by_email(&pool, "nope@x.com").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_users_ordered() {
        let pool = setup().await;
        create_user(&pool, "u2", "bob@example.com", None, "user", 2000)
            .await
            .unwrap();
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        let users = list_users(&pool).await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].id, "u1"); // earlier created_at first
    }

    #[tokio::test]
    async fn suspend_and_unsuspend() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        suspend_user(&pool, "u1", "spam", 2000).await.unwrap();
        let user = get_user_by_id(&pool, "u1").await.unwrap().unwrap();
        assert_eq!(user.suspended_at, Some(2000));
        assert_eq!(user.suspended_reason.as_deref(), Some("spam"));

        unsuspend_user(&pool, "u1").await.unwrap();
        let user = get_user_by_id(&pool, "u1").await.unwrap().unwrap();
        assert!(user.suspended_at.is_none());
        assert!(user.suspended_reason.is_none());
    }

    #[tokio::test]
    async fn update_password() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", Some("old"), "user", 1000)
            .await
            .unwrap();

        update_user_password(&pool, "u1", "new_hash").await.unwrap();
        let user = get_user_by_id(&pool, "u1").await.unwrap().unwrap();
        assert_eq!(user.password_hash.as_deref(), Some("new_hash"));
    }

    #[tokio::test]
    async fn update_did() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        update_user_did(&pool, "u1", "did:key:z6Mk...", "abcdef").await.unwrap();
        let user = get_user_by_id(&pool, "u1").await.unwrap().unwrap();
        assert_eq!(user.did_key.as_deref(), Some("did:key:z6Mk..."));
        assert_eq!(user.recovery_pubkey.as_deref(), Some("abcdef"));
    }

    #[tokio::test]
    async fn create_and_list_personas() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        let personas = list_personas_for_user(&pool, "u1").await.unwrap();
        assert_eq!(personas.len(), 1);
        assert_eq!(personas[0].username, "alice");
        assert_eq!(personas[0].user_id, "u1");
    }

    #[tokio::test]
    async fn delete_persona_removes() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        delete_persona(&pool, "p1").await.unwrap();
        let personas = list_personas_for_user(&pool, "u1").await.unwrap();
        assert!(personas.is_empty());
    }

    #[tokio::test]
    async fn quota_get_set() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        let q = get_quota(&pool, "u1").await.unwrap();
        assert!(q.media_bytes.is_none());
        assert!(q.posts_per_day.is_none());

        set_quota(&pool, "u1", "media_bytes", Some(1_000_000)).await.unwrap();
        set_quota(&pool, "u1", "posts_per_day", Some(10)).await.unwrap();

        let q = get_quota(&pool, "u1").await.unwrap();
        assert_eq!(q.media_bytes, Some(1_000_000));
        assert_eq!(q.posts_per_day, Some(10));

        // Clear a quota
        set_quota(&pool, "u1", "media_bytes", None).await.unwrap();
        let q = get_quota(&pool, "u1").await.unwrap();
        assert!(q.media_bytes.is_none());
    }

    #[tokio::test]
    async fn set_quota_invalid_field() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        let result = set_quota(&pool, "u1", "bogus", Some(42)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn check_post_quota_no_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        // No quota set: always allowed
        assert!(check_post_quota(&pool, "u1", 5000).await.unwrap());
    }

    #[tokio::test]
    async fn check_post_quota_under_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "posts_per_day", Some(5)).await.unwrap();

        // Insert a post within the last 24h
        let now = 100_000i64;
        let Pool::Sqlite(sq) = &pool;
        sqlx::query(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(1i64)
        .bind("u1")
        .bind("p1")
        .bind("https://example.com/posts/1")
        .bind("hello")
        .bind("<p>hello</p>")
        .bind("public")
        .bind(now - 100)
        .execute(sq)
        .await
        .unwrap();

        assert!(check_post_quota(&pool, "u1", now).await.unwrap());
    }

    #[tokio::test]
    async fn check_post_quota_at_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "posts_per_day", Some(2)).await.unwrap();

        let now = 100_000i64;
        let Pool::Sqlite(sq) = &pool;
        for i in 0..2 {
            sqlx::query(
                "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(i + 1i64)
            .bind("u1")
            .bind("p1")
            .bind(format!("https://example.com/posts/{}", i + 1))
            .bind("hello")
            .bind("<p>hello</p>")
            .bind("public")
            .bind(now - 100)
            .execute(sq)
            .await
            .unwrap();
        }

        assert!(!check_post_quota(&pool, "u1", now).await.unwrap());
    }

    #[tokio::test]
    async fn check_media_quota_no_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();

        assert!(check_media_quota(&pool, "u1").await.unwrap());
    }

    #[tokio::test]
    async fn check_media_quota_under_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "media_bytes", Some(1_000_000)).await.unwrap();

        let Pool::Sqlite(sq) = &pool;
        sqlx::query(
            "INSERT INTO media (id, user_id, persona_id, file_path, mime_type, file_size, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(1i64)
        .bind("u1")
        .bind("p1")
        .bind("/tmp/test.jpg")
        .bind("image/jpeg")
        .bind(500_000i64)
        .bind(1000i64)
        .execute(sq)
        .await
        .unwrap();

        assert!(check_media_quota(&pool, "u1").await.unwrap());
    }

    #[tokio::test]
    async fn check_media_quota_over_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "media_bytes", Some(100)).await.unwrap();

        let Pool::Sqlite(sq) = &pool;
        sqlx::query(
            "INSERT INTO media (id, user_id, persona_id, file_path, mime_type, file_size, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(1i64)
        .bind("u1")
        .bind("p1")
        .bind("/tmp/test.jpg")
        .bind("image/jpeg")
        .bind(200i64)
        .bind(1000i64)
        .execute(sq)
        .await
        .unwrap();

        assert!(!check_media_quota(&pool, "u1").await.unwrap());
    }

    #[tokio::test]
    async fn check_follower_quota_no_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        assert!(check_follower_quota(&pool, "p1").await.unwrap());
    }

    #[tokio::test]
    async fn check_follower_quota_at_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "followers", Some(1)).await.unwrap();

        // Insert a remote account and a follower
        let Pool::Sqlite(sq) = &pool;
        sqlx::query(
            "INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
             public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(1i64)
        .bind("https://remote/users/bob")
        .bind("bob")
        .bind("remote")
        .bind("Bob")
        .bind("pk")
        .bind("https://remote/users/bob#key")
        .bind("https://remote/inbox")
        .bind(1000i64)
        .execute(sq)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO followers (persona_id, user_id, remote_account_id, accepted_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind("p1")
        .bind("u1")
        .bind(1i64)
        .bind(2000i64)
        .execute(sq)
        .await
        .unwrap();

        assert!(!check_follower_quota(&pool, "p1").await.unwrap());
    }

    #[tokio::test]
    async fn check_following_quota_no_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        assert!(check_following_quota(&pool, "p1").await.unwrap());
    }

    #[tokio::test]
    async fn check_following_quota_at_limit() {
        let pool = setup().await;
        create_user(&pool, "u1", "alice@example.com", None, "user", 1000)
            .await
            .unwrap();
        create_persona_for_user(
            &pool, "p1", "u1", "alice", "Alice", "privkey", "pubkey", 1000,
        )
        .await
        .unwrap();

        set_quota(&pool, "u1", "following", Some(1)).await.unwrap();

        // Insert a remote account and a follow
        let Pool::Sqlite(sq) = &pool;
        sqlx::query(
            "INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
             public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(1i64)
        .bind("https://remote/users/bob")
        .bind("bob")
        .bind("remote")
        .bind("Bob")
        .bind("pk")
        .bind("https://remote/users/bob#key")
        .bind("https://remote/inbox")
        .bind(1000i64)
        .execute(sq)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO follows (persona_id, user_id, followee_remote_id, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind("p1")
        .bind("u1")
        .bind(1i64)
        .bind(2000i64)
        .execute(sq)
        .await
        .unwrap();

        assert!(!check_following_quota(&pool, "p1").await.unwrap());
    }
}
