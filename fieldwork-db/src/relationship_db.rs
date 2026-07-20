//! Composite relationship queries.
//!
//! Builds a [`Relationship`] struct by composing checks from
//! [`crate::follows_db`], [`crate::followers_db`], and
//! [`crate::interactions_db`].

use crate::db::Pool;

/// Composite relationship between a source persona and a target account.
#[derive(Debug, Clone, Default)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
    pub blocking: bool,
    pub blocked_by: bool,
    pub muting: bool,
    pub muting_notifications: bool,
    pub requested: bool,
    pub domain_blocking: bool,
    pub endorsed: bool,
    pub show_reblogs: bool,
    pub notifying: bool,
}

/// Build a composite relationship between a source persona and a target.
///
/// Exactly one of `target_persona_id` (local) or `target_remote_id` (remote)
/// should be `Some`.
pub async fn get_relationship(
    pool: &Pool,
    source_persona_id: i64,
    target_persona_id: Option<i64>,
    target_remote_id: Option<i64>,
) -> Result<Relationship, sqlx::Error> {
    let following = match (target_persona_id, target_remote_id) {
        (Some(local_id), _) => is_following_local(pool, source_persona_id, local_id).await?,
        (_, Some(remote_id)) => {
            crate::follows_db::is_following_remote(pool, source_persona_id, remote_id).await?
        }
        _ => false,
    };

    let followed_by = match (target_persona_id, target_remote_id) {
        (Some(local_id), _) => is_following_local(pool, local_id, source_persona_id).await?,
        (_, Some(remote_id)) => {
            crate::followers_db::is_following(pool, source_persona_id, remote_id).await?
        }
        _ => false,
    };

    let blocking =
        crate::interactions_db::is_blocked(pool, source_persona_id, target_persona_id, target_remote_id)
            .await?;

    let blocked_by = match target_persona_id {
        Some(local_id) => {
            crate::interactions_db::is_blocked(pool, local_id, Some(source_persona_id), None)
                .await?
        }
        _ => false,
    };

    let muting =
        crate::interactions_db::is_muted(pool, source_persona_id, target_persona_id, target_remote_id)
            .await?;

    let (show_reblogs, notifying) = match target_remote_id {
        Some(remote_id) => get_follow_flags(pool, source_persona_id, remote_id)
            .await?
            .unwrap_or((true, false)),
        None => (true, false),
    };

    Ok(Relationship {
        following,
        followed_by,
        blocking,
        blocked_by,
        muting,
        muting_notifications: muting,
        requested: false,
        domain_blocking: false,
        endorsed: false,
        show_reblogs,
        notifying,
    })
}

/// Get follow flags (show_reblogs, notify) for a follow relationship.
pub async fn get_follow_flags(
    pool: &Pool,
    persona_id: i64,
    target_remote_id: i64,
) -> Result<Option<(bool, bool)>, sqlx::Error> {
    let row: Option<(bool, bool)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT show_reblogs, notify FROM follows \
                 WHERE persona_id = $1 AND followee_remote_id = $2",
            )
            .bind(persona_id)
            .bind(target_remote_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT show_reblogs, notify FROM follows \
                 WHERE persona_id = ?1 AND followee_remote_id = ?2",
            )
            .bind(persona_id)
            .bind(target_remote_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row)
}

/// Check if persona A is following local persona B.
async fn is_following_local(
    pool: &Pool,
    persona_id: i64,
    target_persona_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT 1 FROM follows \
                 WHERE persona_id = $1 AND followee_persona_id = $2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT 1 FROM follows \
                 WHERE persona_id = ?1 AND followee_persona_id = ?2 LIMIT 1",
            )
            .bind(persona_id)
            .bind(target_persona_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.is_some())
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
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'pk2', 'pub2', 1000); \
             INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name, \
                 public_key_pem, public_key_id, inbox_url, last_fetched_at) \
             VALUES (1, 'https://remote/users/carol', 'carol', 'remote', 'Carol', \
                     'pk', 'https://remote/users/carol#key', 'https://remote/inbox', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn relationship_default_all_false() {
        let pool = setup().await;
        let rel = get_relationship(&pool, 101, Some(102), None).await.unwrap();
        assert!(!rel.following);
        assert!(!rel.followed_by);
        assert!(!rel.blocking);
        assert!(!rel.muting);
    }

    #[tokio::test]
    async fn relationship_following_local() {
        let pool = setup().await;
        crate::follows_db::follow_local(&pool, 101, 1, 102, 2000).await.unwrap();

        let rel = get_relationship(&pool, 101, Some(102), None).await.unwrap();
        assert!(rel.following);
        assert!(!rel.followed_by);
    }

    #[tokio::test]
    async fn relationship_followed_by_local() {
        let pool = setup().await;
        crate::follows_db::follow_local(&pool, 102, 1, 101, 2000).await.unwrap();

        let rel = get_relationship(&pool, 101, Some(102), None).await.unwrap();
        assert!(!rel.following);
        assert!(rel.followed_by);
    }

    #[tokio::test]
    async fn relationship_blocking() {
        let pool = setup().await;
        crate::interactions_db::block(&pool, 1, 101, Some(102), None, 2000).await.unwrap();

        let rel = get_relationship(&pool, 101, Some(102), None).await.unwrap();
        assert!(rel.blocking);
    }

    #[tokio::test]
    async fn relationship_muting() {
        let pool = setup().await;
        crate::interactions_db::mute(&pool, 1, 101, Some(102), None, 2000).await.unwrap();

        let rel = get_relationship(&pool, 101, Some(102), None).await.unwrap();
        assert!(rel.muting);
        assert!(rel.muting_notifications);
    }

    #[tokio::test]
    async fn relationship_following_remote() {
        let pool = setup().await;
        crate::follows_db::follow_remote(&pool, 101, 1, 1, 2000).await.unwrap();

        let rel = get_relationship(&pool, 101, None, Some(1)).await.unwrap();
        assert!(rel.following);
    }

    #[tokio::test]
    async fn get_follow_flags_returns_none_when_not_following() {
        let pool = setup().await;
        assert!(get_follow_flags(&pool, 101, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_follow_flags_returns_defaults() {
        let pool = setup().await;
        crate::follows_db::follow_remote(&pool, 101, 1, 1, 2000).await.unwrap();

        let flags = get_follow_flags(&pool, 101, 1).await.unwrap().unwrap();
        // Default values in the follows table
        assert!(flags.0); // show_reblogs defaults to true
        assert!(!flags.1); // notify defaults to false
    }
}
