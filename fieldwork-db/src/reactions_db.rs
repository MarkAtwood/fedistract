//! Database operations for Misskey-style emoji reactions.
//!
//! Reactions differ from Mastodon favourites: a user can add multiple distinct
//! emoji reactions to a single post, and each emoji is tracked independently
//! with a count.

use crate::db::Pool;

/// Add a reaction. Silently ignores duplicates (same persona + post + emoji).
pub async fn add_reaction(
    pool: &Pool,
    id: i64,
    user_id: i64,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
    emoji: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO reactions (id, user_id, persona_id, post_id, remote_post_id, emoji, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(emoji)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO reactions \
                 (id, user_id, persona_id, post_id, remote_post_id, emoji, created_at) \
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7 \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM reactions \
                     WHERE persona_id = ?3 AND emoji = ?6 \
                       AND (post_id IS ?4) AND (remote_post_id IS ?5) \
                 )",
            )
            .bind(id)
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(emoji)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a reaction (specific emoji from specific persona on specific post).
pub async fn remove_reaction(
    pool: &Pool,
    persona_id: i64,
    post_id: Option<i64>,
    remote_post_id: Option<i64>,
    emoji: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM reactions \
                 WHERE persona_id = $1 \
                 AND post_id IS NOT DISTINCT FROM $2 \
                 AND remote_post_id IS NOT DISTINCT FROM $3 \
                 AND emoji = $4",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(emoji)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM reactions \
                 WHERE persona_id = ?1 \
                 AND (post_id IS ?2) \
                 AND (remote_post_id IS ?3) \
                 AND emoji = ?4",
            )
            .bind(persona_id)
            .bind(post_id)
            .bind(remote_post_id)
            .bind(emoji)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get aggregated reaction counts for a local post: Vec<(emoji, count)>.
pub async fn reactions_for_post(
    pool: &Pool,
    post_id: i64,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT emoji, COUNT(*) as cnt FROM reactions \
                 WHERE post_id = $1 \
                 GROUP BY emoji ORDER BY cnt DESC",
            )
            .bind(post_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT emoji, COUNT(*) as cnt FROM reactions \
                 WHERE post_id = ?1 \
                 GROUP BY emoji ORDER BY cnt DESC",
            )
            .bind(post_id)
            .fetch_all(sq)
            .await
        }
    }
}

/// Get the list of emoji this persona has reacted with on a local post.
pub async fn user_reactions(
    pool: &Pool,
    persona_id: i64,
    post_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT emoji FROM reactions \
                 WHERE persona_id = $1 AND post_id = $2",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT emoji FROM reactions \
                 WHERE persona_id = ?1 AND post_id = ?2",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|(e,)| e).collect())
}

#[cfg(test)]
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
             INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (2, 'bob@example.com', 'Bob', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 2, 'bob', 'pk2', 'pub2', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1001, 1, 101, 'https://example.com/p/1001', 'hello', '<p>hello</p>', 'public', 5000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    #[tokio::test]
    async fn add_and_query_reactions() {
        let (pool, _sq) = setup().await;

        add_reaction(&pool, 5001, 1, 101, Some(1001), None, "\u{1f44d}", 1000)
            .await
            .unwrap();
        add_reaction(&pool, 5002, 1, 101, Some(1001), None, "\u{2764}\u{fe0f}", 1001)
            .await
            .unwrap();

        // Duplicate should be silently ignored
        add_reaction(&pool, 5003, 1, 101, Some(1001), None, "\u{1f44d}", 1002)
            .await
            .unwrap();

        let counts = reactions_for_post(&pool, 1001).await.unwrap();
        assert_eq!(counts.len(), 2);
        for (_, count) in &counts {
            assert_eq!(*count, 1);
        }

        let my_reactions = user_reactions(&pool, 101, 1001).await.unwrap();
        assert_eq!(my_reactions.len(), 2);
        assert!(my_reactions.contains(&"\u{1f44d}".to_string()));
        assert!(my_reactions.contains(&"\u{2764}\u{fe0f}".to_string()));
    }

    #[tokio::test]
    async fn remove_reaction_works() {
        let (pool, _sq) = setup().await;

        add_reaction(&pool, 5010, 1, 101, Some(1001), None, "\u{1f44d}", 1000)
            .await
            .unwrap();

        let before = reactions_for_post(&pool, 1001).await.unwrap();
        assert_eq!(before.len(), 1);

        remove_reaction(&pool, 101, Some(1001), None, "\u{1f44d}")
            .await
            .unwrap();

        let after = reactions_for_post(&pool, 1001).await.unwrap();
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn multiple_users_same_emoji() {
        let (pool, _sq) = setup().await;

        add_reaction(&pool, 5020, 1, 101, Some(1001), None, "\u{1f44d}", 1000)
            .await
            .unwrap();
        add_reaction(&pool, 5021, 2, 102, Some(1001), None, "\u{1f44d}", 1001)
            .await
            .unwrap();

        let counts = reactions_for_post(&pool, 1001).await.unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].0, "\u{1f44d}");
        assert_eq!(counts[0].1, 2);

        let my = user_reactions(&pool, 101, 1001).await.unwrap();
        assert_eq!(my.len(), 1);
        let theirs = user_reactions(&pool, 102, 1001).await.unwrap();
        assert_eq!(theirs.len(), 1);
    }
}
