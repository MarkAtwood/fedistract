//! Database operations for followed hashtags.
//!
//! Pure CRUD against the canonical `followed_tags` table.

use crate::db::Pool;

/// Follow a tag for a user. Silently ignores duplicates.
pub async fn follow_tag(
    pool: &Pool,
    user_id: i64,
    tag: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO followed_tags (user_id, tag, created_at) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(tag)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO followed_tags (user_id, tag, created_at) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(user_id)
            .bind(tag)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unfollow a tag for a user.
pub async fn unfollow_tag(pool: &Pool, user_id: i64, tag: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM followed_tags WHERE user_id = $1 AND tag = $2",
            )
            .bind(user_id)
            .bind(tag)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM followed_tags WHERE user_id = ?1 AND tag = ?2")
                .bind(user_id)
                .bind(tag)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Get all followed tags for a user. Returns (tag, created_at) pairs.
pub async fn get_followed_tags(
    pool: &Pool,
    user_id: i64,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT tag, created_at FROM followed_tags \
                 WHERE user_id = $1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT tag, created_at FROM followed_tags \
                 WHERE user_id = ?1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await
        }
    }
}

/// Check if a user is following a specific tag.
pub async fn is_following_tag(
    pool: &Pool,
    user_id: i64,
    tag: &str,
) -> Result<bool, sqlx::Error> {
    let count: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM followed_tags \
                 WHERE user_id = $1 AND tag = $2",
            )
            .bind(user_id)
            .bind(tag)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM followed_tags \
                 WHERE user_id = ?1 AND tag = ?2",
            )
            .bind(user_id)
            .bind(tag)
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
             VALUES (1, 'test@example.com', 'Test', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn follow_and_check() {
        let pool = setup().await;
        assert!(!is_following_tag(&pool, 1, "rust").await.unwrap());

        follow_tag(&pool, 1, "rust", 2000).await.unwrap();
        assert!(is_following_tag(&pool, 1, "rust").await.unwrap());
    }

    #[tokio::test]
    async fn follow_is_idempotent() {
        let pool = setup().await;
        follow_tag(&pool, 1, "rust", 2000).await.unwrap();
        follow_tag(&pool, 1, "rust", 3000).await.unwrap();

        let tags = get_followed_tags(&pool, 1).await.unwrap();
        assert_eq!(tags.len(), 1);
    }

    #[tokio::test]
    async fn unfollow_removes() {
        let pool = setup().await;
        follow_tag(&pool, 1, "rust", 2000).await.unwrap();
        unfollow_tag(&pool, 1, "rust").await.unwrap();
        assert!(!is_following_tag(&pool, 1, "rust").await.unwrap());
    }

    #[tokio::test]
    async fn unfollow_nonexistent_is_ok() {
        let pool = setup().await;
        unfollow_tag(&pool, 1, "nosuchtag").await.unwrap();
    }

    #[tokio::test]
    async fn get_followed_tags_ordered() {
        let pool = setup().await;
        follow_tag(&pool, 1, "rust", 1000).await.unwrap();
        follow_tag(&pool, 1, "fediverse", 2000).await.unwrap();
        follow_tag(&pool, 1, "activitypub", 3000).await.unwrap();

        let tags = get_followed_tags(&pool, 1).await.unwrap();
        assert_eq!(tags.len(), 3);
        // Ordered by created_at DESC
        assert_eq!(tags[0].0, "activitypub");
        assert_eq!(tags[1].0, "fediverse");
        assert_eq!(tags[2].0, "rust");
    }

    #[tokio::test]
    async fn tags_are_per_user() {
        let pool = setup().await;
        // Add a second user
        let Pool::Sqlite(sq) = &pool;
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (2, 'user2@example.com', 'User2', 'user', 1000);",
        )
        .execute(sq)
        .await
        .unwrap();

        follow_tag(&pool, 1, "rust", 1000).await.unwrap();
        follow_tag(&pool, 2, "python", 1000).await.unwrap();

        assert!(is_following_tag(&pool, 1, "rust").await.unwrap());
        assert!(!is_following_tag(&pool, 1, "python").await.unwrap());
        assert!(!is_following_tag(&pool, 2, "rust").await.unwrap());
        assert!(is_following_tag(&pool, 2, "python").await.unwrap());
    }
}
