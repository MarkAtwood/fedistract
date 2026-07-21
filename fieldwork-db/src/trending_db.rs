//! Trending / discover queries for the explore API.
//!
//! Aggregates over `post_tags`, `favourites`, and `posts` (boosts) to power
//! the Pixelfed-compatible discover endpoints. Each function dispatches on
//! [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;

/// Trending hashtags: tags with the most posts in a time window.
///
/// Returns `(tag, post_count)` pairs ordered by post count descending.
/// `since` is a unix timestamp; only posts created at or after this time
/// are counted.
pub async fn trending_tags(
    pool: &Pool,
    limit: i64,
    since: i64,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT pt.tag, COUNT(*) AS cnt \
                 FROM post_tags pt \
                 JOIN posts p ON p.id = pt.post_id \
                 WHERE p.created_at >= $1 AND p.visibility = 'public' \
                 GROUP BY pt.tag \
                 ORDER BY cnt DESC, pt.tag \
                 LIMIT $2",
            )
            .bind(since)
            .bind(limit)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT pt.tag, COUNT(*) AS cnt \
                 FROM post_tags pt \
                 JOIN posts p ON p.id = pt.post_id \
                 WHERE p.created_at >= ?1 AND p.visibility = 'public' \
                 GROUP BY pt.tag \
                 ORDER BY cnt DESC, pt.tag \
                 LIMIT ?2",
            )
            .bind(since)
            .bind(limit)
            .fetch_all(sq)
            .await
        }
    }
}

/// Trending posts: post IDs ordered by interaction count (favourites + boosts)
/// in a time window.
///
/// `since` is a unix timestamp. Only public posts are included.
/// Boosts are counted by looking at `posts.boost_of_id`.
pub async fn trending_posts(
    pool: &Pool,
    limit: i64,
    since: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT p.id \
                 FROM posts p \
                 LEFT JOIN favourites f ON f.post_id = p.id AND f.created_at >= $1 \
                 LEFT JOIN posts b ON b.boost_of_id = p.id AND b.created_at >= $1 \
                 WHERE p.created_at >= $1 AND p.visibility = 'public' \
                   AND p.boost_of_id IS NULL \
                 GROUP BY p.id \
                 ORDER BY (COUNT(DISTINCT f.persona_id) + COUNT(DISTINCT b.id)) DESC, p.id DESC \
                 LIMIT $2",
            )
            .bind(since)
            .bind(limit)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT p.id \
                 FROM posts p \
                 LEFT JOIN favourites f ON f.post_id = p.id AND f.created_at >= ?1 \
                 LEFT JOIN posts b ON b.boost_of_id = p.id AND b.created_at >= ?1 \
                 WHERE p.created_at >= ?1 AND p.visibility = 'public' \
                   AND p.boost_of_id IS NULL \
                 GROUP BY p.id \
                 ORDER BY (COUNT(DISTINCT f.persona_id) + COUNT(DISTINCT b.id)) DESC, p.id DESC \
                 LIMIT ?2",
            )
            .bind(since)
            .bind(limit)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|r| r.0).collect())
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
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
             VALUES (102, 2, 'bob', 'pk2', 'pub2', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    #[tokio::test]
    async fn trending_tags_empty() {
        let (pool, _) = setup().await;
        let tags = trending_tags(&pool, 10, 0).await.unwrap();
        assert!(tags.is_empty());
    }

    #[tokio::test]
    async fn trending_tags_counts_correctly() {
        let (pool, sq) = setup().await;
        // Create 3 public posts
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'b', '<p>b</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (3, 1, 101, 'https://example.com/p/3', 'c', '<p>c</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (4, 1, 101, 'https://example.com/p/4', 'd', '<p>d</p>', 'unlisted', 5000); \
             INSERT INTO post_tags (post_id, tag) VALUES (1, 'rust'); \
             INSERT INTO post_tags (post_id, tag) VALUES (2, 'rust'); \
             INSERT INTO post_tags (post_id, tag) VALUES (3, 'rust'); \
             INSERT INTO post_tags (post_id, tag) VALUES (1, 'fedi'); \
             INSERT INTO post_tags (post_id, tag) VALUES (2, 'fedi'); \
             INSERT INTO post_tags (post_id, tag) VALUES (3, 'golang'); \
             INSERT INTO post_tags (post_id, tag) VALUES (4, 'unlisted');",
        )
        .execute(&sq)
        .await
        .unwrap();

        let tags = trending_tags(&pool, 10, 4000).await.unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], ("rust".into(), 3));
        assert_eq!(tags[1], ("fedi".into(), 2));
        assert_eq!(tags[2], ("golang".into(), 1));
    }

    #[tokio::test]
    async fn trending_tags_respects_since() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'b', '<p>b</p>', 'public', 5000); \
             INSERT INTO post_tags (post_id, tag) VALUES (1, 'old'); \
             INSERT INTO post_tags (post_id, tag) VALUES (2, 'new');",
        )
        .execute(&sq)
        .await
        .unwrap();

        let tags = trending_tags(&pool, 10, 3000).await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "new");
    }

    #[tokio::test]
    async fn trending_tags_respects_limit() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'b', '<p>b</p>', 'public', 5000); \
             INSERT INTO post_tags (post_id, tag) VALUES (1, 'a'); \
             INSERT INTO post_tags (post_id, tag) VALUES (1, 'b'); \
             INSERT INTO post_tags (post_id, tag) VALUES (2, 'a');",
        )
        .execute(&sq)
        .await
        .unwrap();

        let tags = trending_tags(&pool, 1, 0).await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "a");
    }

    #[tokio::test]
    async fn trending_posts_empty() {
        let (pool, _) = setup().await;
        let posts = trending_posts(&pool, 10, 0).await.unwrap();
        assert!(posts.is_empty());
    }

    #[tokio::test]
    async fn trending_posts_by_favs() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'b', '<p>b</p>', 'public', 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (1, 101, 1, 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (2, 102, 1, 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (1, 101, 2, 5000);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let posts = trending_posts(&pool, 10, 4000).await.unwrap();
        assert_eq!(posts, vec![1, 2]);
    }

    #[tokio::test]
    async fn trending_posts_by_boosts() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'b', '<p>b</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at, boost_of_id) \
             VALUES (10, 2, 102, 'https://example.com/p/10', '', '', 'public', 5000, 2); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at, boost_of_id) \
             VALUES (11, 1, 101, 'https://example.com/p/11', '', '', 'public', 5000, 2);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let posts = trending_posts(&pool, 10, 4000).await.unwrap();
        // Post 2 has 2 boosts, post 1 has 0
        assert_eq!(posts[0], 2);
        assert_eq!(posts[1], 1);
    }

    #[tokio::test]
    async fn trending_posts_excludes_non_public() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'unlisted', 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (1, 101, 1, 5000);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let posts = trending_posts(&pool, 10, 4000).await.unwrap();
        assert!(posts.is_empty());
    }

    #[tokio::test]
    async fn trending_posts_excludes_boosts_themselves() {
        let (pool, sq) = setup().await;
        // A boost post should not appear as a trending post itself
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'a', '<p>a</p>', 'public', 5000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at, boost_of_id) \
             VALUES (10, 2, 102, 'https://example.com/p/10', '', '', 'public', 5000, 1);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let posts = trending_posts(&pool, 10, 4000).await.unwrap();
        // Only post 1 should appear, not the boost (post 10)
        assert_eq!(posts, vec![1]);
    }

    #[tokio::test]
    async fn trending_posts_respects_since() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (1, 1, 101, 'https://example.com/p/1', 'old', '<p>old</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (2, 1, 101, 'https://example.com/p/2', 'new', '<p>new</p>', 'public', 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (1, 101, 1, 5000); \
             INSERT INTO favourites (user_id, persona_id, post_id, created_at) VALUES (1, 101, 2, 5000);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let posts = trending_posts(&pool, 10, 3000).await.unwrap();
        assert_eq!(posts, vec![2]);
    }
}
