//! Database operations for notifications.
//!
//! Pure CRUD operations against the canonical `notifications` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.

use crate::db::Pool;

/// A row from the `notifications` table.
#[derive(Debug, Clone)]
pub struct NotificationRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub kind: String,
    pub from_persona_id: Option<i64>,
    pub from_remote_account_id: Option<i64>,
    pub post_id: Option<i64>,
    pub remote_post_id: Option<i64>,
    pub created_at: i64,
    pub read_at: Option<i64>,
}

type NotifTuple = (
    i64, i64, i64, String,
    Option<i64>, Option<i64>,
    Option<i64>, Option<i64>,
    i64, Option<i64>,
);

fn tuple_to_row(t: NotifTuple) -> NotificationRow {
    NotificationRow {
        id: t.0,
        user_id: t.1,
        persona_id: t.2,
        kind: t.3,
        from_persona_id: t.4,
        from_remote_account_id: t.5,
        post_id: t.6,
        remote_post_id: t.7,
        created_at: t.8,
        read_at: t.9,
    }
}

/// Insert a notification.
pub async fn create_notification(
    pool: &Pool,
    row: &NotificationRow,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO notifications \
                 (id, user_id, persona_id, kind, from_persona_id, from_remote_account_id, \
                  post_id, remote_post_id, created_at, read_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.persona_id)
            .bind(&row.kind)
            .bind(&row.from_persona_id)
            .bind(row.from_remote_account_id)
            .bind(row.post_id)
            .bind(row.remote_post_id)
            .bind(row.created_at)
            .bind(row.read_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO notifications \
                 (id, user_id, persona_id, kind, from_persona_id, from_remote_account_id, \
                  post_id, remote_post_id, created_at, read_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.persona_id)
            .bind(&row.kind)
            .bind(&row.from_persona_id)
            .bind(row.from_remote_account_id)
            .bind(row.post_id)
            .bind(row.remote_post_id)
            .bind(row.created_at)
            .bind(row.read_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get notifications for a user, newest first.
pub async fn get_notifications(
    pool: &Pool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<NotificationRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<NotifTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, kind, \
                 from_persona_id, from_remote_account_id, \
                 post_id, remote_post_id, created_at, read_at \
                 FROM notifications WHERE user_id = $1 \
                 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<NotifTuple> = sqlx::query_as(
                "SELECT id, user_id, persona_id, kind, \
                 from_persona_id, from_remote_account_id, \
                 post_id, remote_post_id, created_at, read_at \
                 FROM notifications WHERE user_id = ?1 \
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_row).collect())
        }
    }
}

/// Mark a single notification as read.
pub async fn mark_read(
    pool: &Pool,
    user_id: i64,
    notification_id: i64,
) -> Result<(), sqlx::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE notifications SET read_at = $1 \
                 WHERE id = $2 AND user_id = $3 AND read_at IS NULL",
            )
            .bind(now)
            .bind(notification_id)
            .bind(user_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE notifications SET read_at = ?1 \
                 WHERE id = ?2 AND user_id = ?3 AND read_at IS NULL",
            )
            .bind(now)
            .bind(notification_id)
            .bind(user_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Mark all notifications as read for a user.
pub async fn mark_all_read(pool: &Pool, user_id: i64, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE notifications SET read_at = $1 \
                 WHERE user_id = $2 AND read_at IS NULL",
            )
            .bind(now)
            .bind(user_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE notifications SET read_at = ?1 \
                 WHERE user_id = ?2 AND read_at IS NULL",
            )
            .bind(now)
            .bind(user_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete all notifications for a user.
pub async fn clear_notifications(pool: &Pool, user_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM notifications WHERE user_id = $1")
                .bind(user_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM notifications WHERE user_id = ?1")
                .bind(user_id)
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

    fn sample_notif(id: i64, kind: &str) -> NotificationRow {
        NotificationRow {
            id,
            user_id: 1,
            persona_id: 101,
            kind: kind.into(),
            from_persona_id: Some(102),
            from_remote_account_id: None,
            post_id: None,
            remote_post_id: None,
            created_at: 2000 + id,
            read_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let pool = setup().await;
        create_notification(&pool, &sample_notif(1, "follow")).await.unwrap();

        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].id, 1);
        assert_eq!(notifs[0].kind, "follow");
        assert_eq!(notifs[0].from_persona_id, Some(102));
        assert!(notifs[0].read_at.is_none());
    }

    #[tokio::test]
    async fn get_returns_newest_first() {
        let pool = setup().await;
        create_notification(&pool, &sample_notif(1, "follow")).await.unwrap();
        create_notification(&pool, &sample_notif(2, "mention")).await.unwrap();
        create_notification(&pool, &sample_notif(3, "favourite")).await.unwrap();

        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert_eq!(notifs.len(), 3);
        assert_eq!(notifs[0].id, 3);
        assert_eq!(notifs[2].id, 1);
    }

    #[tokio::test]
    async fn get_respects_limit() {
        let pool = setup().await;
        for i in 1..=5 {
            create_notification(&pool, &sample_notif(i, "follow")).await.unwrap();
        }
        let notifs = get_notifications(&pool, 1, 2).await.unwrap();
        assert_eq!(notifs.len(), 2);
    }

    #[tokio::test]
    async fn mark_read_sets_timestamp() {
        let pool = setup().await;
        create_notification(&pool, &sample_notif(1, "follow")).await.unwrap();
        mark_read(&pool, 1, 1).await.unwrap();

        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert!(notifs[0].read_at.is_some());
    }

    #[tokio::test]
    async fn mark_all_read_updates_all() {
        let pool = setup().await;
        create_notification(&pool, &sample_notif(1, "follow")).await.unwrap();
        create_notification(&pool, &sample_notif(2, "mention")).await.unwrap();
        mark_all_read(&pool, 1, 5000).await.unwrap();

        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert!(notifs.iter().all(|n| n.read_at == Some(5000)));
    }

    #[tokio::test]
    async fn clear_deletes_all() {
        let pool = setup().await;
        create_notification(&pool, &sample_notif(1, "follow")).await.unwrap();
        create_notification(&pool, &sample_notif(2, "mention")).await.unwrap();
        clear_notifications(&pool, 1).await.unwrap();

        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert!(notifs.is_empty());
    }

    #[tokio::test]
    async fn operations_on_empty_are_ok() {
        let pool = setup().await;
        mark_read(&pool, 1, 999).await.unwrap();
        mark_all_read(&pool, 1, 5000).await.unwrap();
        clear_notifications(&pool, 1).await.unwrap();
        let notifs = get_notifications(&pool, 1, 10).await.unwrap();
        assert!(notifs.is_empty());
    }
}
