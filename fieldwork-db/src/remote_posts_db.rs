//! Database operations for cached remote (federated) posts.
//!
//! Pure CRUD operations against the canonical `remote_posts` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit
//! the correct SQL dialect.

use crate::db::Pool;

/// A row from the `remote_posts` table.
#[derive(Debug, Clone)]
pub struct RemotePostRow {
    pub id: i64,
    pub ap_uri: String,
    pub remote_account_id: i64,
    pub in_reply_to_uri: Option<String>,
    pub content_html: String,
    pub spoiler_text: String,
    pub visibility: String,
    pub sensitive: bool,
    pub language: Option<String>,
    pub context_url: Option<String>,
    pub created_at: i64,
    pub fetched_at: i64,
}

type RemotePostTuple = (
    i64, String, i64, Option<String>,
    String, String, String, bool,
    Option<String>, Option<String>,
    i64, i64,
);

fn tuple_to_row(t: RemotePostTuple) -> RemotePostRow {
    RemotePostRow {
        id: t.0,
        ap_uri: t.1,
        remote_account_id: t.2,
        in_reply_to_uri: t.3,
        content_html: t.4,
        spoiler_text: t.5,
        visibility: t.6,
        sensitive: t.7,
        language: t.8,
        context_url: t.9,
        created_at: t.10,
        fetched_at: t.11,
    }
}

const COLUMNS: &str = "\
    id, ap_uri, remote_account_id, in_reply_to_uri, \
    content_html, spoiler_text, visibility, sensitive, \
    language, context_url, created_at, fetched_at";

/// Insert a remote post. Silently ignores duplicates (by ap_uri).
pub async fn insert_remote_post(
    pool: &Pool,
    post: &RemotePostRow,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO remote_posts (\
                    id, ap_uri, remote_account_id, in_reply_to_uri, \
                    content_html, spoiler_text, visibility, sensitive, \
                    language, context_url, created_at, fetched_at\
                 ) VALUES (\
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12\
                 ) ON CONFLICT (ap_uri) DO NOTHING",
            )
            .bind(post.id)
            .bind(&post.ap_uri)
            .bind(post.remote_account_id)
            .bind(&post.in_reply_to_uri)
            .bind(&post.content_html)
            .bind(&post.spoiler_text)
            .bind(&post.visibility)
            .bind(post.sensitive)
            .bind(&post.language)
            .bind(&post.context_url)
            .bind(post.created_at)
            .bind(post.fetched_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO remote_posts (\
                    id, ap_uri, remote_account_id, in_reply_to_uri, \
                    content_html, spoiler_text, visibility, sensitive, \
                    language, context_url, created_at, fetched_at\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12\
                 )",
            )
            .bind(post.id)
            .bind(&post.ap_uri)
            .bind(post.remote_account_id)
            .bind(&post.in_reply_to_uri)
            .bind(&post.content_html)
            .bind(&post.spoiler_text)
            .bind(&post.visibility)
            .bind(post.sensitive)
            .bind(&post.language)
            .bind(&post.context_url)
            .bind(post.created_at)
            .bind(post.fetched_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a remote post by its ActivityPub URI. Returns `None` if not found.
pub async fn get_by_uri(
    pool: &Pool,
    ap_uri: &str,
) -> Result<Option<RemotePostRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<RemotePostTuple> = sqlx::query_as(
                &format!("SELECT {COLUMNS} FROM remote_posts WHERE ap_uri = $1"),
            )
            .bind(ap_uri)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_row))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<RemotePostTuple> = sqlx::query_as(
                &format!("SELECT {COLUMNS} FROM remote_posts WHERE ap_uri = ?1"),
            )
            .bind(ap_uri)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_row))
        }
    }
}

/// Update a remote post's content fields and fetched_at timestamp.
pub async fn update_remote_post(
    pool: &Pool,
    id: i64,
    content_html: &str,
    spoiler_text: &str,
    sensitive: bool,
    fetched_at: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE remote_posts \
                 SET content_html = $1, spoiler_text = $2, sensitive = $3, fetched_at = $4 \
                 WHERE id = $5",
            )
            .bind(content_html)
            .bind(spoiler_text)
            .bind(sensitive)
            .bind(fetched_at)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE remote_posts \
                 SET content_html = ?1, spoiler_text = ?2, sensitive = ?3, fetched_at = ?4 \
                 WHERE id = ?5",
            )
            .bind(content_html)
            .bind(spoiler_text)
            .bind(sensitive)
            .bind(fetched_at)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete a remote post by its ActivityPub URI.
pub async fn delete_by_uri(pool: &Pool, ap_uri: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM remote_posts WHERE ap_uri = $1")
                .bind(ap_uri)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM remote_posts WHERE ap_uri = ?1")
                .bind(ap_uri)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Delete all remote posts by a remote account (cascade on account removal).
pub async fn delete_by_account(
    pool: &Pool,
    remote_account_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM remote_posts WHERE remote_account_id = $1")
                .bind(remote_account_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM remote_posts WHERE remote_account_id = ?1")
                .bind(remote_account_id)
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
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/bob', 'bob', 'remote', 'Bob', \
                     'pk', 'https://remote/users/bob#key', 'https://remote/inbox', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (2, 'https://other/users/carol', 'carol', 'other', 'Carol', \
                     'pk2', 'https://other/users/carol#key', 'https://other/inbox', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn make_remote_post(id: i64, account_id: i64) -> RemotePostRow {
        RemotePostRow {
            id,
            ap_uri: format!("https://remote/posts/{id}"),
            remote_account_id: account_id,
            in_reply_to_uri: None,
            content_html: "<p>hello from remote</p>".into(),
            spoiler_text: String::new(),
            visibility: "public".into(),
            sensitive: false,
            language: Some("en".into()),
            context_url: None,
            created_at: 2000 + id,
            fetched_at: 3000 + id,
        }
    }

    #[tokio::test]
    async fn insert_and_get() {
        let pool = setup().await;
        let post = make_remote_post(100, 1);
        insert_remote_post(&pool, &post).await.unwrap();

        let fetched = get_by_uri(&pool, "https://remote/posts/100")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.id, 100);
        assert_eq!(fetched.remote_account_id, 1);
        assert_eq!(fetched.content_html, "<p>hello from remote</p>");
    }

    #[tokio::test]
    async fn insert_is_idempotent() {
        let pool = setup().await;
        let post = make_remote_post(101, 1);
        insert_remote_post(&pool, &post).await.unwrap();
        // Second insert with same ap_uri should be silently ignored.
        let mut post2 = make_remote_post(999, 1);
        post2.ap_uri = "https://remote/posts/101".into();
        insert_remote_post(&pool, &post2).await.unwrap();

        let fetched = get_by_uri(&pool, "https://remote/posts/101")
            .await
            .unwrap()
            .unwrap();
        // Original id should remain, not 999.
        assert_eq!(fetched.id, 101);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_by_uri(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_content() {
        let pool = setup().await;
        insert_remote_post(&pool, &make_remote_post(102, 1)).await.unwrap();

        update_remote_post(&pool, 102, "<p>updated</p>", "cw", true, 9000)
            .await
            .unwrap();

        let fetched = get_by_uri(&pool, "https://remote/posts/102")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.content_html, "<p>updated</p>");
        assert_eq!(fetched.spoiler_text, "cw");
        assert!(fetched.sensitive);
        assert_eq!(fetched.fetched_at, 9000);
    }

    #[tokio::test]
    async fn delete_by_uri_removes() {
        let pool = setup().await;
        insert_remote_post(&pool, &make_remote_post(103, 1)).await.unwrap();
        delete_by_uri(&pool, "https://remote/posts/103").await.unwrap();

        assert!(get_by_uri(&pool, "https://remote/posts/103").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_by_uri_nonexistent_is_ok() {
        let pool = setup().await;
        delete_by_uri(&pool, "nope").await.unwrap();
    }

    #[tokio::test]
    async fn delete_by_account_removes_all() {
        let pool = setup().await;
        insert_remote_post(&pool, &make_remote_post(200, 1)).await.unwrap();
        insert_remote_post(&pool, &make_remote_post(201, 1)).await.unwrap();
        insert_remote_post(&pool, &make_remote_post(300, 2)).await.unwrap();

        delete_by_account(&pool, 1).await.unwrap();

        assert!(get_by_uri(&pool, "https://remote/posts/200").await.unwrap().is_none());
        assert!(get_by_uri(&pool, "https://remote/posts/201").await.unwrap().is_none());
        // Account 2's post should remain.
        assert!(get_by_uri(&pool, "https://remote/posts/300").await.unwrap().is_some());
    }
}
