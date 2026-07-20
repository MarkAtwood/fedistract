//! Database operations for user interactions: favourites, bookmarks, pins, blocks, mutes.
//!
//! Pure CRUD operations against the canonical tables defined in [`crate::db`].
//! Each function dispatches on [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Favourites
// ---------------------------------------------------------------------------

/// Favourite a post. Silently ignores duplicates.
/// Exactly one of `post_id` or `remote_post_id` must be `Some`.
pub async fn favourite(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO favourites (user_id, persona_id, post_id, remote_post_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            // SQLite UNIQUE doesn't enforce when NULLs are present in the constraint
            // columns, so we use WHERE NOT EXISTS for idempotency.
            sqlx::query(
                "INSERT INTO favourites \
                 (user_id, persona_id, post_id, remote_post_id, created_at) \
                 SELECT ?1, ?2, ?3, ?4, ?5 \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM favourites \
                     WHERE persona_id = ?2 AND (post_id IS ?3) AND (remote_post_id IS ?4) \
                 )",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a favourite.
pub async fn unfavourite(
    pool: &Pool,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM favourites \
                 WHERE persona_id = $1 \
                 AND post_id IS NOT DISTINCT FROM $2 \
                 AND remote_post_id IS NOT DISTINCT FROM $3",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM favourites \
                 WHERE persona_id = ?1 \
                 AND (post_id IS ?2) \
                 AND (remote_post_id IS ?3)",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check whether a local post is favourited by a persona.
pub async fn is_favourited(
    pool: &Pool,
    persona_id: i64,
    post_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM favourites \
                 WHERE persona_id = $1 AND post_id = $2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM favourites \
                 WHERE persona_id = ?1 AND post_id = ?2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
}

// ---------------------------------------------------------------------------
// Bookmarks
// ---------------------------------------------------------------------------

/// Bookmark a post. Silently ignores duplicates.
pub async fn bookmark(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO bookmarks (user_id, persona_id, post_id, remote_post_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            // SQLite UNIQUE doesn't enforce when NULLs are present in the constraint
            // columns, so we use WHERE NOT EXISTS for idempotency.
            sqlx::query(
                "INSERT INTO bookmarks \
                 (user_id, persona_id, post_id, remote_post_id, created_at) \
                 SELECT ?1, ?2, ?3, ?4, ?5 \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM bookmarks \
                     WHERE persona_id = ?2 AND (post_id IS ?3) AND (remote_post_id IS ?4) \
                 )",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a bookmark.
pub async fn unbookmark(
    pool: &Pool,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM bookmarks \
                 WHERE persona_id = $1 \
                 AND post_id IS NOT DISTINCT FROM $2 \
                 AND remote_post_id IS NOT DISTINCT FROM $3",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM bookmarks \
                 WHERE persona_id = ?1 \
                 AND (post_id IS ?2) \
                 AND (remote_post_id IS ?3)",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pinned posts
// ---------------------------------------------------------------------------

/// Pin a local post.
pub async fn pin_post(
    pool: &Pool,
    persona_id: i64,
    post_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO pinned_posts (persona_id, post_id, pinned_at) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO pinned_posts (persona_id, post_id, pinned_at) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unpin a local post.
pub async fn unpin_post(
    pool: &Pool,
    persona_id: i64,
    post_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM pinned_posts WHERE persona_id = $1 AND post_id = $2",
            )
            .bind(persona_id)
            .bind(post_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM pinned_posts WHERE persona_id = ?1 AND post_id = ?2",
            )
            .bind(persona_id)
            .bind(post_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

/// Block an account. Exactly one of `target_persona_id` or `target_remote_id` should be set.
pub async fn block(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO blocks (user_id, persona_id, target_persona_id, target_remote_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO blocks (user_id, persona_id, target_persona_id, target_remote_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a block.
pub async fn unblock(
    pool: &Pool,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM blocks \
                 WHERE persona_id = $1 \
                 AND target_persona_id IS NOT DISTINCT FROM $2 \
                 AND target_remote_id IS NOT DISTINCT FROM $3",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM blocks \
                 WHERE persona_id = ?1 \
                 AND (target_persona_id IS ?2) \
                 AND (target_remote_id IS ?3)",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check whether an account is blocked.
pub async fn is_blocked(
    pool: &Pool,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM blocks \
                 WHERE persona_id = $1 \
                 AND target_persona_id IS NOT DISTINCT FROM $2 \
                 AND target_remote_id IS NOT DISTINCT FROM $3 \
                 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM blocks \
                 WHERE persona_id = ?1 \
                 AND (target_persona_id IS ?2) \
                 AND (target_remote_id IS ?3) \
                 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
}

// ---------------------------------------------------------------------------
// Mutes
// ---------------------------------------------------------------------------

/// Mute an account.
pub async fn mute(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO mutes (user_id, persona_id, target_persona_id, target_remote_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO mutes (user_id, persona_id, target_persona_id, target_remote_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a mute.
pub async fn unmute(
    pool: &Pool,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM mutes \
                 WHERE persona_id = $1 \
                 AND target_persona_id IS NOT DISTINCT FROM $2 \
                 AND target_remote_id IS NOT DISTINCT FROM $3",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM mutes \
                 WHERE persona_id = ?1 \
                 AND (target_persona_id IS ?2) \
                 AND (target_remote_id IS ?3)",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check whether an account is muted.
pub async fn is_muted(
    pool: &Pool,
    persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM mutes \
                 WHERE persona_id = $1 \
                 AND target_persona_id IS NOT DISTINCT FROM $2 \
                 AND target_remote_id IS NOT DISTINCT FROM $3 \
                 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM mutes \
                 WHERE persona_id = ?1 \
                 AND (target_persona_id IS ?2) \
                 AND (target_remote_id IS ?3) \
                 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .bind(target_remote_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
}

#[cfg(test)]
#[cfg(feature = "db-sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> (Pool, sqlx::SqlitePool) {
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
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'pk2', 'pub2', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (200, 1, 101, 'https://example.com/posts/200', 'yo', '<p>yo</p>', 'public', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/carol', 'carol', 'remote', 'Carol', \
                     'pk', 'https://remote/users/carol#key', 'https://remote/inbox', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    // --- Favourites ---

    #[tokio::test]
    async fn favourite_and_check() {
        let (pool, _) = setup().await;
        favourite(&pool, 1, 101, Some(100), None, 2000).await.unwrap();
        assert!(is_favourited(&pool, 101, 100).await.unwrap());
        assert!(!is_favourited(&pool, 101, 200).await.unwrap());
    }

    #[tokio::test]
    async fn favourite_is_idempotent() {
        let (pool, _) = setup().await;
        favourite(&pool, 1, 101, Some(100), None, 2000).await.unwrap();
        favourite(&pool, 1, 101, Some(100), None, 3000).await.unwrap();
        assert!(is_favourited(&pool, 101, 100).await.unwrap());
    }

    #[tokio::test]
    async fn unfavourite_removes() {
        let (pool, _) = setup().await;
        favourite(&pool, 1, 101, Some(100), None, 2000).await.unwrap();
        unfavourite(&pool, 101, Some(100), None).await.unwrap();
        assert!(!is_favourited(&pool, 101, 100).await.unwrap());
    }

    // --- Bookmarks ---

    #[tokio::test]
    async fn bookmark_and_unbookmark() {
        let (pool, sq) = setup().await;
        bookmark(&pool, 1, 101, Some(100), None, 2000).await.unwrap();

        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM bookmarks WHERE persona_id = 101 AND post_id = 100",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 1);

        unbookmark(&pool, 101, Some(100), None).await.unwrap();
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM bookmarks WHERE persona_id = 101 AND post_id = 100",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn bookmark_is_idempotent() {
        let (pool, sq) = setup().await;
        bookmark(&pool, 1, 101, Some(100), None, 2000).await.unwrap();
        bookmark(&pool, 1, 101, Some(100), None, 3000).await.unwrap();
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM bookmarks WHERE persona_id = 101",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 1);
    }

    // --- Pins ---

    #[tokio::test]
    async fn pin_and_unpin() {
        let (pool, sq) = setup().await;
        pin_post(&pool, 101, 100, 2000).await.unwrap();

        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pinned_posts WHERE persona_id = 101 AND post_id = 100",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 1);

        unpin_post(&pool, 101, 100).await.unwrap();
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pinned_posts WHERE persona_id = 101",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn pin_is_idempotent() {
        let (pool, sq) = setup().await;
        pin_post(&pool, 101, 100, 2000).await.unwrap();
        pin_post(&pool, 101, 100, 3000).await.unwrap();
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pinned_posts WHERE persona_id = 101",
        )
        .fetch_one(&sq)
        .await
        .unwrap();
        assert_eq!(count.0, 1);
    }

    // --- Blocks ---

    #[tokio::test]
    async fn block_local_and_check() {
        let (pool, _) = setup().await;
        block(&pool, 1, 101, Some(102), None, 2000).await.unwrap();
        assert!(is_blocked(&pool, 101, Some(102), None).await.unwrap());
        assert!(!is_blocked(&pool, 101, None, Some(1)).await.unwrap());
    }

    #[tokio::test]
    async fn block_remote_and_check() {
        let (pool, _) = setup().await;
        block(&pool, 1, 101, None, Some(1), 2000).await.unwrap();
        assert!(is_blocked(&pool, 101, None, Some(1)).await.unwrap());
    }

    #[tokio::test]
    async fn unblock_removes() {
        let (pool, _) = setup().await;
        block(&pool, 1, 101, Some(102), None, 2000).await.unwrap();
        unblock(&pool, 101, Some(102), None).await.unwrap();
        assert!(!is_blocked(&pool, 101, Some(102), None).await.unwrap());
    }

    // --- Mutes ---

    #[tokio::test]
    async fn mute_and_check() {
        let (pool, _) = setup().await;
        mute(&pool, 1, 101, Some(102), None, 2000).await.unwrap();
        assert!(is_muted(&pool, 101, Some(102), None).await.unwrap());
        assert!(!is_muted(&pool, 101, None, Some(1)).await.unwrap());
    }

    #[tokio::test]
    async fn unmute_removes() {
        let (pool, _) = setup().await;
        mute(&pool, 1, 101, None, Some(1), 2000).await.unwrap();
        unmute(&pool, 101, None, Some(1)).await.unwrap();
        assert!(!is_muted(&pool, 101, None, Some(1)).await.unwrap());
    }

    #[tokio::test]
    async fn unblock_nonexistent_is_ok() {
        let (pool, _) = setup().await;
        unblock(&pool, 101, Some(102), None).await.unwrap();
    }

    #[tokio::test]
    async fn unmute_nonexistent_is_ok() {
        let (pool, _) = setup().await;
        unmute(&pool, 101, Some(102), None).await.unwrap();
    }
}
