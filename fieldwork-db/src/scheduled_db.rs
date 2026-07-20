//! Database operations for scheduled statuses.
//!
//! Pure CRUD operations against the canonical `scheduled_statuses` table
//! defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// A row from the `scheduled_statuses` table.
#[derive(Debug, Clone)]
pub struct ScheduledRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub scheduled_at: i64,
    pub params_json: String,
    pub created_at: i64,
}

type RowTuple = (i64, i64, i64, i64, String, i64);

fn tuple_to_row(t: RowTuple) -> ScheduledRow {
    ScheduledRow {
        id: t.0,
        user_id: t.1,
        persona_id: t.2,
        scheduled_at: t.3,
        params_json: t.4,
        created_at: t.5,
    }
}

/// Create a scheduled status.
pub async fn create_scheduled(pool: &Pool, row: &ScheduledRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO scheduled_statuses \
                 (id, user_id, persona_id, scheduled_at, params_json, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.persona_id)
            .bind(row.scheduled_at)
            .bind(&row.params_json)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO scheduled_statuses \
                 (id, user_id, persona_id, scheduled_at, params_json, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.persona_id)
            .bind(row.scheduled_at)
            .bind(&row.params_json)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a scheduled status by ID.
pub async fn get_scheduled(pool: &Pool, id: i64) -> Result<Option<ScheduledRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_row))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_row))
        }
    }
}

/// List all scheduled statuses for a user.
pub async fn list_scheduled(
    pool: &Pool,
    user_id: i64,
) -> Result<Vec<ScheduledRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE user_id = $1 ORDER BY scheduled_at",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE user_id = ?1 ORDER BY scheduled_at",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
    }
}

/// Update a scheduled status. Only provided fields are changed.
pub async fn update_scheduled(
    pool: &Pool,
    id: i64,
    scheduled_at: Option<i64>,
    params_json: Option<&str>,
) -> Result<(), sqlx::Error> {
    let current = match get_scheduled(pool, id).await? {
        Some(c) => c,
        None => return Ok(()),
    };
    let new_at = scheduled_at.unwrap_or(current.scheduled_at);
    let new_params = params_json.unwrap_or(&current.params_json);

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE scheduled_statuses SET scheduled_at = $1, params_json = $2 WHERE id = $3",
            )
            .bind(new_at)
            .bind(new_params)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE scheduled_statuses SET scheduled_at = ?1, params_json = ?2 WHERE id = ?3",
            )
            .bind(new_at)
            .bind(new_params)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete a scheduled status.
pub async fn delete_scheduled(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM scheduled_statuses WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM scheduled_statuses WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Fetch all scheduled statuses that are due (scheduled_at <= now).
pub async fn fetch_due(pool: &Pool, now: i64) -> Result<Vec<ScheduledRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE scheduled_at <= $1 ORDER BY scheduled_at",
            )
            .bind(now)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<RowTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, scheduled_at, params_json, created_at \
                 FROM scheduled_statuses WHERE scheduled_at <= ?1 ORDER BY scheduled_at",
            )
            .bind(now)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
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
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_row(id: i64, scheduled_at: i64) -> ScheduledRow {
        ScheduledRow {
            id,
            user_id: 1,
            persona_id: 101,
            scheduled_at,
            params_json: r#"{"content":"hello"}"#.into(),
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let pool = setup().await;
        create_scheduled(&pool, &sample_row(1, 5000)).await.unwrap();

        let fetched = get_scheduled(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.persona_id, 101);
        assert_eq!(fetched.scheduled_at, 5000);
    }

    #[tokio::test]
    async fn list_by_user() {
        let pool = setup().await;
        create_scheduled(&pool, &sample_row(1, 5000)).await.unwrap();
        create_scheduled(&pool, &sample_row(2, 3000)).await.unwrap();

        let list = list_scheduled(&pool, 1).await.unwrap();
        assert_eq!(list.len(), 2);
        // ordered by scheduled_at
        assert_eq!(list[0].scheduled_at, 3000);
        assert_eq!(list[1].scheduled_at, 5000);
    }

    #[tokio::test]
    async fn update_scheduled_partial() {
        let pool = setup().await;
        create_scheduled(&pool, &sample_row(1, 5000)).await.unwrap();

        update_scheduled(&pool, 1, Some(9000), None).await.unwrap();
        let fetched = get_scheduled(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.scheduled_at, 9000);
        assert_eq!(fetched.params_json, r#"{"content":"hello"}"#);
    }

    #[tokio::test]
    async fn delete_scheduled_works() {
        let pool = setup().await;
        create_scheduled(&pool, &sample_row(1, 5000)).await.unwrap();
        delete_scheduled(&pool, 1).await.unwrap();
        assert!(get_scheduled(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fetch_due_returns_ready() {
        let pool = setup().await;
        create_scheduled(&pool, &sample_row(1, 1000)).await.unwrap();
        create_scheduled(&pool, &sample_row(2, 5000)).await.unwrap();
        create_scheduled(&pool, &sample_row(3, 3000)).await.unwrap();

        let due = fetch_due(&pool, 3000).await.unwrap();
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].id, 1);
        assert_eq!(due[1].id, 3);
    }
}
