//! Database operations for WriteFreely-style long-form articles and collections.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CollectionRow {
    pub alias: String,
    pub user_id: i64,
    pub persona_id: i64,
    pub title: String,
    pub description: String,
    pub style_sheet: String,
    pub visibility: String,
    pub created_at: i64,
}

type CollectionTuple = (String, i64, i64, String, String, String, String, i64);

fn tuple_to_collection(t: CollectionTuple) -> CollectionRow {
    CollectionRow {
        alias: t.0,
        user_id: t.1,
        persona_id: t.2,
        title: t.3,
        description: t.4,
        style_sheet: t.5,
        visibility: t.6,
        created_at: t.7,
    }
}

const COLLECTION_COLS: &str =
    "alias, user_id, persona_id, title, description, style_sheet, visibility, created_at";

#[derive(Debug, Clone)]
pub struct ArticleRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub collection_alias: Option<String>,
    pub slug: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: String,
    pub font: String,
    pub language: Option<String>,
    pub rtl: bool,
    pub pinned: bool,
    pub pin_position: Option<i32>,
    pub draft: bool,
    pub ap_id: String,
    pub edit_token: Option<String>,
    pub views: i64,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

#[cfg(feature = "sqlite")]
fn sqlite_row_to_article(r: sqlx::sqlite::SqliteRow) -> ArticleRow {
    use sqlx::Row;
    ArticleRow {
        id: r.get(0), user_id: r.get(1), persona_id: r.get(2),
        collection_alias: r.get(3), slug: r.get(4),
        title: r.get(5), body: r.get(6), body_html: r.get(7),
        font: r.get(8), language: r.get(9),
        rtl: { let v: i32 = r.get(10); v != 0 },
        pinned: { let v: i32 = r.get(11); v != 0 },
        pin_position: r.get(12),
        draft: { let v: i32 = r.get(13); v != 0 },
        ap_id: r.get(14), edit_token: r.get(15),
        views: r.get(16), created_at: r.get(17), updated_at: r.get(18),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_article(r: sqlx::postgres::PgRow) -> ArticleRow {
    use sqlx::Row;
    ArticleRow {
        id: r.get(0), user_id: r.get(1), persona_id: r.get(2),
        collection_alias: r.get(3), slug: r.get(4),
        title: r.get(5), body: r.get(6), body_html: r.get(7),
        font: r.get(8), language: r.get(9),
        rtl: r.get(10), pinned: r.get(11),
        pin_position: r.get(12),
        draft: r.get(13),
        ap_id: r.get(14), edit_token: r.get(15),
        views: r.get(16), created_at: r.get(17), updated_at: r.get(18),
    }
}

const ARTICLE_COLS: &str = "\
    id, user_id, persona_id, collection_alias, slug, \
    title, body, body_html, font, language, \
    rtl, pinned, pin_position, draft, ap_id, \
    edit_token, views, created_at, updated_at";

// ---------------------------------------------------------------------------
// Collection CRUD
// ---------------------------------------------------------------------------

pub async fn create_collection(pool: &Pool, row: &CollectionRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO collections (alias, user_id, persona_id, title, description, style_sheet, visibility, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&row.alias).bind(row.user_id).bind(row.persona_id)
            .bind(&row.title).bind(&row.description).bind(&row.style_sheet)
            .bind(&row.visibility).bind(row.created_at)
            .execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO collections (alias, user_id, persona_id, title, description, style_sheet, visibility, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(&row.alias).bind(row.user_id).bind(row.persona_id)
            .bind(&row.title).bind(&row.description).bind(&row.style_sheet)
            .bind(&row.visibility).bind(row.created_at)
            .execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn get_collection(pool: &Pool, alias: &str) -> Result<Option<CollectionRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<CollectionTuple> = sqlx::query_as(
                &format!("SELECT {COLLECTION_COLS} FROM collections WHERE alias = $1"),
            ).bind(alias).fetch_optional(pg).await?;
            Ok(row.map(tuple_to_collection))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<CollectionTuple> = sqlx::query_as(
                &format!("SELECT {COLLECTION_COLS} FROM collections WHERE alias = ?1"),
            ).bind(alias).fetch_optional(sq).await?;
            Ok(row.map(tuple_to_collection))
        }
    }
}

pub async fn list_user_collections(pool: &Pool, user_id: i64) -> Result<Vec<CollectionRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<CollectionTuple> = sqlx::query_as(
                &format!("SELECT {COLLECTION_COLS} FROM collections WHERE user_id = $1 ORDER BY created_at DESC"),
            ).bind(user_id).fetch_all(pg).await?;
            Ok(rows.into_iter().map(tuple_to_collection).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<CollectionTuple> = sqlx::query_as(
                &format!("SELECT {COLLECTION_COLS} FROM collections WHERE user_id = ?1 ORDER BY created_at DESC"),
            ).bind(user_id).fetch_all(sq).await?;
            Ok(rows.into_iter().map(tuple_to_collection).collect())
        }
    }
}

pub async fn update_collection(
    pool: &Pool,
    alias: &str,
    title: Option<&str>,
    description: Option<&str>,
    style_sheet: Option<&str>,
    visibility: Option<&str>,
) -> Result<(), sqlx::Error> {
    let mut sets = Vec::new();
    let mut vals: Vec<String> = Vec::new();
    if let Some(t) = title { sets.push("title"); vals.push(t.to_string()); }
    if let Some(d) = description { sets.push("description"); vals.push(d.to_string()); }
    if let Some(s) = style_sheet { sets.push("style_sheet"); vals.push(s.to_string()); }
    if let Some(v) = visibility { sets.push("visibility"); vals.push(v.to_string()); }
    if sets.is_empty() { return Ok(()); }

    // Build dynamic SET clause
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let set_clause: String = sets.iter().enumerate()
                .map(|(i, col)| format!("{col} = ${}", i + 1))
                .collect::<Vec<_>>().join(", ");
            let where_idx = sets.len() + 1;
            let sql = format!("UPDATE collections SET {set_clause} WHERE alias = ${where_idx}");
            let mut q = sqlx::query(&sql);
            for v in &vals { q = q.bind(v); }
            q = q.bind(alias);
            q.execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let set_clause: String = sets.iter().enumerate()
                .map(|(i, col)| format!("{col} = ?{}", i + 1))
                .collect::<Vec<_>>().join(", ");
            let where_idx = sets.len() + 1;
            let sql = format!("UPDATE collections SET {set_clause} WHERE alias = ?{where_idx}");
            let mut q = sqlx::query(&sql);
            for v in &vals { q = q.bind(v); }
            q = q.bind(alias);
            q.execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn delete_collection(pool: &Pool, alias: &str) -> Result<(), sqlx::Error> {
    // Unlink articles first (set collection_alias = NULL)
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE articles SET collection_alias = NULL WHERE collection_alias = $1")
                .bind(alias).execute(pg).await?;
            sqlx::query("DELETE FROM collections WHERE alias = $1")
                .bind(alias).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE articles SET collection_alias = NULL WHERE collection_alias = ?1")
                .bind(alias).execute(sq).await?;
            sqlx::query("DELETE FROM collections WHERE alias = ?1")
                .bind(alias).execute(sq).await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Article CRUD
// ---------------------------------------------------------------------------

pub async fn create_article(pool: &Pool, a: &ArticleRow) -> Result<(), sqlx::Error> {
    let rtl_int: i32 = if a.rtl { 1 } else { 0 };
    let pinned_int: i32 = if a.pinned { 1 } else { 0 };
    let draft_int: i32 = if a.draft { 1 } else { 0 };
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(&format!(
                "INSERT INTO articles ({ARTICLE_COLS}) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)"
            ))
            .bind(a.id).bind(a.user_id).bind(a.persona_id)
            .bind(&a.collection_alias).bind(&a.slug)
            .bind(&a.title).bind(&a.body).bind(&a.body_html)
            .bind(&a.font).bind(&a.language)
            .bind(rtl_int).bind(pinned_int).bind(a.pin_position)
            .bind(draft_int).bind(&a.ap_id)
            .bind(&a.edit_token).bind(a.views)
            .bind(a.created_at).bind(a.updated_at)
            .execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(&format!(
                "INSERT INTO articles ({ARTICLE_COLS}) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)"
            ))
            .bind(a.id).bind(a.user_id).bind(a.persona_id)
            .bind(&a.collection_alias).bind(&a.slug)
            .bind(&a.title).bind(&a.body).bind(&a.body_html)
            .bind(&a.font).bind(&a.language)
            .bind(rtl_int).bind(pinned_int).bind(a.pin_position)
            .bind(draft_int).bind(&a.ap_id)
            .bind(&a.edit_token).bind(a.views)
            .bind(a.created_at).bind(a.updated_at)
            .execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn get_article(pool: &Pool, id: i64) -> Result<Option<ArticleRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {ARTICLE_COLS} FROM articles WHERE id = $1"),
            ).bind(id).fetch_optional(pg).await?;
            Ok(row.map(pg_row_to_article))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {ARTICLE_COLS} FROM articles WHERE id = ?1"),
            ).bind(id).fetch_optional(sq).await?;
            Ok(row.map(sqlite_row_to_article))
        }
    }
}

pub async fn get_article_by_slug(
    pool: &Pool,
    collection_alias: &str,
    slug: &str,
) -> Result<Option<ArticleRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {ARTICLE_COLS} FROM articles WHERE collection_alias = $1 AND slug = $2"),
            ).bind(collection_alias).bind(slug).fetch_optional(pg).await?;
            Ok(row.map(pg_row_to_article))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {ARTICLE_COLS} FROM articles WHERE collection_alias = ?1 AND slug = ?2"),
            ).bind(collection_alias).bind(slug).fetch_optional(sq).await?;
            Ok(row.map(sqlite_row_to_article))
        }
    }
}

pub async fn list_collection_articles(
    pool: &Pool,
    alias: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<ArticleRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(&format!(
                "SELECT {ARTICLE_COLS} FROM articles \
                 WHERE collection_alias = $1 AND draft = 0 \
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            )).bind(alias).bind(limit).bind(offset).fetch_all(pg).await?;
            Ok(rows.into_iter().map(pg_row_to_article).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(&format!(
                "SELECT {ARTICLE_COLS} FROM articles \
                 WHERE collection_alias = ?1 AND draft = 0 \
                 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
            )).bind(alias).bind(limit).bind(offset).fetch_all(sq).await?;
            Ok(rows.into_iter().map(sqlite_row_to_article).collect())
        }
    }
}

pub async fn list_user_articles(
    pool: &Pool,
    user_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<ArticleRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(&format!(
                "SELECT {ARTICLE_COLS} FROM articles \
                 WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            )).bind(user_id).bind(limit).bind(offset).fetch_all(pg).await?;
            Ok(rows.into_iter().map(pg_row_to_article).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(&format!(
                "SELECT {ARTICLE_COLS} FROM articles \
                 WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
            )).bind(user_id).bind(limit).bind(offset).fetch_all(sq).await?;
            Ok(rows.into_iter().map(sqlite_row_to_article).collect())
        }
    }
}

pub async fn update_article(
    pool: &Pool,
    id: i64,
    body: Option<&str>,
    body_html: Option<&str>,
    title: Option<&str>,
    font: Option<&str>,
    language: Option<&str>,
    rtl: Option<bool>,
    now: i64,
) -> Result<(), sqlx::Error> {
    // Simple approach: always update the fields that were provided + updated_at
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE articles SET \
                 body = COALESCE($1, body), \
                 body_html = COALESCE($2, body_html), \
                 title = COALESCE($3, title), \
                 font = COALESCE($4, font), \
                 language = COALESCE($5, language), \
                 rtl = COALESCE($6, rtl), \
                 updated_at = $7 \
                 WHERE id = $8"
            )
            .bind(body).bind(body_html).bind(title)
            .bind(font).bind(language)
            .bind(rtl.map(|b| if b { 1i32 } else { 0 }))
            .bind(now).bind(id)
            .execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE articles SET \
                 body = COALESCE(?1, body), \
                 body_html = COALESCE(?2, body_html), \
                 title = COALESCE(?3, title), \
                 font = COALESCE(?4, font), \
                 language = COALESCE(?5, language), \
                 rtl = COALESCE(?6, rtl), \
                 updated_at = ?7 \
                 WHERE id = ?8"
            )
            .bind(body).bind(body_html).bind(title)
            .bind(font).bind(language)
            .bind(rtl.map(|b| if b { 1i32 } else { 0 }))
            .bind(now).bind(id)
            .execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn delete_article(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM articles WHERE id = $1")
                .bind(id).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM articles WHERE id = ?1")
                .bind(id).execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn increment_views(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE articles SET views = views + 1 WHERE id = $1")
                .bind(id).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE articles SET views = views + 1 WHERE id = ?1")
                .bind(id).execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn pin_article(pool: &Pool, id: i64, position: i32) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE articles SET pinned = 1, pin_position = $1 WHERE id = $2")
                .bind(position).bind(id).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE articles SET pinned = 1, pin_position = ?1 WHERE id = ?2")
                .bind(position).bind(id).execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn unpin_article(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE articles SET pinned = 0, pin_position = NULL WHERE id = $1")
                .bind(id).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE articles SET pinned = 0, pin_position = NULL WHERE id = ?1")
                .bind(id).execute(sq).await?;
        }
    }
    Ok(())
}

pub async fn move_to_collection(
    pool: &Pool,
    article_id: i64,
    collection_alias: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE articles SET collection_alias = $1 WHERE id = $2")
                .bind(collection_alias).bind(article_id).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE articles SET collection_alias = ?1 WHERE id = ?2")
                .bind(collection_alias).bind(article_id).execute(sq).await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_collection() -> CollectionRow {
        CollectionRow {
            alias: "my-blog".into(),
            user_id: 1,
            persona_id: 101,
            title: "My Blog".into(),
            description: "A personal blog".into(),
            style_sheet: String::new(),
            visibility: "public".into(),
            created_at: 2000,
        }
    }

    fn sample_article(id: i64) -> ArticleRow {
        ArticleRow {
            id,
            user_id: 1,
            persona_id: 101,
            collection_alias: Some("my-blog".into()),
            slug: Some("hello-world".into()),
            title: "Hello World".into(),
            body: "# Hello\n\nThis is my first post.".into(),
            body_html: "<h1>Hello</h1>\n<p>This is my first post.</p>".into(),
            font: "norm".into(),
            language: Some("en".into()),
            rtl: false,
            pinned: false,
            pin_position: None,
            draft: false,
            ap_id: format!("https://example.com/articles/{id}"),
            edit_token: None,
            views: 0,
            created_at: 3000,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get_collection() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        let c = get_collection(&pool, "my-blog").await.unwrap().unwrap();
        assert_eq!(c.title, "My Blog");
        assert_eq!(c.visibility, "public");
    }

    #[tokio::test]
    async fn list_user_collections_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        let cols = list_user_collections(&pool, 1).await.unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].alias, "my-blog");
    }

    #[tokio::test]
    async fn delete_collection_unlinks_articles() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        delete_collection(&pool, "my-blog").await.unwrap();
        assert!(get_collection(&pool, "my-blog").await.unwrap().is_none());
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert!(a.collection_alias.is_none());
    }

    #[tokio::test]
    async fn create_and_get_article() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert_eq!(a.title, "Hello World");
        assert_eq!(a.font, "norm");
        assert!(!a.draft);
    }

    #[tokio::test]
    async fn get_article_by_slug_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        let a = get_article_by_slug(&pool, "my-blog", "hello-world").await.unwrap().unwrap();
        assert_eq!(a.id, 1);
    }

    #[tokio::test]
    async fn list_collection_articles_excludes_drafts() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        let mut draft = sample_article(2);
        draft.slug = Some("draft-post".into());
        draft.draft = true;
        draft.ap_id = "https://example.com/articles/2".into();
        create_article(&pool, &draft).await.unwrap();
        let articles = list_collection_articles(&pool, "my-blog", 10, 0).await.unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].id, 1);
    }

    #[tokio::test]
    async fn update_article_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        update_article(&pool, 1, Some("Updated body"), Some("<p>Updated body</p>"), Some("New Title"), None, None, None, 4000).await.unwrap();
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert_eq!(a.title, "New Title");
        assert_eq!(a.body, "Updated body");
        assert_eq!(a.updated_at, Some(4000));
    }

    #[tokio::test]
    async fn delete_article_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        delete_article(&pool, 1).await.unwrap();
        assert!(get_article(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn increment_views_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        increment_views(&pool, 1).await.unwrap();
        increment_views(&pool, 1).await.unwrap();
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert_eq!(a.views, 2);
    }

    #[tokio::test]
    async fn pin_unpin_works() {
        let pool = setup().await;
        create_collection(&pool, &sample_collection()).await.unwrap();
        create_article(&pool, &sample_article(1)).await.unwrap();
        pin_article(&pool, 1, 1).await.unwrap();
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert!(a.pinned);
        assert_eq!(a.pin_position, Some(1));
        unpin_article(&pool, 1).await.unwrap();
        let a = get_article(&pool, 1).await.unwrap().unwrap();
        assert!(!a.pinned);
        assert!(a.pin_position.is_none());
    }

    #[tokio::test]
    async fn anonymous_article_with_edit_token() {
        let pool = setup().await;
        let mut a = sample_article(1);
        a.collection_alias = None;
        a.slug = None;
        a.edit_token = Some("secret-token-123".into());
        create_article(&pool, &a).await.unwrap();
        let fetched = get_article(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.edit_token.as_deref(), Some("secret-token-123"));
        assert!(fetched.collection_alias.is_none());
    }
}
