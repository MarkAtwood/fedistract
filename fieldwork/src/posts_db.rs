//! Database operations for local posts.
//!
//! Pure CRUD operations against the canonical `posts` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit
//! the correct SQL dialect.
//!
//! `PostRow` has 18 columns, exceeding sqlx's 16-tuple `FromRow` limit,
//! so we use `query().fetch_*()` + `Row::get` by column index.

use crate::db::Pool;

/// A row from the `posts` table.
#[derive(Debug, Clone)]
pub struct PostRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub ap_id: String,
    pub in_reply_to_id: Option<i64>,
    pub in_reply_to_uri: Option<String>,
    pub boost_of_id: Option<i64>,
    pub boost_of_uri: Option<String>,
    pub content: String,
    pub content_html: String,
    pub spoiler_text: String,
    pub visibility: String,
    pub sensitive: bool,
    pub language: Option<String>,
    pub context_url: Option<String>,
    pub created_at: i64,
    pub edited_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub deleted_reason: Option<String>,
}

const COLUMNS: &str = "\
    id, user_id, persona_id, ap_id, \
    in_reply_to_id, in_reply_to_uri, boost_of_id, boost_of_uri, \
    content, content_html, spoiler_text, visibility, \
    sensitive, language, context_url, \
    created_at, edited_at, deleted_at, deleted_reason";

#[cfg(feature = "db-sqlite")]
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

#[cfg(feature = "db-postgres")]
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

/// Insert a new post.
pub async fn create_post(pool: &Pool, post: &PostRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO posts (\
                    id, user_id, persona_id, ap_id, \
                    in_reply_to_id, in_reply_to_uri, boost_of_id, boost_of_uri, \
                    content, content_html, spoiler_text, visibility, \
                    sensitive, language, context_url, \
                    created_at, edited_at, deleted_at, deleted_reason\
                 ) VALUES (\
                    $1, $2, $3, $4, \
                    $5, $6, $7, $8, \
                    $9, $10, $11, $12, \
                    $13, $14, $15, \
                    $16, $17, $18, $19\
                 )",
            )
            .bind(post.id)
            .bind(&post.user_id)
            .bind(&post.persona_id)
            .bind(&post.ap_id)
            .bind(post.in_reply_to_id)
            .bind(&post.in_reply_to_uri)
            .bind(post.boost_of_id)
            .bind(&post.boost_of_uri)
            .bind(&post.content)
            .bind(&post.content_html)
            .bind(&post.spoiler_text)
            .bind(&post.visibility)
            .bind(post.sensitive)
            .bind(&post.language)
            .bind(&post.context_url)
            .bind(post.created_at)
            .bind(post.edited_at)
            .bind(post.deleted_at)
            .bind(&post.deleted_reason)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO posts (\
                    id, user_id, persona_id, ap_id, \
                    in_reply_to_id, in_reply_to_uri, boost_of_id, boost_of_uri, \
                    content, content_html, spoiler_text, visibility, \
                    sensitive, language, context_url, \
                    created_at, edited_at, deleted_at, deleted_reason\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, \
                    ?5, ?6, ?7, ?8, \
                    ?9, ?10, ?11, ?12, \
                    ?13, ?14, ?15, \
                    ?16, ?17, ?18, ?19\
                 )",
            )
            .bind(post.id)
            .bind(&post.user_id)
            .bind(&post.persona_id)
            .bind(&post.ap_id)
            .bind(post.in_reply_to_id)
            .bind(&post.in_reply_to_uri)
            .bind(post.boost_of_id)
            .bind(&post.boost_of_uri)
            .bind(&post.content)
            .bind(&post.content_html)
            .bind(&post.spoiler_text)
            .bind(&post.visibility)
            .bind(post.sensitive)
            .bind(&post.language)
            .bind(&post.context_url)
            .bind(post.created_at)
            .bind(post.edited_at)
            .bind(post.deleted_at)
            .bind(&post.deleted_reason)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a post by primary key. Returns `None` if not found.
pub async fn get_post(pool: &Pool, id: i64) -> Result<Option<PostRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let sql = format!("SELECT {COLUMNS} FROM posts WHERE id = $1");
            let row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pg)
                .await?;
            Ok(row.map(pg_row_to_post))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!("SELECT {COLUMNS} FROM posts WHERE id = ?1");
            let row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(sq)
                .await?;
            Ok(row.map(sqlite_row_to_post))
        }
    }
}

/// Get a post by its ActivityPub ID. Returns `None` if not found.
pub async fn get_post_by_ap_id(
    pool: &Pool,
    ap_id: &str,
) -> Result<Option<PostRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let sql = format!("SELECT {COLUMNS} FROM posts WHERE ap_id = $1");
            let row = sqlx::query(&sql)
                .bind(ap_id)
                .fetch_optional(pg)
                .await?;
            Ok(row.map(pg_row_to_post))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!("SELECT {COLUMNS} FROM posts WHERE ap_id = ?1");
            let row = sqlx::query(&sql)
                .bind(ap_id)
                .fetch_optional(sq)
                .await?;
            Ok(row.map(sqlite_row_to_post))
        }
    }
}

/// Update a post's content fields and edited_at timestamp.
pub async fn update_post_content(
    pool: &Pool,
    id: i64,
    content: &str,
    content_html: &str,
    edited_at: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE posts SET content = $1, content_html = $2, edited_at = $3 \
                 WHERE id = $4",
            )
            .bind(content)
            .bind(content_html)
            .bind(edited_at)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE posts SET content = ?1, content_html = ?2, edited_at = ?3 \
                 WHERE id = ?4",
            )
            .bind(content)
            .bind(content_html)
            .bind(edited_at)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Soft-delete a post by setting `deleted_at` and optional `deleted_reason`.
pub async fn delete_post(
    pool: &Pool,
    id: i64,
    now: i64,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE posts SET deleted_at = $1, deleted_reason = $2 WHERE id = $3",
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
                "UPDATE posts SET deleted_at = ?1, deleted_reason = ?2 WHERE id = ?3",
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

/// List posts by persona, paginated by ID descending.
///
/// If `max_id` is provided, only posts with `id < max_id` are returned.
/// Excludes soft-deleted posts.
pub async fn posts_by_persona(
    pool: &Pool,
    persona_id: i64,
    limit: i64,
    max_id: Option<i64>,
) -> Result<Vec<PostRow>, sqlx::Error> {
    let max = max_id.unwrap_or(i64::MAX);
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE persona_id = $1 AND id < $2 AND deleted_at IS NULL \
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
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {COLUMNS} FROM posts \
                 WHERE persona_id = ?1 AND id < ?2 AND deleted_at IS NULL \
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

/// Count non-deleted posts for a persona.
pub async fn posts_count(pool: &Pool, persona_id: i64) -> Result<i64, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::bigint FROM posts \
                 WHERE persona_id = $1 AND deleted_at IS NULL",
            )
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM posts \
                 WHERE persona_id = ?1 AND deleted_at IS NULL",
            )
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0)
}

/// Check if a post exists by its ActivityPub ID.
pub async fn exists_by_ap_id(pool: &Pool, ap_id: &str) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT 1 FROM posts WHERE ap_id = $1 LIMIT 1")
                .bind(ap_id)
                .fetch_optional(pg)
                .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT 1 FROM posts WHERE ap_id = ?1 LIMIT 1")
                .bind(ap_id)
                .fetch_optional(sq)
                .await?
        }
    };
    Ok(row.is_some())
}

/// Update a post's full content and metadata (for edits).
pub async fn update_post_full(
    pool: &Pool,
    id: i64,
    content: &str,
    content_html: &str,
    spoiler_text: &str,
    sensitive: bool,
    edited_at: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE posts SET content = $1, content_html = $2, spoiler_text = $3, \
                 sensitive = $4, edited_at = $5 WHERE id = $6",
            )
            .bind(content)
            .bind(content_html)
            .bind(spoiler_text)
            .bind(sensitive)
            .bind(edited_at)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE posts SET content = ?1, content_html = ?2, spoiler_text = ?3, \
                 sensitive = ?4, edited_at = ?5 WHERE id = ?6",
            )
            .bind(content)
            .bind(content_html)
            .bind(spoiler_text)
            .bind(sensitive)
            .bind(edited_at)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Physically DELETE a post row (hard delete).
pub async fn hard_delete_post(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM posts WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM posts WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Insert a boost (reblog) post.
pub async fn create_boost(
    pool: &Pool,
    id: i64,
    user_id: i64,
    persona_id: i64,
    boost_of_id: i64,
    boost_of_uri: &str,
    ap_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO posts \
                 (id, user_id, persona_id, boost_of_id, boost_of_uri, ap_id, \
                  content, content_html, spoiler_text, visibility, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, '', '', '', 'public', $7)",
            )
            .bind(id)
            .bind(user_id)
            .bind(persona_id)
            .bind(boost_of_id)
            .bind(boost_of_uri)
            .bind(ap_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO posts \
                 (id, user_id, persona_id, boost_of_id, boost_of_uri, ap_id, \
                  content, content_html, spoiler_text, visibility, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', '', '', 'public', ?7)",
            )
            .bind(id)
            .bind(user_id)
            .bind(persona_id)
            .bind(boost_of_id)
            .bind(boost_of_uri)
            .bind(ap_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Find a boost by persona and original post. Returns the boost's post ID.
pub async fn find_boost(
    pool: &Pool,
    persona_id: i64,
    boost_of_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let row: Option<(i64,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id FROM posts WHERE persona_id = $1 AND boost_of_id = $2",
            )
            .bind(persona_id)
            .bind(boost_of_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id FROM posts WHERE persona_id = ?1 AND boost_of_id = ?2",
            )
            .bind(persona_id)
            .bind(boost_of_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.map(|(id,)| id))
}

/// Delete a boost by persona and original post.
pub async fn delete_boost(
    pool: &Pool,
    persona_id: i64,
    boost_of_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM posts WHERE persona_id = $1 AND boost_of_id = $2",
            )
            .bind(persona_id)
            .bind(boost_of_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM posts WHERE persona_id = ?1 AND boost_of_id = ?2",
            )
            .bind(persona_id)
            .bind(boost_of_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get featured (pinned) posts for a persona via JOIN with pinned_posts.
pub async fn featured_posts(
    pool: &Pool,
    persona_id: i64,
) -> Result<Vec<PostRow>, sqlx::Error> {
    // pinned_posts also has persona_id, so we must prefix all columns with p.
    const P_COLUMNS: &str = "\
        p.id, p.user_id, p.persona_id, p.ap_id, \
        p.in_reply_to_id, p.in_reply_to_uri, p.boost_of_id, p.boost_of_uri, \
        p.content, p.content_html, p.spoiler_text, p.visibility, \
        p.sensitive, p.language, p.context_url, \
        p.created_at, p.edited_at, p.deleted_at, p.deleted_reason";
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {P_COLUMNS} FROM posts p \
                 JOIN pinned_posts pp ON pp.post_id = p.id \
                 WHERE pp.persona_id = $1 AND p.deleted_at IS NULL \
                 ORDER BY pp.pinned_at DESC"
            );
            let rows = sqlx::query(&sql)
                .bind(persona_id)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(pg_row_to_post).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {P_COLUMNS} FROM posts p \
                 JOIN pinned_posts pp ON pp.post_id = p.id \
                 WHERE pp.persona_id = ?1 AND p.deleted_at IS NULL \
                 ORDER BY pp.pinned_at DESC"
            );
            let rows = sqlx::query(&sql)
                .bind(persona_id)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(sqlite_row_to_post).collect())
        }
    }
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
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn make_post(id: i64) -> PostRow {
        PostRow {
            id,
            user_id: 1,
            persona_id: 101,
            ap_id: format!("https://example.com/posts/{id}"),
            in_reply_to_id: None,
            in_reply_to_uri: None,
            boost_of_id: None,
            boost_of_uri: None,
            content: "hello".into(),
            content_html: "<p>hello</p>".into(),
            spoiler_text: String::new(),
            visibility: "public".into(),
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
    async fn create_and_get() {
        let pool = setup().await;
        let post = make_post(100);
        create_post(&pool, &post).await.unwrap();

        let fetched = get_post(&pool, 100).await.unwrap().unwrap();
        assert_eq!(fetched.ap_id, "https://example.com/posts/100");
        assert_eq!(fetched.content, "hello");
    }

    #[tokio::test]
    async fn get_by_ap_id() {
        let pool = setup().await;
        create_post(&pool, &make_post(101)).await.unwrap();

        let fetched = get_post_by_ap_id(&pool, "https://example.com/posts/101")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.id, 101);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_post(&pool, 999).await.unwrap().is_none());
        assert!(get_post_by_ap_id(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_content() {
        let pool = setup().await;
        create_post(&pool, &make_post(102)).await.unwrap();

        update_post_content(&pool, 102, "updated", "<p>updated</p>", 5000)
            .await
            .unwrap();

        let fetched = get_post(&pool, 102).await.unwrap().unwrap();
        assert_eq!(fetched.content, "updated");
        assert_eq!(fetched.content_html, "<p>updated</p>");
        assert_eq!(fetched.edited_at, Some(5000));
    }

    #[tokio::test]
    async fn soft_delete() {
        let pool = setup().await;
        create_post(&pool, &make_post(103)).await.unwrap();

        delete_post(&pool, 103, 6000, Some("spam")).await.unwrap();

        let fetched = get_post(&pool, 103).await.unwrap().unwrap();
        assert_eq!(fetched.deleted_at, Some(6000));
        assert_eq!(fetched.deleted_reason.as_deref(), Some("spam"));
    }

    #[tokio::test]
    async fn posts_by_persona_paginated() {
        let pool = setup().await;
        for i in 1..=5 {
            create_post(&pool, &make_post(i)).await.unwrap();
        }

        let page1 = posts_by_persona(&pool, 101, 3, None).await.unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page1[0].id, 5);
        assert_eq!(page1[2].id, 3);

        let page2 = posts_by_persona(&pool, 101, 3, Some(3)).await.unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].id, 2);
    }

    #[tokio::test]
    async fn posts_by_persona_excludes_deleted() {
        let pool = setup().await;
        create_post(&pool, &make_post(200)).await.unwrap();
        create_post(&pool, &make_post(201)).await.unwrap();
        delete_post(&pool, 200, 9000, None).await.unwrap();

        let posts = posts_by_persona(&pool, 101, 10, None).await.unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, 201);
    }

    #[tokio::test]
    async fn posts_count_excludes_deleted() {
        let pool = setup().await;
        create_post(&pool, &make_post(300)).await.unwrap();
        create_post(&pool, &make_post(301)).await.unwrap();
        assert_eq!(posts_count(&pool, 101).await.unwrap(), 2);

        delete_post(&pool, 300, 9000, None).await.unwrap();
        assert_eq!(posts_count(&pool, 101).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn exists_by_ap_id_found() {
        let pool = setup().await;
        create_post(&pool, &make_post(400)).await.unwrap();
        assert!(exists_by_ap_id(&pool, "https://example.com/posts/400").await.unwrap());
    }

    #[tokio::test]
    async fn exists_by_ap_id_not_found() {
        let pool = setup().await;
        assert!(!exists_by_ap_id(&pool, "https://example.com/posts/nope").await.unwrap());
    }

    #[tokio::test]
    async fn update_post_full_works() {
        let pool = setup().await;
        create_post(&pool, &make_post(401)).await.unwrap();

        update_post_full(&pool, 401, "edited", "<p>edited</p>", "cw", true, 9000)
            .await
            .unwrap();

        let fetched = get_post(&pool, 401).await.unwrap().unwrap();
        assert_eq!(fetched.content, "edited");
        assert_eq!(fetched.content_html, "<p>edited</p>");
        assert_eq!(fetched.spoiler_text, "cw");
        assert!(fetched.sensitive);
        assert_eq!(fetched.edited_at, Some(9000));
    }

    #[tokio::test]
    async fn hard_delete_removes_row() {
        let pool = setup().await;
        create_post(&pool, &make_post(402)).await.unwrap();
        hard_delete_post(&pool, 402).await.unwrap();
        assert!(get_post(&pool, 402).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn create_and_find_boost() {
        let pool = setup().await;
        create_post(&pool, &make_post(500)).await.unwrap();
        create_boost(&pool, 501, 1, 101, 500, "https://example.com/posts/500", "https://example.com/boosts/501", 3000)
            .await
            .unwrap();

        let found = find_boost(&pool, 101, 500).await.unwrap();
        assert_eq!(found, Some(501));
    }

    #[tokio::test]
    async fn find_boost_returns_none() {
        let pool = setup().await;
        assert!(find_boost(&pool, 101, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_boost_removes() {
        let pool = setup().await;
        create_post(&pool, &make_post(500)).await.unwrap();
        create_boost(&pool, 502, 1, 101, 500, "https://example.com/posts/500", "https://example.com/boosts/502", 3000)
            .await
            .unwrap();

        delete_boost(&pool, 101, 500).await.unwrap();
        assert!(find_boost(&pool, 101, 500).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn featured_posts_returns_pinned() {
        let pool = setup().await;
        create_post(&pool, &make_post(600)).await.unwrap();
        create_post(&pool, &make_post(601)).await.unwrap();

        // Pin one post
        crate::interactions_db::pin_post(&pool, 101, 600, 3000).await.unwrap();

        let pinned = featured_posts(&pool, 101).await.unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].id, 600);
    }

    #[tokio::test]
    async fn featured_posts_excludes_deleted() {
        let pool = setup().await;
        create_post(&pool, &make_post(700)).await.unwrap();
        crate::interactions_db::pin_post(&pool, 101, 700, 3000).await.unwrap();
        delete_post(&pool, 700, 9000, None).await.unwrap();

        let pinned = featured_posts(&pool, 101).await.unwrap();
        assert!(pinned.is_empty());
    }
}
