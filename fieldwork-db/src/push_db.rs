//! Database operations for Web Push subscriptions and VAPID key management.
//!
//! Pure CRUD against the canonical `push_subscriptions` and `vapid_keys` tables.

use crate::db::Pool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PushSubscriptionRow {
    pub id: i64,
    pub user_id: i64,
    pub endpoint: String,
    pub key_p256dh: String,
    pub key_auth: String,
    pub alerts_mention: bool,
    pub alerts_favourite: bool,
    pub alerts_reblog: bool,
    pub alerts_follow: bool,
    pub alerts_poll: bool,
    pub policy: String,
    pub created_at: i64,
}

/// Create a push subscription, replacing any existing one for this user.
pub async fn create_subscription(
    pool: &Pool,
    row: &PushSubscriptionRow,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1")
                .bind(&row.user_id)
                .execute(pg)
                .await?;
            sqlx::query(
                "INSERT INTO push_subscriptions \
                 (id, user_id, endpoint, key_p256dh, key_auth, \
                  alerts_mention, alerts_favourite, alerts_reblog, alerts_follow, alerts_poll, \
                  policy, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.endpoint)
            .bind(&row.key_p256dh)
            .bind(&row.key_auth)
            .bind(row.alerts_mention)
            .bind(row.alerts_favourite)
            .bind(row.alerts_reblog)
            .bind(row.alerts_follow)
            .bind(row.alerts_poll)
            .bind(&row.policy)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ?1")
                .bind(&row.user_id)
                .execute(sq)
                .await?;
            sqlx::query(
                "INSERT INTO push_subscriptions \
                 (id, user_id, endpoint, key_p256dh, key_auth, \
                  alerts_mention, alerts_favourite, alerts_reblog, alerts_follow, alerts_poll, \
                  policy, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .bind(row.id)
            .bind(&row.user_id)
            .bind(&row.endpoint)
            .bind(&row.key_p256dh)
            .bind(&row.key_auth)
            .bind(row.alerts_mention)
            .bind(row.alerts_favourite)
            .bind(row.alerts_reblog)
            .bind(row.alerts_follow)
            .bind(row.alerts_poll)
            .bind(&row.policy)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get the push subscription for a user, if any.
pub async fn get_subscription(
    pool: &Pool,
    user_id: i64,
) -> Result<Option<PushSubscriptionRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, user_id, endpoint, key_p256dh, key_auth, \
                 alerts_mention, alerts_favourite, alerts_reblog, alerts_follow, alerts_poll, \
                 policy, created_at \
                 FROM push_subscriptions WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, user_id, endpoint, key_p256dh, key_auth, \
                 alerts_mention, alerts_favourite, alerts_reblog, alerts_follow, alerts_poll, \
                 policy, created_at \
                 FROM push_subscriptions WHERE user_id = ?1",
            )
            .bind(user_id)
            .fetch_optional(sq)
            .await
        }
    }
}

/// Update alerts and policy on an existing subscription.
pub async fn update_subscription(
    pool: &Pool,
    id: i64,
    alerts_mention: bool,
    alerts_favourite: bool,
    alerts_reblog: bool,
    alerts_follow: bool,
    alerts_poll: bool,
    policy: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE push_subscriptions SET \
                 alerts_mention = $1, alerts_favourite = $2, alerts_reblog = $3, \
                 alerts_follow = $4, alerts_poll = $5, policy = $6 \
                 WHERE id = $7",
            )
            .bind(alerts_mention)
            .bind(alerts_favourite)
            .bind(alerts_reblog)
            .bind(alerts_follow)
            .bind(alerts_poll)
            .bind(policy)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE push_subscriptions SET \
                 alerts_mention = ?1, alerts_favourite = ?2, alerts_reblog = ?3, \
                 alerts_follow = ?4, alerts_poll = ?5, policy = ?6 \
                 WHERE id = ?7",
            )
            .bind(alerts_mention)
            .bind(alerts_favourite)
            .bind(alerts_reblog)
            .bind(alerts_follow)
            .bind(alerts_poll)
            .bind(policy)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete the push subscription for a user.
pub async fn delete_subscription(pool: &Pool, user_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1")
                .bind(user_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ?1")
                .bind(user_id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Get the VAPID keypair, if one has been stored.
pub async fn get_vapid_keys(pool: &Pool) -> Result<Option<(String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT private_key_pem, public_key_base64 FROM vapid_keys WHERE id = 1",
            )
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT private_key_pem, public_key_base64 FROM vapid_keys WHERE id = 1",
            )
            .fetch_optional(sq)
            .await
        }
    }
}

/// Store or replace the VAPID keypair.
pub async fn set_vapid_keys(
    pool: &Pool,
    private_pem: &str,
    public_base64: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO vapid_keys (id, private_key_pem, public_key_base64) \
                 VALUES (1, $1, $2) \
                 ON CONFLICT (id) DO UPDATE SET private_key_pem = $1, public_key_base64 = $2",
            )
            .bind(private_pem)
            .bind(public_base64)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR REPLACE INTO vapid_keys (id, private_key_pem, public_key_base64) \
                 VALUES (1, ?1, ?2)",
            )
            .bind(private_pem)
            .bind(public_base64)
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

    fn make_row() -> PushSubscriptionRow {
        PushSubscriptionRow {
            id: 100,
            user_id: 1,
            endpoint: "https://push.example.com/sub1".to_string(),
            key_p256dh: "p256dh-key".to_string(),
            key_auth: "auth-key".to_string(),
            alerts_mention: true,
            alerts_favourite: true,
            alerts_reblog: false,
            alerts_follow: true,
            alerts_poll: false,
            policy: "all".to_string(),
            created_at: 2000,
        }
    }

    #[tokio::test]
    async fn create_and_get_subscription() {
        let pool = setup().await;
        let row = make_row();
        create_subscription(&pool, &row).await.unwrap();

        let got = get_subscription(&pool, 1).await.unwrap().unwrap();
        assert_eq!(got.id, 100);
        assert_eq!(got.endpoint, "https://push.example.com/sub1");
        assert!(got.alerts_mention);
        assert!(!got.alerts_reblog);
    }

    #[tokio::test]
    async fn create_replaces_existing() {
        let pool = setup().await;
        let row = make_row();
        create_subscription(&pool, &row).await.unwrap();

        let mut row2 = make_row();
        row2.id = 200;
        row2.endpoint = "https://push.example.com/sub2".to_string();
        create_subscription(&pool, &row2).await.unwrap();

        let got = get_subscription(&pool, 1).await.unwrap().unwrap();
        assert_eq!(got.id, 200);
        assert_eq!(got.endpoint, "https://push.example.com/sub2");
    }

    #[tokio::test]
    async fn update_subscription_changes_alerts() {
        let pool = setup().await;
        let row = make_row();
        create_subscription(&pool, &row).await.unwrap();

        update_subscription(&pool, 100, false, false, true, false, true, "follower")
            .await
            .unwrap();

        let got = get_subscription(&pool, 1).await.unwrap().unwrap();
        assert!(!got.alerts_mention);
        assert!(got.alerts_reblog);
        assert!(got.alerts_poll);
        assert_eq!(got.policy, "follower");
    }

    #[tokio::test]
    async fn delete_subscription_removes() {
        let pool = setup().await;
        create_subscription(&pool, &make_row()).await.unwrap();
        delete_subscription(&pool, 1).await.unwrap();
        assert!(get_subscription(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_subscription(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn vapid_keys_roundtrip() {
        let pool = setup().await;
        assert!(get_vapid_keys(&pool).await.unwrap().is_none());

        set_vapid_keys(&pool, "pem-data", "base64-pub").await.unwrap();
        let (pem, pub_b64) = get_vapid_keys(&pool).await.unwrap().unwrap();
        assert_eq!(pem, "pem-data");
        assert_eq!(pub_b64, "base64-pub");
    }

    #[tokio::test]
    async fn vapid_keys_replace() {
        let pool = setup().await;
        set_vapid_keys(&pool, "pem1", "pub1").await.unwrap();
        set_vapid_keys(&pool, "pem2", "pub2").await.unwrap();
        let (pem, _) = get_vapid_keys(&pool).await.unwrap().unwrap();
        assert_eq!(pem, "pem2");
    }
}
