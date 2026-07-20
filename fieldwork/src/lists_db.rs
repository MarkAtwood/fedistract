//! Database operations for user-curated follow lists.
//!
//! Pure CRUD operations against the canonical `lists` and `list_accounts`
//! tables defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// A row from the `lists` table.
#[derive(Debug, Clone)]
pub struct ListRow {
    pub id: i64,
    pub user_id: String,
    pub title: String,
    pub replies_policy: String,
    pub created_at: i64,
}

/// Create a new list.
pub async fn create_list(pool: &Pool, row: &ListRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO lists (id, user_id, title, replies_policy, created_at) \
                 VALUES ($1, $2::uuid, $3, $4, $5)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.title)
            .bind(&row.replies_policy)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO lists (id, user_id, title, replies_policy, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.title)
            .bind(&row.replies_policy)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a list by ID.
pub async fn get_list(pool: &Pool, id: i64) -> Result<Option<ListRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: Option<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, user_id::text, title, replies_policy, created_at \
                 FROM lists WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(|r| ListRow {
                id: r.0,
                user_id: r.1,
                title: r.2,
                replies_policy: r.3,
                created_at: r.4,
            }))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, user_id, title, replies_policy, created_at \
                 FROM lists WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(|r| ListRow {
                id: r.0,
                user_id: r.1,
                title: r.2,
                replies_policy: r.3,
                created_at: r.4,
            }))
        }
    }
}

/// Get all lists for a user.
pub async fn get_lists(pool: &Pool, user_id: &str) -> Result<Vec<ListRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, user_id::text, title, replies_policy, created_at \
                 FROM lists WHERE user_id = $1::uuid ORDER BY id",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| ListRow {
                    id: r.0,
                    user_id: r.1,
                    title: r.2,
                    replies_policy: r.3,
                    created_at: r.4,
                })
                .collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, user_id, title, replies_policy, created_at \
                 FROM lists WHERE user_id = ?1 ORDER BY id",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| ListRow {
                    id: r.0,
                    user_id: r.1,
                    title: r.2,
                    replies_policy: r.3,
                    created_at: r.4,
                })
                .collect())
        }
    }
}

/// Update a list's title and/or replies_policy.
pub async fn update_list(
    pool: &Pool,
    id: i64,
    title: Option<&str>,
    replies_policy: Option<&str>,
) -> Result<(), sqlx::Error> {
    // Fetch current values, then apply updates
    let current = get_list(pool, id).await?;
    let current = match current {
        Some(c) => c,
        None => return Ok(()),
    };
    let new_title = title.unwrap_or(&current.title);
    let new_rp = replies_policy.unwrap_or(&current.replies_policy);

    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE lists SET title = $1, replies_policy = $2 WHERE id = $3",
            )
            .bind(new_title)
            .bind(new_rp)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE lists SET title = ?1, replies_policy = ?2 WHERE id = ?3",
            )
            .bind(new_title)
            .bind(new_rp)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete a list (list_accounts cascade-deleted by FK).
pub async fn delete_list(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM lists WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM lists WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Add a member to a list. Silently ignores duplicates.
///
/// The UNIQUE constraint on `list_accounts` includes nullable columns, so
/// standard `ON CONFLICT` / `INSERT OR IGNORE` cannot deduplicate rows with
/// NULLs. We use `WHERE NOT EXISTS` with `IS NOT DISTINCT FROM` (Postgres)
/// or `IS` (SQLite) to handle NULL-safe equality.
pub async fn add_to_list(
    pool: &Pool,
    list_id: i64,
    persona_id: Option<&str>,
    remote_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO list_accounts (list_id, persona_id, remote_id) \
                 SELECT $1, $2::uuid, $3 \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM list_accounts \
                     WHERE list_id = $1 \
                     AND persona_id IS NOT DISTINCT FROM $2::uuid \
                     AND remote_id IS NOT DISTINCT FROM $3 \
                 )",
            )
            .bind(list_id)
            .bind(persona_id)
            .bind(remote_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO list_accounts (list_id, persona_id, remote_id) \
                 SELECT ?1, ?2, ?3 \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM list_accounts \
                     WHERE list_id = ?1 \
                     AND persona_id IS ?2 \
                     AND remote_id IS ?3 \
                 )",
            )
            .bind(list_id)
            .bind(persona_id)
            .bind(remote_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a member from a list.
pub async fn remove_from_list(
    pool: &Pool,
    list_id: i64,
    persona_id: Option<&str>,
    remote_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM list_accounts \
                 WHERE list_id = $1 \
                 AND persona_id IS NOT DISTINCT FROM $2::uuid \
                 AND remote_id IS NOT DISTINCT FROM $3",
            )
            .bind(list_id)
            .bind(persona_id)
            .bind(remote_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM list_accounts \
                 WHERE list_id = ?1 \
                 AND persona_id IS ?2 \
                 AND remote_id IS ?3",
            )
            .bind(list_id)
            .bind(persona_id)
            .bind(remote_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// List all members of a list as (persona_id, remote_id) pairs.
pub async fn list_members(
    pool: &Pool,
    list_id: i64,
) -> Result<Vec<(Option<String>, Option<i64>)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT persona_id::text, remote_id FROM list_accounts \
                 WHERE list_id = $1 ORDER BY persona_id, remote_id",
            )
            .bind(list_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT persona_id, remote_id FROM list_accounts \
                 WHERE list_id = ?1 ORDER BY persona_id, remote_id",
            )
            .bind(list_id)
            .fetch_all(sq)
            .await
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
             VALUES ('u1', 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES ('p1', 'u1', 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_get_list() {
        let pool = setup().await;
        let row = ListRow {
            id: 1,
            user_id: "u1".into(),
            title: "Friends".into(),
            replies_policy: "list".into(),
            created_at: 2000,
        };
        create_list(&pool, &row).await.unwrap();

        let fetched = get_list(&pool, 1).await.unwrap().expect("list should exist");
        assert_eq!(fetched.title, "Friends");
        assert_eq!(fetched.replies_policy, "list");
    }

    #[tokio::test]
    async fn get_lists_filters_by_user() {
        let pool = setup().await;
        create_list(
            &pool,
            &ListRow { id: 1, user_id: "u1".into(), title: "A".into(), replies_policy: "list".into(), created_at: 1000 },
        ).await.unwrap();
        create_list(
            &pool,
            &ListRow { id: 2, user_id: "u1".into(), title: "B".into(), replies_policy: "none".into(), created_at: 2000 },
        ).await.unwrap();

        let lists = get_lists(&pool, "u1").await.unwrap();
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].title, "A");

        let empty = get_lists(&pool, "u-none").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn update_list_partial() {
        let pool = setup().await;
        create_list(
            &pool,
            &ListRow { id: 1, user_id: "u1".into(), title: "Old".into(), replies_policy: "list".into(), created_at: 1000 },
        ).await.unwrap();

        update_list(&pool, 1, Some("New"), None).await.unwrap();
        let fetched = get_list(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.title, "New");
        assert_eq!(fetched.replies_policy, "list");
    }

    #[tokio::test]
    async fn delete_list_removes_members() {
        let pool = setup().await;
        create_list(
            &pool,
            &ListRow { id: 1, user_id: "u1".into(), title: "X".into(), replies_policy: "list".into(), created_at: 1000 },
        ).await.unwrap();
        add_to_list(&pool, 1, Some("p1"), None).await.unwrap();

        delete_list(&pool, 1).await.unwrap();
        assert!(get_list(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn add_remove_list_members() {
        let pool = setup().await;
        create_list(
            &pool,
            &ListRow { id: 1, user_id: "u1".into(), title: "X".into(), replies_policy: "list".into(), created_at: 1000 },
        ).await.unwrap();

        add_to_list(&pool, 1, Some("p1"), None).await.unwrap();
        // duplicate is ignored
        add_to_list(&pool, 1, Some("p1"), None).await.unwrap();

        let members = list_members(&pool, 1).await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].0.as_deref(), Some("p1"));
        assert!(members[0].1.is_none());

        remove_from_list(&pool, 1, Some("p1"), None).await.unwrap();
        let members = list_members(&pool, 1).await.unwrap();
        assert!(members.is_empty());
    }
}
