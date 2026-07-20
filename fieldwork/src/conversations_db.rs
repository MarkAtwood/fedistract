//! Database operations for conversation read/hidden state.
//!
//! Pure CRUD against the canonical `conversation_read_markers` and
//! `conversation_hidden` tables.

use crate::db::Pool;

/// Mark a conversation post as read for a user.
pub async fn mark_read(pool: &Pool, user_id: &str, post_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO conversation_read_markers (user_id, post_id) \
                 VALUES ($1::uuid, $2) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO conversation_read_markers (user_id, post_id) \
                 VALUES (?1, ?2)",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check if a conversation post has been read by a user.
pub async fn is_read(pool: &Pool, user_id: &str, post_id: i64) -> Result<bool, sqlx::Error> {
    let count: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM conversation_read_markers \
                 WHERE user_id = $1::uuid AND post_id = $2",
            )
            .bind(user_id)
            .bind(post_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM conversation_read_markers \
                 WHERE user_id = ?1 AND post_id = ?2",
            )
            .bind(user_id)
            .bind(post_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(count.0 > 0)
}

/// Hide a conversation post for a user.
pub async fn hide(pool: &Pool, user_id: &str, post_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO conversation_hidden (user_id, post_id) \
                 VALUES ($1::uuid, $2) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO conversation_hidden (user_id, post_id) \
                 VALUES (?1, ?2)",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unhide a conversation post for a user.
pub async fn unhide(pool: &Pool, user_id: &str, post_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM conversation_hidden \
                 WHERE user_id = $1::uuid AND post_id = $2",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM conversation_hidden \
                 WHERE user_id = ?1 AND post_id = ?2",
            )
            .bind(user_id)
            .bind(post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check if a conversation post is hidden for a user.
pub async fn is_hidden(pool: &Pool, user_id: &str, post_id: i64) -> Result<bool, sqlx::Error> {
    let count: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM conversation_hidden \
                 WHERE user_id = $1::uuid AND post_id = $2",
            )
            .bind(user_id)
            .bind(post_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM conversation_hidden \
                 WHERE user_id = ?1 AND post_id = ?2",
            )
            .bind(user_id)
            .bind(post_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(count.0 > 0)
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
             VALUES (1, 'u1', 'p1', 'https://example/posts/1', 'hello', '<p>hello</p>', 'direct', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 'u1', 'p1', 'https://example/posts/2', 'world', '<p>world</p>', 'direct', 1001);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn mark_read_and_check() {
        let pool = setup().await;
        assert!(!is_read(&pool, "u1", 1).await.unwrap());
        mark_read(&pool, "u1", 1).await.unwrap();
        assert!(is_read(&pool, "u1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn mark_read_is_idempotent() {
        let pool = setup().await;
        mark_read(&pool, "u1", 1).await.unwrap();
        mark_read(&pool, "u1", 1).await.unwrap();
        assert!(is_read(&pool, "u1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn read_state_per_post() {
        let pool = setup().await;
        mark_read(&pool, "u1", 1).await.unwrap();
        assert!(is_read(&pool, "u1", 1).await.unwrap());
        assert!(!is_read(&pool, "u1", 2).await.unwrap());
    }

    #[tokio::test]
    async fn hide_and_check() {
        let pool = setup().await;
        assert!(!is_hidden(&pool, "u1", 1).await.unwrap());
        hide(&pool, "u1", 1).await.unwrap();
        assert!(is_hidden(&pool, "u1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn hide_is_idempotent() {
        let pool = setup().await;
        hide(&pool, "u1", 1).await.unwrap();
        hide(&pool, "u1", 1).await.unwrap();
        assert!(is_hidden(&pool, "u1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn unhide_removes() {
        let pool = setup().await;
        hide(&pool, "u1", 1).await.unwrap();
        unhide(&pool, "u1", 1).await.unwrap();
        assert!(!is_hidden(&pool, "u1", 1).await.unwrap());
    }

    #[tokio::test]
    async fn unhide_nonexistent_is_ok() {
        let pool = setup().await;
        unhide(&pool, "u1", 1).await.unwrap();
    }
}
