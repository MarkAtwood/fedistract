//! Search queries for personas, remote accounts, tags, and posts.
//!
//! Uses SQL `LIKE` pattern matching. Callers are responsible for
//! constructing the LIKE pattern (e.g., `%query%`) and escaping
//! special characters.

use crate::db::Pool;

/// Search local personas by username or display_name.
///
/// Returns `(id, username, display_name)` tuples.
pub async fn search_personas(
    pool: &Pool,
    query: &str,
    limit: i64,
) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, username, display_name FROM personas \
                 WHERE username LIKE $1 OR display_name LIKE $1 \
                 LIMIT $2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, username, display_name FROM personas \
                 WHERE username LIKE ?1 OR display_name LIKE ?1 \
                 LIMIT ?2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(sq)
            .await
        }
    }
}

/// Search remote accounts by username, domain, or display_name.
///
/// Returns `(id, username, domain, display_name)` tuples.
pub async fn search_remote_accounts(
    pool: &Pool,
    query: &str,
    limit: i64,
) -> Result<Vec<(i64, String, String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, username, domain, display_name FROM remote_accounts \
                 WHERE username LIKE $1 OR display_name LIKE $1 \
                 LIMIT $2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, username, domain, display_name FROM remote_accounts \
                 WHERE username LIKE ?1 OR display_name LIKE ?1 \
                 LIMIT ?2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(sq)
            .await
        }
    }
}

/// Search tags by name pattern. Returns distinct tag names.
pub async fn search_tags(
    pool: &Pool,
    query: &str,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT DISTINCT tag FROM post_tags WHERE tag LIKE $1 LIMIT $2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT DISTINCT tag FROM post_tags WHERE tag LIKE ?1 LIMIT ?2",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|(t,)| t).collect())
}

/// Get all posts for search reindexing.
///
/// Returns `(id, content, content_html)` for every post, ordered by ID.
pub async fn all_posts_for_reindex(
    pool: &Pool,
) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, content, content_html FROM posts ORDER BY id",
            )
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, content, content_html FROM posts ORDER BY id",
            )
            .fetch_all(sq)
            .await
        }
    }
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
             INSERT INTO personas (id, user_id, username, display_name, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'Alice Wonderland', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, display_name, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'Bob Builder', 'pk2', 'pub2', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/carol', 'carol', 'remote.example', 'Carol Singer', \
                     'pk', 'https://remote/users/carol#key', 'https://remote/inbox', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hello world', '<p>hello world</p>', 'public', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (200, 1, 101, 'https://example.com/posts/200', 'goodbye', '<p>goodbye</p>', 'public', 2000); \
             INSERT INTO post_tags (post_id, tag) VALUES (100, 'rust'); \
             INSERT INTO post_tags (post_id, tag) VALUES (100, 'fediverse'); \
             INSERT INTO post_tags (post_id, tag) VALUES (200, 'rustlang');",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn search_personas_by_username() {
        let pool = setup().await;
        let results = search_personas(&pool, "%ali%", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "alice");
    }

    #[tokio::test]
    async fn search_personas_by_display_name() {
        let pool = setup().await;
        let results = search_personas(&pool, "%Builder%", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "bob");
    }

    #[tokio::test]
    async fn search_personas_no_match() {
        let pool = setup().await;
        let results = search_personas(&pool, "%zzz%", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_personas_respects_limit() {
        let pool = setup().await;
        let results = search_personas(&pool, "%", 1).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_remote_accounts_by_username() {
        let pool = setup().await;
        let results = search_remote_accounts(&pool, "%carol%", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "carol");
        assert_eq!(results[0].2, "remote.example");
    }

    #[tokio::test]
    async fn search_remote_accounts_no_match() {
        let pool = setup().await;
        let results = search_remote_accounts(&pool, "%zzz%", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_tags_finds_matching() {
        let pool = setup().await;
        let results = search_tags(&pool, "rust%", 10).await.unwrap();
        assert_eq!(results.len(), 2); // "rust" and "rustlang"
    }

    #[tokio::test]
    async fn search_tags_exact() {
        let pool = setup().await;
        let results = search_tags(&pool, "fediverse", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "fediverse");
    }

    #[tokio::test]
    async fn search_tags_no_match() {
        let pool = setup().await;
        let results = search_tags(&pool, "zzz%", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn all_posts_for_reindex_returns_all() {
        let pool = setup().await;
        let posts = all_posts_for_reindex(&pool).await.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].0, 100);
        assert_eq!(posts[1].0, 200);
        assert_eq!(posts[0].1, "hello world");
    }

    #[tokio::test]
    async fn all_posts_for_reindex_empty() {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();

        let posts = all_posts_for_reindex(&pool).await.unwrap();
        assert!(posts.is_empty());
    }
}
