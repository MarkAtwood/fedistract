//! Database operations for timeline markers (Mastodon Client API).
//!
//! Pure CRUD against the canonical `markers` table.

use crate::db::Pool;

/// Get a timeline marker. Returns (last_read_id, version, updated_at) if set.
pub async fn get_marker(
    pool: &Pool,
    user_id: i64,
    timeline: &str,
) -> Result<Option<(String, i32, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT last_read_id, version, updated_at \
                 FROM markers WHERE user_id = $1 AND timeline = $2",
            )
            .bind(user_id)
            .bind(timeline)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT last_read_id, version, updated_at \
                 FROM markers WHERE user_id = ?1 AND timeline = ?2",
            )
            .bind(user_id)
            .bind(timeline)
            .fetch_optional(sq)
            .await
        }
    }
}

/// Set a timeline marker. Upserts the row, incrementing the version on update.
pub async fn set_marker(
    pool: &Pool,
    user_id: i64,
    timeline: &str,
    last_read_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO markers (user_id, timeline, last_read_id, version, updated_at) \
                 VALUES ($1, $2, $3, 1, $4) \
                 ON CONFLICT (user_id, timeline) DO UPDATE SET \
                 last_read_id = $3, version = markers.version + 1, updated_at = $4",
            )
            .bind(user_id)
            .bind(timeline)
            .bind(last_read_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO markers (user_id, timeline, last_read_id, version, updated_at) \
                 VALUES (?1, ?2, ?3, 1, ?4) \
                 ON CONFLICT (user_id, timeline) DO UPDATE SET \
                 last_read_id = ?3, version = markers.version + 1, updated_at = ?4",
            )
            .bind(user_id)
            .bind(timeline)
            .bind(last_read_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
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
             VALUES (1, 'test@example.com', 'Test', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_marker(&pool, 1, "home").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn set_and_get_marker() {
        let pool = setup().await;
        set_marker(&pool, 1, "home", "12345", 2000).await.unwrap();

        let (last_read, version, updated_at) =
            get_marker(&pool, 1, "home").await.unwrap().unwrap();
        assert_eq!(last_read, "12345");
        assert_eq!(version, 1);
        assert_eq!(updated_at, 2000);
    }

    #[tokio::test]
    async fn set_marker_increments_version() {
        let pool = setup().await;
        set_marker(&pool, 1, "home", "100", 1000).await.unwrap();
        set_marker(&pool, 1, "home", "200", 2000).await.unwrap();
        set_marker(&pool, 1, "home", "300", 3000).await.unwrap();

        let (last_read, version, updated_at) =
            get_marker(&pool, 1, "home").await.unwrap().unwrap();
        assert_eq!(last_read, "300");
        assert_eq!(version, 3);
        assert_eq!(updated_at, 3000);
    }

    #[tokio::test]
    async fn different_timelines_independent() {
        let pool = setup().await;
        set_marker(&pool, 1, "home", "100", 1000).await.unwrap();
        set_marker(&pool, 1, "notifications", "200", 2000)
            .await
            .unwrap();

        let (home_id, _, _) = get_marker(&pool, 1, "home").await.unwrap().unwrap();
        let (notif_id, _, _) = get_marker(&pool, 1, "notifications")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(home_id, "100");
        assert_eq!(notif_id, "200");
    }
}
