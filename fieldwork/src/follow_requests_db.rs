//! Database operations for inbound follow requests (pending approval).
//!
//! Pure CRUD against the canonical `follow_requests` table.

use crate::db::Pool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FollowRequestRow {
    pub id: i64,
    pub requester_remote_id: i64,
    pub target_persona_id: i64,
    pub ap_id: String,
    pub created_at: i64,
}

/// Create a follow request. The caller must provide a unique `id` and `ap_id`.
pub async fn create_request(pool: &Pool, row: &FollowRequestRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO follow_requests (id, requester_remote_id, target_persona_id, ap_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(row.id)
            .bind(row.requester_remote_id)
            .bind(&row.target_persona_id)
            .bind(&row.ap_id)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO follow_requests (id, requester_remote_id, target_persona_id, ap_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(row.id)
            .bind(row.requester_remote_id)
            .bind(&row.target_persona_id)
            .bind(&row.ap_id)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get pending follow requests for a persona, ordered by newest first.
pub async fn get_requests(
    pool: &Pool,
    persona_id: i64,
    limit: i64,
) -> Result<Vec<FollowRequestRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, requester_remote_id, target_persona_id, \
                 ap_id, created_at \
                 FROM follow_requests WHERE target_persona_id = $1 \
                 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(persona_id)
            .bind(limit)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, requester_remote_id, target_persona_id, ap_id, created_at \
                 FROM follow_requests WHERE target_persona_id = ?1 \
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .bind(persona_id)
            .bind(limit)
            .fetch_all(sq)
            .await
        }
    }
}

/// Look up a follow request by its ActivityPub ID.
pub async fn get_request_by_ap_id(
    pool: &Pool,
    ap_id: &str,
) -> Result<Option<FollowRequestRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, requester_remote_id, target_persona_id, \
                 ap_id, created_at \
                 FROM follow_requests WHERE ap_id = $1",
            )
            .bind(ap_id)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, requester_remote_id, target_persona_id, ap_id, created_at \
                 FROM follow_requests WHERE ap_id = ?1",
            )
            .bind(ap_id)
            .fetch_optional(sq)
            .await
        }
    }
}

/// Delete a follow request by ID (after accept or reject).
pub async fn delete_request(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM follow_requests WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM follow_requests WHERE id = ?1")
                .bind(id)
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

    #[tokio::test]
    async fn create_and_get_requests() {
        let pool = setup().await;
        let row = FollowRequestRow {
            id: 10,
            requester_remote_id: 1,
            target_persona_id: 101,
            ap_id: "https://remote/activities/follow-1".to_string(),
            created_at: 2000,
        };
        create_request(&pool, &row).await.unwrap();

        let reqs = get_requests(&pool, 101, 10).await.unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].requester_remote_id, 1);
    }

    #[tokio::test]
    async fn get_request_by_ap_id_found() {
        let pool = setup().await;
        let row = FollowRequestRow {
            id: 10,
            requester_remote_id: 1,
            target_persona_id: 101,
            ap_id: "https://remote/activities/follow-1".to_string(),
            created_at: 2000,
        };
        create_request(&pool, &row).await.unwrap();

        let found = get_request_by_ap_id(&pool, "https://remote/activities/follow-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, 10);
    }

    #[tokio::test]
    async fn get_request_by_ap_id_not_found() {
        let pool = setup().await;
        assert!(get_request_by_ap_id(&pool, "https://no/such/id")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn delete_request_removes() {
        let pool = setup().await;
        let row = FollowRequestRow {
            id: 10,
            requester_remote_id: 1,
            target_persona_id: 101,
            ap_id: "https://remote/activities/follow-1".to_string(),
            created_at: 2000,
        };
        create_request(&pool, &row).await.unwrap();
        delete_request(&pool, 10).await.unwrap();
        assert!(get_requests(&pool, 101, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_requests_respects_limit() {
        let pool = setup().await;
        for i in 0..5 {
            let row = FollowRequestRow {
                id: 10 + i,
                requester_remote_id: 1,
                target_persona_id: 101,
                ap_id: format!("https://remote/follow-{i}"),
                created_at: 2000 + i as i64,
            };
            create_request(&pool, &row).await.unwrap();
        }
        let reqs = get_requests(&pool, 101, 3).await.unwrap();
        assert_eq!(reqs.len(), 3);
    }
}
