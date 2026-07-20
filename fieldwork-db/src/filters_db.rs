//! Database operations for content filters with keywords.
//!
//! Pure CRUD operations against the canonical `filters` and `filter_keywords`
//! tables defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// A row from the `filters` table.
#[derive(Debug, Clone)]
pub struct FilterRow {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub context: String,
    pub filter_action: String,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

/// A row from the `filter_keywords` table.
#[derive(Debug, Clone)]
pub struct FilterKeywordRow {
    pub id: i64,
    pub filter_id: i64,
    pub keyword: String,
    pub whole_word: bool,
}

/// Create a new filter.
pub async fn create_filter(pool: &Pool, filter: &FilterRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO filters \
                 (id, user_id, title, context, filter_action, expires_at, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(filter.id)
            .bind(&filter.user_id)
            .bind(&filter.title)
            .bind(&filter.context)
            .bind(&filter.filter_action)
            .bind(filter.expires_at)
            .bind(filter.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO filters \
                 (id, user_id, title, context, filter_action, expires_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(filter.id)
            .bind(&filter.user_id)
            .bind(&filter.title)
            .bind(&filter.context)
            .bind(&filter.filter_action)
            .bind(filter.expires_at)
            .bind(filter.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a filter by ID.
pub async fn get_filter(pool: &Pool, id: i64) -> Result<Option<FilterRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<(i64, i64, String, String, String, Option<i64>, i64)> =
                sqlx::query_as(
                    "SELECT id, user_id, title, context, filter_action, expires_at, created_at \
                     FROM filters WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(pg)
                .await?;
            Ok(row.map(|r| FilterRow {
                id: r.0,
                user_id: r.1,
                title: r.2,
                context: r.3,
                filter_action: r.4,
                expires_at: r.5,
                created_at: r.6,
            }))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<(i64, i64, String, String, String, Option<i64>, i64)> =
                sqlx::query_as(
                    "SELECT id, user_id, title, context, filter_action, expires_at, created_at \
                     FROM filters WHERE id = ?1",
                )
                .bind(id)
                .fetch_optional(sq)
                .await?;
            Ok(row.map(|r| FilterRow {
                id: r.0,
                user_id: r.1,
                title: r.2,
                context: r.3,
                filter_action: r.4,
                expires_at: r.5,
                created_at: r.6,
            }))
        }
    }
}

/// Get all filters for a user.
pub async fn get_filters(pool: &Pool, user_id: i64) -> Result<Vec<FilterRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, i64, String, String, String, Option<i64>, i64)> =
                sqlx::query_as(
                    "SELECT id, user_id, title, context, filter_action, expires_at, created_at \
                     FROM filters WHERE user_id = $1 ORDER BY id",
                )
                .bind(user_id)
                .fetch_all(pg)
                .await?;
            Ok(rows
                .into_iter()
                .map(|r| FilterRow {
                    id: r.0,
                    user_id: r.1,
                    title: r.2,
                    context: r.3,
                    filter_action: r.4,
                    expires_at: r.5,
                    created_at: r.6,
                })
                .collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, i64, String, String, String, Option<i64>, i64)> =
                sqlx::query_as(
                    "SELECT id, user_id, title, context, filter_action, expires_at, created_at \
                     FROM filters WHERE user_id = ?1 ORDER BY id",
                )
                .bind(user_id)
                .fetch_all(sq)
                .await?;
            Ok(rows
                .into_iter()
                .map(|r| FilterRow {
                    id: r.0,
                    user_id: r.1,
                    title: r.2,
                    context: r.3,
                    filter_action: r.4,
                    expires_at: r.5,
                    created_at: r.6,
                })
                .collect())
        }
    }
}

/// Update a filter. Only provided fields are changed.
pub async fn update_filter(
    pool: &Pool,
    id: i64,
    title: Option<&str>,
    context: Option<&str>,
    action: Option<&str>,
    expires: Option<Option<i64>>,
) -> Result<(), sqlx::Error> {
    let current = match get_filter(pool, id).await? {
        Some(c) => c,
        None => return Ok(()),
    };
    let new_title = title.unwrap_or(&current.title);
    let new_context = context.unwrap_or(&current.context);
    let new_action = action.unwrap_or(&current.filter_action);
    let new_expires = expires.unwrap_or(current.expires_at);

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE filters SET title = $1, context = $2, filter_action = $3, \
                 expires_at = $4 WHERE id = $5",
            )
            .bind(new_title)
            .bind(new_context)
            .bind(new_action)
            .bind(new_expires)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE filters SET title = ?1, context = ?2, filter_action = ?3, \
                 expires_at = ?4 WHERE id = ?5",
            )
            .bind(new_title)
            .bind(new_context)
            .bind(new_action)
            .bind(new_expires)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete a filter. Keywords are cascade-deleted by FK.
pub async fn delete_filter(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM filters WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM filters WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Add a keyword to a filter.
pub async fn add_keyword(pool: &Pool, kw: &FilterKeywordRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO filter_keywords (id, filter_id, keyword, whole_word) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(kw.id)
            .bind(kw.filter_id)
            .bind(&kw.keyword)
            .bind(kw.whole_word)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO filter_keywords (id, filter_id, keyword, whole_word) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(kw.id)
            .bind(kw.filter_id)
            .bind(&kw.keyword)
            .bind(kw.whole_word)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all keywords for a filter.
pub async fn get_keywords(
    pool: &Pool,
    filter_id: i64,
) -> Result<Vec<FilterKeywordRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, i64, String, bool)> = sqlx::query_as(
                "SELECT id, filter_id, keyword, whole_word \
                 FROM filter_keywords WHERE filter_id = $1 ORDER BY id",
            )
            .bind(filter_id)
            .fetch_all(pg)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| FilterKeywordRow {
                    id: r.0,
                    filter_id: r.1,
                    keyword: r.2,
                    whole_word: r.3,
                })
                .collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, i64, String, bool)> = sqlx::query_as(
                "SELECT id, filter_id, keyword, whole_word \
                 FROM filter_keywords WHERE filter_id = ?1 ORDER BY id",
            )
            .bind(filter_id)
            .fetch_all(sq)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| FilterKeywordRow {
                    id: r.0,
                    filter_id: r.1,
                    keyword: r.2,
                    whole_word: r.3,
                })
                .collect())
        }
    }
}

/// Delete a single keyword.
pub async fn delete_keyword(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM filter_keywords WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM filter_keywords WHERE id = ?1")
                .bind(id)
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
             VALUES (1, 'test@example.com', 'Test', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_get_filter() {
        let pool = setup().await;
        let f = FilterRow {
            id: 1,
            user_id: 1,
            title: "Spoilers".into(),
            context: "[\"home\"]".into(),
            filter_action: "warn".into(),
            expires_at: None,
            created_at: 2000,
        };
        create_filter(&pool, &f).await.unwrap();

        let fetched = get_filter(&pool, 1).await.unwrap().expect("filter should exist");
        assert_eq!(fetched.title, "Spoilers");
        assert_eq!(fetched.filter_action, "warn");
        assert!(fetched.expires_at.is_none());
    }

    #[tokio::test]
    async fn get_filters_by_user() {
        let pool = setup().await;
        create_filter(
            &pool,
            &FilterRow { id: 1, user_id: 1, title: "A".into(), context: "[]".into(), filter_action: "warn".into(), expires_at: None, created_at: 1000 },
        ).await.unwrap();
        create_filter(
            &pool,
            &FilterRow { id: 2, user_id: 1, title: "B".into(), context: "[]".into(), filter_action: "hide".into(), expires_at: Some(9999), created_at: 2000 },
        ).await.unwrap();

        let filters = get_filters(&pool, 1).await.unwrap();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[1].filter_action, "hide");
    }

    #[tokio::test]
    async fn update_filter_partial() {
        let pool = setup().await;
        create_filter(
            &pool,
            &FilterRow { id: 1, user_id: 1, title: "Old".into(), context: "[]".into(), filter_action: "warn".into(), expires_at: None, created_at: 1000 },
        ).await.unwrap();

        update_filter(&pool, 1, Some("New"), None, Some("hide"), None).await.unwrap();
        let fetched = get_filter(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.title, "New");
        assert_eq!(fetched.filter_action, "hide");
        assert_eq!(fetched.context, "[]");
    }

    #[tokio::test]
    async fn delete_filter_cascades_keywords() {
        let pool = setup().await;
        create_filter(
            &pool,
            &FilterRow { id: 1, user_id: 1, title: "X".into(), context: "[]".into(), filter_action: "warn".into(), expires_at: None, created_at: 1000 },
        ).await.unwrap();
        add_keyword(
            &pool,
            &FilterKeywordRow { id: 10, filter_id: 1, keyword: "spoiler".into(), whole_word: true },
        ).await.unwrap();

        delete_filter(&pool, 1).await.unwrap();
        assert!(get_filter(&pool, 1).await.unwrap().is_none());
        let kws = get_keywords(&pool, 1).await.unwrap();
        assert!(kws.is_empty());
    }

    #[tokio::test]
    async fn keyword_crud() {
        let pool = setup().await;
        create_filter(
            &pool,
            &FilterRow { id: 1, user_id: 1, title: "X".into(), context: "[]".into(), filter_action: "warn".into(), expires_at: None, created_at: 1000 },
        ).await.unwrap();

        add_keyword(&pool, &FilterKeywordRow { id: 10, filter_id: 1, keyword: "foo".into(), whole_word: true }).await.unwrap();
        add_keyword(&pool, &FilterKeywordRow { id: 11, filter_id: 1, keyword: "bar".into(), whole_word: false }).await.unwrap();

        let kws = get_keywords(&pool, 1).await.unwrap();
        assert_eq!(kws.len(), 2);
        assert_eq!(kws[0].keyword, "foo");
        assert!(kws[0].whole_word);
        assert!(!kws[1].whole_word);

        delete_keyword(&pool, 10).await.unwrap();
        let kws = get_keywords(&pool, 1).await.unwrap();
        assert_eq!(kws.len(), 1);
        assert_eq!(kws[0].keyword, "bar");
    }
}
