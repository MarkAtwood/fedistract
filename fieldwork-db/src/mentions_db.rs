//! Database operations for post mentions.
//!
//! Pure CRUD operations against the canonical `mentions` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit
//! the correct SQL dialect.

use crate::db::Pool;

/// A row from the `mentions` table.
#[derive(Debug, Clone)]
pub struct MentionRow {
    pub post_id: i64,
    pub mentioned_remote_id: Option<i64>,
    pub mentioned_persona_id: Option<i64>,
}

/// Add a mention to a post.
pub async fn add_mention(
    pool: &Pool,
    post_id: i64,
    remote_id: Option<i64>,
    persona_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO mentions (post_id, mentioned_remote_id, mentioned_persona_id) \
                 VALUES ($1, $2, $3)",
            )
            .bind(post_id)
            .bind(remote_id)
            .bind(persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO mentions (post_id, mentioned_remote_id, mentioned_persona_id) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(post_id)
            .bind(remote_id)
            .bind(persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all mentions for a post.
pub async fn get_mentions(
    pool: &Pool,
    post_id: i64,
) -> Result<Vec<MentionRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, Option<i64>, Option<i64>)> = sqlx::query_as(
                "SELECT post_id, mentioned_remote_id, mentioned_persona_id \
                 FROM mentions WHERE post_id = $1",
            )
            .bind(post_id)
            .fetch_all(pg)
            .await?;
            Ok(rows
                .into_iter()
                .map(|(pid, rid, pid2)| MentionRow {
                    post_id: pid,
                    mentioned_remote_id: rid,
                    mentioned_persona_id: pid2,
                })
                .collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, Option<i64>, Option<i64>)> = sqlx::query_as(
                "SELECT post_id, mentioned_remote_id, mentioned_persona_id \
                 FROM mentions WHERE post_id = ?1",
            )
            .bind(post_id)
            .fetch_all(sq)
            .await?;
            Ok(rows
                .into_iter()
                .map(|(pid, rid, pid2)| MentionRow {
                    post_id: pid,
                    mentioned_remote_id: rid,
                    mentioned_persona_id: pid2,
                })
                .collect())
        }
    }
}

/// Remove all mentions for a post.
pub async fn remove_mentions(pool: &Pool, post_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM mentions WHERE post_id = $1")
                .bind(post_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM mentions WHERE post_id = ?1")
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
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'pk2', 'pub2', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/carol', 'carol', 'remote', 'Carol', \
                     'pk', 'https://remote/users/carol#key', 'https://remote/inbox', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn add_and_get_local_mention() {
        let pool = setup().await;
        add_mention(&pool, 100, None, Some(102)).await.unwrap();

        let mentions = get_mentions(&pool, 100).await.unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].mentioned_persona_id, Some(102));
        assert!(mentions[0].mentioned_remote_id.is_none());
    }

    #[tokio::test]
    async fn add_and_get_remote_mention() {
        let pool = setup().await;
        add_mention(&pool, 100, Some(1), None).await.unwrap();

        let mentions = get_mentions(&pool, 100).await.unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].mentioned_remote_id, Some(1));
        assert!(mentions[0].mentioned_persona_id.is_none());
    }

    #[tokio::test]
    async fn multiple_mentions() {
        let pool = setup().await;
        add_mention(&pool, 100, None, Some(102)).await.unwrap();
        add_mention(&pool, 100, Some(1), None).await.unwrap();

        let mentions = get_mentions(&pool, 100).await.unwrap();
        assert_eq!(mentions.len(), 2);
    }

    #[tokio::test]
    async fn remove_mentions_clears_all() {
        let pool = setup().await;
        add_mention(&pool, 100, None, Some(102)).await.unwrap();
        add_mention(&pool, 100, Some(1), None).await.unwrap();
        remove_mentions(&pool, 100).await.unwrap();

        let mentions = get_mentions(&pool, 100).await.unwrap();
        assert!(mentions.is_empty());
    }

    #[tokio::test]
    async fn get_mentions_empty() {
        let pool = setup().await;
        let mentions = get_mentions(&pool, 100).await.unwrap();
        assert!(mentions.is_empty());
    }

    #[tokio::test]
    async fn remove_nonexistent_is_ok() {
        let pool = setup().await;
        remove_mentions(&pool, 999).await.unwrap();
    }
}
