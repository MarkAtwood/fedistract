//! Relay subscription database operations.
//!
//! Pure data-layer module for managing ActivityPub relay subscriptions.
//! Each function dispatches on [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;
use std::fmt;
use std::str::FromStr;

/// Relay subscription state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayState {
    Pending,
    Accepted,
    Rejected,
}

impl fmt::Display for RelayState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => f.write_str("pending"),
            Self::Accepted => f.write_str("accepted"),
            Self::Rejected => f.write_str("rejected"),
        }
    }
}

impl FromStr for RelayState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            other => Err(format!("unknown relay state: {other}")),
        }
    }
}

#[cfg(feature = "activities")]
impl serde::Serialize for RelayState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "activities")]
impl<'de> serde::Deserialize<'de> for RelayState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A row from the `relays` table.
#[derive(Debug, Clone)]
pub struct RelayRow {
    pub id: i64,
    pub actor_uri: String,
    pub inbox_url: String,
    pub persona_id: Option<String>,
    pub follow_id: String,
    pub state: RelayState,
    pub created_at: i64,
}

type RawRow = (i64, String, String, Option<String>, String, String, i64);

fn raw_to_relay(row: RawRow) -> RelayRow {
    RelayRow {
        id: row.0,
        actor_uri: row.1,
        inbox_url: row.2,
        persona_id: row.3,
        follow_id: row.4,
        state: RelayState::from_str(&row.5).unwrap_or(RelayState::Pending),
        created_at: row.6,
    }
}

/// Insert a new relay subscription with state `Pending`. Returns the generated row ID.
pub async fn subscribe(
    pool: &Pool,
    actor_uri: &str,
    inbox_url: &str,
    persona_id: Option<&str>,
    follow_id: &str,
) -> Result<i64, sqlx::Error> {
    let id = crate::id::generate_id();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO relays (id, actor_uri, inbox_url, persona_id, follow_id, state, created_at) \
                 VALUES ($1, $2, $3, $4::uuid, $5, 'pending', $6)",
            )
            .bind(id)
            .bind(actor_uri)
            .bind(inbox_url)
            .bind(persona_id)
            .bind(follow_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO relays (id, actor_uri, inbox_url, persona_id, follow_id, state, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
            )
            .bind(id)
            .bind(actor_uri)
            .bind(inbox_url)
            .bind(persona_id)
            .bind(follow_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(id)
}

/// Mark a relay subscription as accepted.
pub async fn accept(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE relays SET state = 'accepted' WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE relays SET state = 'accepted' WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Mark a relay subscription as rejected.
pub async fn reject(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE relays SET state = 'rejected' WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE relays SET state = 'rejected' WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Delete a relay subscription.
pub async fn unsubscribe(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM relays WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM relays WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Fetch all accepted relay subscriptions.
pub async fn get_accepted(pool: &Pool) -> Result<Vec<RelayRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<RawRow> = sqlx::query_as(
                "SELECT id, actor_uri, inbox_url, persona_id::text, follow_id, state, created_at \
                 FROM relays WHERE state = 'accepted'",
            )
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(raw_to_relay).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<RawRow> = sqlx::query_as(
                "SELECT id, actor_uri, inbox_url, persona_id, follow_id, state, created_at \
                 FROM relays WHERE state = 'accepted'",
            )
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(raw_to_relay).collect())
        }
    }
}

/// Find a relay subscription by actor URI.
pub async fn find_by_actor(
    pool: &Pool,
    actor_uri: &str,
) -> Result<Option<RelayRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: Option<RawRow> = sqlx::query_as(
                "SELECT id, actor_uri, inbox_url, persona_id::text, follow_id, state, created_at \
                 FROM relays WHERE actor_uri = $1",
            )
            .bind(actor_uri)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(raw_to_relay))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<RawRow> = sqlx::query_as(
                "SELECT id, actor_uri, inbox_url, persona_id, follow_id, state, created_at \
                 FROM relays WHERE actor_uri = ?1",
            )
            .bind(actor_uri)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(raw_to_relay))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[cfg(feature = "db-sqlite")]
    async fn test_pool() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq);
        db::migrate(&pool).await.unwrap();
        pool
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn subscribe_creates_pending_relay() {
        let pool = test_pool().await;

        let id = subscribe(&pool, "https://relay.example/actor", "https://relay.example/inbox", None, "follow-1")
            .await
            .unwrap();
        assert!(id > 0);

        let row = find_by_actor(&pool, "https://relay.example/actor")
            .await
            .unwrap()
            .expect("relay should exist");

        assert_eq!(row.id, id);
        assert_eq!(row.actor_uri, "https://relay.example/actor");
        assert_eq!(row.inbox_url, "https://relay.example/inbox");
        assert!(row.persona_id.is_none());
        assert_eq!(row.follow_id, "follow-1");
        assert_eq!(row.state, RelayState::Pending);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn subscribe_with_persona() {
        let pool = test_pool().await;

        // Need a user and persona to satisfy FK
        let Pool::Sqlite(sq) = &pool;
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES ('u1', 'test@example.com', 'Test', 'user', 1700000000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES ('p1', 'u1', 'alice', 'pk', 'pub', 1700000000);",
        )
        .execute(sq)
        .await
        .unwrap();

        let id = subscribe(&pool, "https://relay.example/actor", "https://relay.example/inbox", Some("p1"), "follow-2")
            .await
            .unwrap();

        let row = find_by_actor(&pool, "https://relay.example/actor")
            .await
            .unwrap()
            .expect("relay should exist");

        assert_eq!(row.id, id);
        assert_eq!(row.persona_id.as_deref(), Some("p1"));
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn accept_updates_state() {
        let pool = test_pool().await;

        let id = subscribe(&pool, "https://relay.example/actor", "https://relay.example/inbox", None, "f1")
            .await
            .unwrap();

        accept(&pool, id).await.unwrap();

        let row = find_by_actor(&pool, "https://relay.example/actor")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, RelayState::Accepted);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn reject_updates_state() {
        let pool = test_pool().await;

        let id = subscribe(&pool, "https://relay.example/actor", "https://relay.example/inbox", None, "f1")
            .await
            .unwrap();

        reject(&pool, id).await.unwrap();

        let row = find_by_actor(&pool, "https://relay.example/actor")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, RelayState::Rejected);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn unsubscribe_deletes_relay() {
        let pool = test_pool().await;

        let id = subscribe(&pool, "https://relay.example/actor", "https://relay.example/inbox", None, "f1")
            .await
            .unwrap();

        unsubscribe(&pool, id).await.unwrap();

        let row = find_by_actor(&pool, "https://relay.example/actor")
            .await
            .unwrap();
        assert!(row.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_accepted_filters_by_state() {
        let pool = test_pool().await;

        let id1 = subscribe(&pool, "https://relay1.example/actor", "https://relay1.example/inbox", None, "f1")
            .await
            .unwrap();
        let _id2 = subscribe(&pool, "https://relay2.example/actor", "https://relay2.example/inbox", None, "f2")
            .await
            .unwrap();
        let id3 = subscribe(&pool, "https://relay3.example/actor", "https://relay3.example/inbox", None, "f3")
            .await
            .unwrap();

        accept(&pool, id1).await.unwrap();
        reject(&pool, id3).await.unwrap();
        // id2 stays pending

        let accepted = get_accepted(&pool).await.unwrap();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].actor_uri, "https://relay1.example/actor");
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn find_by_actor_returns_none_for_missing() {
        let pool = test_pool().await;

        let row = find_by_actor(&pool, "https://nonexistent.example/actor")
            .await
            .unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn relay_state_display_roundtrip() {
        for state in [RelayState::Pending, RelayState::Accepted, RelayState::Rejected] {
            let s = state.to_string();
            let parsed: RelayState = s.parse().unwrap();
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn relay_state_from_str_rejects_unknown() {
        let result = "unknown".parse::<RelayState>();
        assert!(result.is_err());
    }

    #[cfg(feature = "activities")]
    #[test]
    fn relay_state_serde_roundtrip() {
        for state in [RelayState::Pending, RelayState::Accepted, RelayState::Rejected] {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: RelayState = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, state);
        }
    }
}
