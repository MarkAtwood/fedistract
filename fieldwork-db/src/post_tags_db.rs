//! Database operations for post hashtags.
//!
//! Pure CRUD operations against the canonical `post_tags` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit
//! the correct SQL dialect.

use crate::db::Pool;

/// Add tags to a post. Silently ignores duplicates.
pub async fn add_tags(
    pool: &Pool,
    post_id: i64,
    tags: &[String],
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            for tag in tags {
                sqlx::query(
                    "INSERT INTO post_tags (post_id, tag) \
                     VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(post_id)
                .bind(tag)
                .execute(pg)
                .await?;
            }
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            for tag in tags {
                sqlx::query(
                    "INSERT OR IGNORE INTO post_tags (post_id, tag) \
                     VALUES (?1, ?2)",
                )
                .bind(post_id)
                .bind(tag)
                .execute(sq)
                .await?;
            }
        }
    }
    Ok(())
}

/// Get all tags for a post.
pub async fn get_tags(pool: &Pool, post_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT tag FROM post_tags WHERE post_id = $1 ORDER BY tag")
                .bind(post_id)
                .fetch_all(pg)
                .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT tag FROM post_tags WHERE post_id = ?1 ORDER BY tag")
                .bind(post_id)
                .fetch_all(sq)
                .await?
        }
    };
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Remove all tags for a post.
pub async fn remove_tags(pool: &Pool, post_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM post_tags WHERE post_id = $1")
                .bind(post_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM post_tags WHERE post_id = ?1")
                .bind(post_id)
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
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (200, 1, 101, 'https://example.com/posts/200', 'yo', '<p>yo</p>', 'public', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn add_and_get_tags() {
        let pool = setup().await;
        let tags = vec!["rust".into(), "fediverse".into()];
        add_tags(&pool, 100, &tags).await.unwrap();

        let fetched = get_tags(&pool, 100).await.unwrap();
        assert_eq!(fetched, vec!["fediverse", "rust"]);
    }

    #[tokio::test]
    async fn add_tags_is_idempotent() {
        let pool = setup().await;
        let tags = vec!["rust".into()];
        add_tags(&pool, 100, &tags).await.unwrap();
        add_tags(&pool, 100, &tags).await.unwrap();

        let fetched = get_tags(&pool, 100).await.unwrap();
        assert_eq!(fetched.len(), 1);
    }

    #[tokio::test]
    async fn remove_tags_clears_all() {
        let pool = setup().await;
        add_tags(&pool, 100, &["a".into(), "b".into()]).await.unwrap();
        remove_tags(&pool, 100).await.unwrap();

        let fetched = get_tags(&pool, 100).await.unwrap();
        assert!(fetched.is_empty());
    }

    #[tokio::test]
    async fn tags_are_per_post() {
        let pool = setup().await;
        add_tags(&pool, 100, &["shared".into(), "only100".into()]).await.unwrap();
        add_tags(&pool, 200, &["shared".into(), "only200".into()]).await.unwrap();

        assert_eq!(get_tags(&pool, 100).await.unwrap(), vec!["only100", "shared"]);
        assert_eq!(get_tags(&pool, 200).await.unwrap(), vec!["only200", "shared"]);
    }

    #[tokio::test]
    async fn get_tags_empty() {
        let pool = setup().await;
        let fetched = get_tags(&pool, 100).await.unwrap();
        assert!(fetched.is_empty());
    }

    #[tokio::test]
    async fn remove_nonexistent_is_ok() {
        let pool = setup().await;
        remove_tags(&pool, 999).await.unwrap();
    }
}
