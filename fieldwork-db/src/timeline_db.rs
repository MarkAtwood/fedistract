//! Paginated timeline queries.
//!
//! Provides home, public, tag, and context (ancestor/descendant) timeline
//! queries that return [`crate::posts_db::PostRow`] vectors.

use crate::db::Pool;
use crate::posts_db::PostRow;

// Re-use the column list and row converters from posts_db (they are pub(crate)).
const COLUMNS: &str = "\
    id, user_id, persona_id, ap_id, \
    in_reply_to_id, in_reply_to_uri, boost_of_id, boost_of_uri, \
    content, content_html, spoiler_text, visibility, \
    sensitive, language, context_url, \
    created_at, edited_at, deleted_at, deleted_reason";

#[cfg(feature = "sqlite")]
fn sqlite_row_to_post(r: sqlx::sqlite::SqliteRow) -> PostRow {
    use sqlx::Row;
    PostRow {
        id: r.get(0),
        user_id: r.get(1),
        persona_id: r.get(2),
        ap_id: r.get(3),
        in_reply_to_id: r.get(4),
        in_reply_to_uri: r.get(5),
        boost_of_id: r.get(6),
        boost_of_uri: r.get(7),
        content: r.get(8),
        content_html: r.get(9),
        spoiler_text: r.get(10),
        visibility: r.get(11),
        sensitive: r.get(12),
        language: r.get(13),
        context_url: r.get(14),
        created_at: r.get(15),
        edited_at: r.get(16),
        deleted_at: r.get(17),
        deleted_reason: r.get(18),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_post(r: sqlx::postgres::PgRow) -> PostRow {
    use sqlx::Row;
    PostRow {
        id: r.get(0),
        user_id: r.get(1),
        persona_id: r.get(2),
        ap_id: r.get(3),
        in_reply_to_id: r.get(4),
        in_reply_to_uri: r.get(5),
        boost_of_id: r.get(6),
        boost_of_uri: r.get(7),
        content: r.get(8),
        content_html: r.get(9),
        spoiler_text: r.get(10),
        visibility: r.get(11),
        sensitive: r.get(12),
        language: r.get(13),
        context_url: r.get(14),
        created_at: r.get(15),
        edited_at: r.get(16),
        deleted_at: r.get(17),
        deleted_reason: r.get(18),
    }
}

/// Home timeline: own posts + posts from followed local personas.
///
/// Paginated by `max_id` (descending order). Excludes soft-deleted posts.
pub async fn home_timeline(
    pool: &Pool,
    persona_id: i64,
    limit: i64,
    max_id: Option<i64>,
) -> Result<Vec<PostRow>, sqlx::Error> {
    let max = max_id.unwrap_or(i64::MAX);
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE (persona_id = $1 \
                        OR persona_id IN (SELECT followee_persona_id FROM follows \
                                          WHERE persona_id = $1 AND followee_persona_id IS NOT NULL)) \
                 AND id < $2 AND deleted_at IS NULL \
                 ORDER BY id DESC LIMIT $3"
            );
            let rows = sqlx::query(&sql)
                .bind(persona_id)
                .bind(max)
                .bind(limit)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(pg_row_to_post).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE (persona_id = ?1 \
                        OR persona_id IN (SELECT followee_persona_id FROM follows \
                                          WHERE persona_id = ?1 AND followee_persona_id IS NOT NULL)) \
                 AND id < ?2 AND deleted_at IS NULL \
                 ORDER BY id DESC LIMIT ?3"
            );
            let rows = sqlx::query(&sql)
                .bind(persona_id)
                .bind(max)
                .bind(limit)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(sqlite_row_to_post).collect())
        }
    }
}

/// Public timeline: all public, non-deleted posts.
pub async fn public_timeline(
    pool: &Pool,
    limit: i64,
    max_id: Option<i64>,
) -> Result<Vec<PostRow>, sqlx::Error> {
    let max = max_id.unwrap_or(i64::MAX);
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE visibility = 'public' AND deleted_at IS NULL AND id < $1 \
                 ORDER BY id DESC LIMIT $2"
            );
            let rows = sqlx::query(&sql)
                .bind(max)
                .bind(limit)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(pg_row_to_post).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE visibility = 'public' AND deleted_at IS NULL AND id < ?1 \
                 ORDER BY id DESC LIMIT ?2"
            );
            let rows = sqlx::query(&sql)
                .bind(max)
                .bind(limit)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(sqlite_row_to_post).collect())
        }
    }
}

/// Tag timeline: posts with a given hashtag.
pub async fn tag_timeline(
    pool: &Pool,
    tag: &str,
    limit: i64,
    max_id: Option<i64>,
) -> Result<Vec<PostRow>, sqlx::Error> {
    let max = max_id.unwrap_or(i64::MAX);
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE id IN (SELECT post_id FROM post_tags WHERE tag = $1) \
                 AND deleted_at IS NULL AND id < $2 \
                 ORDER BY id DESC LIMIT $3"
            );
            let rows = sqlx::query(&sql)
                .bind(tag)
                .bind(max)
                .bind(limit)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(pg_row_to_post).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE id IN (SELECT post_id FROM post_tags WHERE tag = ?1) \
                 AND deleted_at IS NULL AND id < ?2 \
                 ORDER BY id DESC LIMIT ?3"
            );
            let rows = sqlx::query(&sql)
                .bind(tag)
                .bind(max)
                .bind(limit)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(sqlite_row_to_post).collect())
        }
    }
}

/// Get ancestors of a post by walking the `in_reply_to_id` chain upward.
///
/// Returns posts in chronological order (oldest ancestor first).
pub async fn context_ancestors(
    pool: &Pool,
    post_id: i64,
) -> Result<Vec<PostRow>, sqlx::Error> {
    // ponytail: iterative walk; upgrade to recursive CTE if depth becomes a concern
    let mut ancestors = Vec::new();
    let mut current_id = post_id;
    loop {
        let parent = match pool {
            #[cfg(feature = "postgres")]
            Pool::Postgres(pg) => {
                let sql = format!(
                    "SELECT {COLUMNS} FROM posts WHERE id = \
                     (SELECT in_reply_to_id FROM posts WHERE id = $1 AND in_reply_to_id IS NOT NULL)"
                );
                sqlx::query(&sql)
                    .bind(current_id)
                    .fetch_optional(pg)
                    .await?
                    .map(pg_row_to_post)
            }
            #[cfg(feature = "sqlite")]
            Pool::Sqlite(sq) => {
                let sql = format!(
                    "SELECT {COLUMNS} FROM posts WHERE id = \
                     (SELECT in_reply_to_id FROM posts WHERE id = ?1 AND in_reply_to_id IS NOT NULL)"
                );
                sqlx::query(&sql)
                    .bind(current_id)
                    .fetch_optional(sq)
                    .await?
                    .map(sqlite_row_to_post)
            }
        };
        match parent {
            Some(p) => {
                current_id = p.id;
                ancestors.push(p);
            }
            None => break,
        }
    }
    ancestors.reverse();
    Ok(ancestors)
}

/// Get descendants of a post (posts where `in_reply_to_id = post_id`), recursive.
///
/// Returns posts in chronological order (oldest first).
pub async fn context_descendants(
    pool: &Pool,
    post_id: i64,
) -> Result<Vec<PostRow>, sqlx::Error> {
    // ponytail: BFS walk; upgrade to recursive CTE if breadth becomes a concern
    let mut descendants = Vec::new();
    let mut queue = vec![post_id];
    while let Some(parent_id) = queue.pop() {
        let children: Vec<PostRow> = match pool {
            #[cfg(feature = "postgres")]
            Pool::Postgres(pg) => {
                let sql = format!(
                    "SELECT {COLUMNS} FROM posts \
                     WHERE in_reply_to_id = $1 AND deleted_at IS NULL \
                     ORDER BY id ASC"
                );
                sqlx::query(&sql)
                    .bind(parent_id)
                    .fetch_all(pg)
                    .await?
                    .into_iter()
                    .map(pg_row_to_post)
                    .collect()
            }
            #[cfg(feature = "sqlite")]
            Pool::Sqlite(sq) => {
                let sql = format!(
                    "SELECT {COLUMNS} FROM posts \
                     WHERE in_reply_to_id = ?1 AND deleted_at IS NULL \
                     ORDER BY id ASC"
                );
                sqlx::query(&sql)
                    .bind(parent_id)
                    .fetch_all(sq)
                    .await?
                    .into_iter()
                    .map(sqlite_row_to_post)
                    .collect()
            }
        };
        for child in children {
            queue.push(child.id);
            descendants.push(child);
        }
    }
    Ok(descendants)
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
    use super::*;
    use crate::db;
    use crate::posts_db;

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
             VALUES (102, 1, 'bob', 'pk2', 'pub2', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn make_post(id: i64, persona_id: i64, visibility: &str) -> posts_db::PostRow {
        posts_db::PostRow {
            id,
            user_id: 1,
            persona_id,
            ap_id: format!("https://example.com/posts/{id}"),
            in_reply_to_id: None,
            in_reply_to_uri: None,
            boost_of_id: None,
            boost_of_uri: None,
            content: "hello".into(),
            content_html: "<p>hello</p>".into(),
            spoiler_text: String::new(),
            visibility: visibility.into(),
            sensitive: false,
            language: Some("en".into()),
            context_url: None,
            created_at: 2000 + id,
            edited_at: None,
            deleted_at: None,
            deleted_reason: None,
        }
    }

    #[tokio::test]
    async fn public_timeline_returns_public_only() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        posts_db::create_post(&pool, &make_post(2, 101, "direct")).await.unwrap();
        posts_db::create_post(&pool, &make_post(3, 101, "public")).await.unwrap();

        let timeline = public_timeline(&pool, 10, None).await.unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].id, 3); // newest first
        assert_eq!(timeline[1].id, 1);
    }

    #[tokio::test]
    async fn public_timeline_paginates() {
        let pool = setup().await;
        for i in 1..=5 {
            posts_db::create_post(&pool, &make_post(i, 101, "public")).await.unwrap();
        }

        let page1 = public_timeline(&pool, 3, None).await.unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page1[0].id, 5);

        let page2 = public_timeline(&pool, 3, Some(3)).await.unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].id, 2);
    }

    #[tokio::test]
    async fn home_timeline_includes_own_and_followed() {
        let pool = setup().await;
        // Alice follows Bob locally
        crate::follows_db::follow_local(&pool, 101, 1, 102, 1000).await.unwrap();

        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        posts_db::create_post(&pool, &make_post(2, 102, "public")).await.unwrap();

        let timeline = home_timeline(&pool, 101, 10, None).await.unwrap();
        assert_eq!(timeline.len(), 2);
    }

    #[tokio::test]
    async fn home_timeline_excludes_unfollowed() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        posts_db::create_post(&pool, &make_post(2, 102, "public")).await.unwrap();

        // Alice does NOT follow Bob
        let timeline = home_timeline(&pool, 101, 10, None).await.unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].persona_id, 101);
    }

    #[tokio::test]
    async fn tag_timeline_filters_by_tag() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        posts_db::create_post(&pool, &make_post(2, 101, "public")).await.unwrap();

        crate::post_tags_db::add_tags(&pool, 1, &["rust".into()]).await.unwrap();

        let timeline = tag_timeline(&pool, "rust", 10, None).await.unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].id, 1);
    }

    #[tokio::test]
    async fn tag_timeline_empty_for_unknown_tag() {
        let pool = setup().await;
        let timeline = tag_timeline(&pool, "nonexistent", 10, None).await.unwrap();
        assert!(timeline.is_empty());
    }

    #[tokio::test]
    async fn context_ancestors_walks_chain() {
        let pool = setup().await;
        // Create a chain: 1 -> 2 -> 3
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        let mut p2 = make_post(2, 101, "public");
        p2.in_reply_to_id = Some(1);
        posts_db::create_post(&pool, &p2).await.unwrap();
        let mut p3 = make_post(3, 101, "public");
        p3.in_reply_to_id = Some(2);
        posts_db::create_post(&pool, &p3).await.unwrap();

        let ancestors = context_ancestors(&pool, 3).await.unwrap();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0].id, 1); // oldest first
        assert_eq!(ancestors[1].id, 2);
    }

    #[tokio::test]
    async fn context_ancestors_empty_for_root() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();

        let ancestors = context_ancestors(&pool, 1).await.unwrap();
        assert!(ancestors.is_empty());
    }

    #[tokio::test]
    async fn context_descendants_finds_replies() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        let mut p2 = make_post(2, 101, "public");
        p2.in_reply_to_id = Some(1);
        posts_db::create_post(&pool, &p2).await.unwrap();
        let mut p3 = make_post(3, 101, "public");
        p3.in_reply_to_id = Some(1);
        posts_db::create_post(&pool, &p3).await.unwrap();

        let descendants = context_descendants(&pool, 1).await.unwrap();
        assert_eq!(descendants.len(), 2);
        assert_eq!(descendants[0].id, 2);
        assert_eq!(descendants[1].id, 3);
    }

    #[tokio::test]
    async fn context_descendants_recursive() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();
        let mut p2 = make_post(2, 101, "public");
        p2.in_reply_to_id = Some(1);
        posts_db::create_post(&pool, &p2).await.unwrap();
        let mut p3 = make_post(3, 101, "public");
        p3.in_reply_to_id = Some(2);
        posts_db::create_post(&pool, &p3).await.unwrap();

        let descendants = context_descendants(&pool, 1).await.unwrap();
        assert_eq!(descendants.len(), 2);
        assert_eq!(descendants[0].id, 2);
        assert_eq!(descendants[1].id, 3);
    }

    #[tokio::test]
    async fn context_descendants_empty() {
        let pool = setup().await;
        posts_db::create_post(&pool, &make_post(1, 101, "public")).await.unwrap();

        let descendants = context_descendants(&pool, 1).await.unwrap();
        assert!(descendants.is_empty());
    }
}
