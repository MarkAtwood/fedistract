//! Database persistence for cached remote actor accounts.
//!
//! Pure CRUD operations against the canonical `remote_accounts` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.

use crate::db::Pool;

/// A row in the `remote_accounts` table.
#[derive(Debug, Clone)]
pub struct RemoteAccountRow {
    pub id: i64,
    pub actor_uri: String,
    pub username: String,
    pub domain: String,
    pub display_name: String,
    pub bio_html: String,
    pub avatar_url: Option<String>,
    pub header_url: Option<String>,
    pub public_key_pem: String,
    pub public_key_id: String,
    pub inbox_url: String,
    pub shared_inbox_url: Option<String>,
    pub followers_url: Option<String>,
    pub is_locked: bool,
    pub bot: bool,
    pub last_fetched_at: i64,
    pub fetched_failed_at: Option<i64>,
    pub fetch_fail_count: i32,
}

/// The SELECT column list used by all queries.  18 columns exceeds sqlx's
/// tuple `FromRow` limit (16), so we use `query().fetch_*()` + `Row::get`
/// instead of `query_as`.
const COLUMNS: &str = "\
    id, actor_uri, username, domain, display_name, bio_html, \
    avatar_url, header_url, public_key_pem, public_key_id, \
    inbox_url, shared_inbox_url, followers_url, \
    is_locked, bot, last_fetched_at, fetched_failed_at, fetch_fail_count";

/// Extract a [`RemoteAccountRow`] from a SQLite row by column index.
#[cfg(feature = "db-sqlite")]
fn sqlite_row_to_account(r: sqlx::sqlite::SqliteRow) -> RemoteAccountRow {
    use sqlx::Row;
    RemoteAccountRow {
        id: r.get(0),
        actor_uri: r.get(1),
        username: r.get(2),
        domain: r.get(3),
        display_name: r.get(4),
        bio_html: r.get(5),
        avatar_url: r.get(6),
        header_url: r.get(7),
        public_key_pem: r.get(8),
        public_key_id: r.get(9),
        inbox_url: r.get(10),
        shared_inbox_url: r.get(11),
        followers_url: r.get(12),
        is_locked: r.get(13),
        bot: r.get(14),
        last_fetched_at: r.get(15),
        fetched_failed_at: r.get(16),
        fetch_fail_count: r.get(17),
    }
}

/// Extract a [`RemoteAccountRow`] from a Postgres row by column index.
#[cfg(feature = "db-postgres")]
fn pg_row_to_account(r: sqlx::postgres::PgRow) -> RemoteAccountRow {
    use sqlx::Row;
    RemoteAccountRow {
        id: r.get(0),
        actor_uri: r.get(1),
        username: r.get(2),
        domain: r.get(3),
        display_name: r.get(4),
        bio_html: r.get(5),
        avatar_url: r.get(6),
        header_url: r.get(7),
        public_key_pem: r.get(8),
        public_key_id: r.get(9),
        inbox_url: r.get(10),
        shared_inbox_url: r.get(11),
        followers_url: r.get(12),
        is_locked: r.get(13),
        bot: r.get(14),
        last_fetched_at: r.get(15),
        fetched_failed_at: r.get(16),
        fetch_fail_count: r.get(17),
    }
}

/// Insert or update a remote account by `actor_uri`. Returns the row id.
///
/// On conflict, all mutable fields are updated and the fetch-failure counters
/// are reset (a successful upsert implies a successful fetch).
pub async fn upsert_remote_account(
    pool: &Pool,
    account: &RemoteAccountRow,
) -> Result<i64, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row: (i64,) = sqlx::query_as(
                "INSERT INTO remote_accounts (\
                    id, actor_uri, username, domain, display_name, bio_html, \
                    avatar_url, header_url, public_key_pem, public_key_id, \
                    inbox_url, shared_inbox_url, followers_url, \
                    is_locked, bot, last_fetched_at, fetched_failed_at, fetch_fail_count\
                 ) VALUES (\
                    $1, $2, $3, $4, $5, $6, \
                    $7, $8, $9, $10, \
                    $11, $12, $13, \
                    $14, $15, $16, NULL, 0\
                 ) \
                 ON CONFLICT(actor_uri) DO UPDATE SET \
                    username         = EXCLUDED.username, \
                    domain           = EXCLUDED.domain, \
                    display_name     = EXCLUDED.display_name, \
                    bio_html         = EXCLUDED.bio_html, \
                    avatar_url       = EXCLUDED.avatar_url, \
                    header_url       = EXCLUDED.header_url, \
                    public_key_pem   = EXCLUDED.public_key_pem, \
                    public_key_id    = EXCLUDED.public_key_id, \
                    inbox_url        = EXCLUDED.inbox_url, \
                    shared_inbox_url = EXCLUDED.shared_inbox_url, \
                    followers_url    = EXCLUDED.followers_url, \
                    is_locked        = EXCLUDED.is_locked, \
                    bot              = EXCLUDED.bot, \
                    last_fetched_at  = EXCLUDED.last_fetched_at, \
                    fetched_failed_at = NULL, \
                    fetch_fail_count  = 0 \
                 RETURNING id",
            )
            .bind(account.id)
            .bind(&account.actor_uri)
            .bind(&account.username)
            .bind(&account.domain)
            .bind(&account.display_name)
            .bind(&account.bio_html)
            .bind(&account.avatar_url)
            .bind(&account.header_url)
            .bind(&account.public_key_pem)
            .bind(&account.public_key_id)
            .bind(&account.inbox_url)
            .bind(&account.shared_inbox_url)
            .bind(&account.followers_url)
            .bind(account.is_locked)
            .bind(account.bot)
            .bind(account.last_fetched_at)
            .fetch_one(pg)
            .await?;
            Ok(row.0)
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row: (i64,) = sqlx::query_as(
                "INSERT INTO remote_accounts (\
                    id, actor_uri, username, domain, display_name, bio_html, \
                    avatar_url, header_url, public_key_pem, public_key_id, \
                    inbox_url, shared_inbox_url, followers_url, \
                    is_locked, bot, last_fetched_at, fetched_failed_at, fetch_fail_count\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, ?5, ?6, \
                    ?7, ?8, ?9, ?10, \
                    ?11, ?12, ?13, \
                    ?14, ?15, ?16, NULL, 0\
                 ) \
                 ON CONFLICT(actor_uri) DO UPDATE SET \
                    username         = excluded.username, \
                    domain           = excluded.domain, \
                    display_name     = excluded.display_name, \
                    bio_html         = excluded.bio_html, \
                    avatar_url       = excluded.avatar_url, \
                    header_url       = excluded.header_url, \
                    public_key_pem   = excluded.public_key_pem, \
                    public_key_id    = excluded.public_key_id, \
                    inbox_url        = excluded.inbox_url, \
                    shared_inbox_url = excluded.shared_inbox_url, \
                    followers_url    = excluded.followers_url, \
                    is_locked        = excluded.is_locked, \
                    bot              = excluded.bot, \
                    last_fetched_at  = excluded.last_fetched_at, \
                    fetched_failed_at = NULL, \
                    fetch_fail_count  = 0 \
                 RETURNING id",
            )
            .bind(account.id)
            .bind(&account.actor_uri)
            .bind(&account.username)
            .bind(&account.domain)
            .bind(&account.display_name)
            .bind(&account.bio_html)
            .bind(&account.avatar_url)
            .bind(&account.header_url)
            .bind(&account.public_key_pem)
            .bind(&account.public_key_id)
            .bind(&account.inbox_url)
            .bind(&account.shared_inbox_url)
            .bind(&account.followers_url)
            .bind(account.is_locked)
            .bind(account.bot)
            .bind(account.last_fetched_at)
            .fetch_one(sq)
            .await?;
            Ok(row.0)
        }
    }
}

/// Look up a remote account by its ActivityPub actor URI.
pub async fn get_by_actor_uri(
    pool: &Pool,
    uri: &str,
) -> Result<Option<RemoteAccountRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE actor_uri = $1"),
            )
            .bind(uri)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_account))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE actor_uri = ?1"),
            )
            .bind(uri)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_account))
        }
    }
}

/// Look up a remote account by its HTTP Signature key ID (`public_key_id`).
///
/// Used during inbox signature verification to find the signing actor.
pub async fn get_by_key_id(
    pool: &Pool,
    key_id: &str,
) -> Result<Option<RemoteAccountRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE public_key_id = $1"),
            )
            .bind(key_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_account))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE public_key_id = ?1"),
            )
            .bind(key_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_account))
        }
    }
}

/// Look up a remote account by WebFinger-style `username@domain`.
pub async fn get_by_webfinger(
    pool: &Pool,
    username: &str,
    domain: &str,
) -> Result<Option<RemoteAccountRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!(
                    "SELECT {COLUMNS} FROM remote_accounts \
                     WHERE username = $1 AND domain = $2"
                ),
            )
            .bind(username)
            .bind(domain)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_account))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!(
                    "SELECT {COLUMNS} FROM remote_accounts \
                     WHERE username = ?1 AND domain = ?2"
                ),
            )
            .bind(username)
            .bind(domain)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_account))
        }
    }
}

/// Record a failed fetch attempt: increment `fetch_fail_count` and set
/// `fetched_failed_at` to `now`.
pub async fn mark_fetch_failure(
    pool: &Pool,
    id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE remote_accounts \
                 SET fetch_fail_count = fetch_fail_count + 1, \
                     fetched_failed_at = $1 \
                 WHERE id = $2",
            )
            .bind(now)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE remote_accounts \
                 SET fetch_fail_count = fetch_fail_count + 1, \
                     fetched_failed_at = ?1 \
                 WHERE id = ?2",
            )
            .bind(now)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Record a successful fetch: reset `fetch_fail_count` to 0, clear
/// `fetched_failed_at`, and update `last_fetched_at` to `now`.
pub async fn mark_fetch_success(
    pool: &Pool,
    id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE remote_accounts \
                 SET last_fetched_at = $1, \
                     fetched_failed_at = NULL, \
                     fetch_fail_count = 0 \
                 WHERE id = $2",
            )
            .bind(now)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE remote_accounts \
                 SET last_fetched_at = ?1, \
                     fetched_failed_at = NULL, \
                     fetch_fail_count = 0 \
                 WHERE id = ?2",
            )
            .bind(now)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Look up a remote account by its primary key ID.
pub async fn get_by_id(
    pool: &Pool,
    id: i64,
) -> Result<Option<RemoteAccountRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_account))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {COLUMNS} FROM remote_accounts WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_account))
        }
    }
}

/// Get the inbox URL for a remote account by actor_uri.
///
/// Prefers `shared_inbox_url` when available.
pub async fn get_inbox(
    pool: &Pool,
    actor_uri: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COALESCE(shared_inbox_url, inbox_url) \
                 FROM remote_accounts WHERE actor_uri = $1",
            )
            .bind(actor_uri)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COALESCE(shared_inbox_url, inbox_url) \
                 FROM remote_accounts WHERE actor_uri = ?1",
            )
            .bind(actor_uri)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.map(|(url,)| url))
}

/// Cascade-delete a remote account and all related data.
///
/// Removes rows from: notifications, remote_posts, followers, follows,
/// and finally the remote_accounts row itself.
pub async fn cascade_delete(
    pool: &Pool,
    remote_account_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM notifications WHERE from_remote_account_id = $1")
                .bind(remote_account_id).execute(pg).await?;
            sqlx::query("DELETE FROM remote_posts WHERE remote_account_id = $1")
                .bind(remote_account_id).execute(pg).await?;
            sqlx::query("DELETE FROM followers WHERE remote_account_id = $1")
                .bind(remote_account_id).execute(pg).await?;
            sqlx::query("DELETE FROM follows WHERE followee_remote_id = $1")
                .bind(remote_account_id).execute(pg).await?;
            sqlx::query("DELETE FROM remote_accounts WHERE id = $1")
                .bind(remote_account_id).execute(pg).await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM notifications WHERE from_remote_account_id = ?1")
                .bind(remote_account_id).execute(sq).await?;
            sqlx::query("DELETE FROM remote_posts WHERE remote_account_id = ?1")
                .bind(remote_account_id).execute(sq).await?;
            sqlx::query("DELETE FROM followers WHERE remote_account_id = ?1")
                .bind(remote_account_id).execute(sq).await?;
            sqlx::query("DELETE FROM follows WHERE followee_remote_id = ?1")
                .bind(remote_account_id).execute(sq).await?;
            sqlx::query("DELETE FROM remote_accounts WHERE id = ?1")
                .bind(remote_account_id).execute(sq).await?;
        }
    }
    Ok(())
}

/// Pure function: returns `true` if the account's `last_fetched_at` is older
/// than `max_age_secs` relative to `now`.
pub fn needs_refetch(account: &RemoteAccountRow, now: i64, max_age_secs: i64) -> bool {
    now - account.last_fetched_at >= max_age_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "db-sqlite")]
    async fn setup_db() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq);
        crate::db::migrate(&pool).await.unwrap();
        pool
    }

    fn sample_account() -> RemoteAccountRow {
        RemoteAccountRow {
            id: crate::id::generate_id(),
            actor_uri: "https://remote.example/users/alice".into(),
            username: "alice".into(),
            domain: "remote.example".into(),
            display_name: "Alice".into(),
            bio_html: "<p>hello</p>".into(),
            avatar_url: Some("https://remote.example/avatar.png".into()),
            header_url: None,
            public_key_pem: "-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----".into(),
            public_key_id: "https://remote.example/users/alice#main-key".into(),
            inbox_url: "https://remote.example/users/alice/inbox".into(),
            shared_inbox_url: Some("https://remote.example/inbox".into()),
            followers_url: Some("https://remote.example/users/alice/followers".into()),
            is_locked: false,
            bot: false,
            last_fetched_at: 1_700_000_000,
            fetched_failed_at: None,
            fetch_fail_count: 0,
        }
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn upsert_and_get_round_trip() {
        let pool = setup_db().await;
        let acct = sample_account();

        let id = upsert_remote_account(&pool, &acct).await.unwrap();
        assert_eq!(id, acct.id);

        let fetched = get_by_actor_uri(&pool, &acct.actor_uri)
            .await
            .unwrap()
            .expect("row should exist");
        assert_eq!(fetched.id, acct.id);
        assert_eq!(fetched.actor_uri, acct.actor_uri);
        assert_eq!(fetched.username, "alice");
        assert_eq!(fetched.domain, "remote.example");
        assert_eq!(fetched.display_name, "Alice");
        assert_eq!(fetched.bio_html, "<p>hello</p>");
        assert_eq!(fetched.avatar_url.as_deref(), Some("https://remote.example/avatar.png"));
        assert!(fetched.header_url.is_none());
        assert_eq!(fetched.public_key_id, "https://remote.example/users/alice#main-key");
        assert_eq!(fetched.inbox_url, "https://remote.example/users/alice/inbox");
        assert_eq!(fetched.shared_inbox_url.as_deref(), Some("https://remote.example/inbox"));
        assert!(!fetched.is_locked);
        assert!(!fetched.bot);
        assert_eq!(fetched.fetch_fail_count, 0);
        assert!(fetched.fetched_failed_at.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn upsert_updates_existing_row() {
        let pool = setup_db().await;
        let mut acct = sample_account();

        let id1 = upsert_remote_account(&pool, &acct).await.unwrap();

        // Update display name and re-upsert with a different id candidate.
        // The RETURNING id should give back the original row's id.
        acct.id = crate::id::generate_id();
        acct.display_name = "Alice Updated".into();
        acct.last_fetched_at = 1_700_001_000;

        let id2 = upsert_remote_account(&pool, &acct).await.unwrap();
        assert_eq!(id1, id2);

        let fetched = get_by_actor_uri(&pool, &acct.actor_uri)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.display_name, "Alice Updated");
        assert_eq!(fetched.last_fetched_at, 1_700_001_000);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_actor_uri_returns_none() {
        let pool = setup_db().await;
        let result = get_by_actor_uri(&pool, "https://nobody.example/users/nope")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_key_id_finds_account() {
        let pool = setup_db().await;
        let acct = sample_account();
        upsert_remote_account(&pool, &acct).await.unwrap();

        let fetched = get_by_key_id(&pool, &acct.public_key_id)
            .await
            .unwrap()
            .expect("should find by key id");
        assert_eq!(fetched.actor_uri, acct.actor_uri);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_key_id_returns_none() {
        let pool = setup_db().await;
        let result = get_by_key_id(&pool, "https://nobody.example/key#nope")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_webfinger_finds_account() {
        let pool = setup_db().await;
        let acct = sample_account();
        upsert_remote_account(&pool, &acct).await.unwrap();

        let fetched = get_by_webfinger(&pool, "alice", "remote.example")
            .await
            .unwrap()
            .expect("should find by webfinger");
        assert_eq!(fetched.actor_uri, acct.actor_uri);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_webfinger_returns_none() {
        let pool = setup_db().await;
        let result = get_by_webfinger(&pool, "nobody", "nowhere.example")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn fetch_failure_tracking() {
        let pool = setup_db().await;
        let acct = sample_account();
        let id = upsert_remote_account(&pool, &acct).await.unwrap();

        // First failure
        mark_fetch_failure(&pool, id, 1_700_001_000).await.unwrap();
        let fetched = get_by_actor_uri(&pool, &acct.actor_uri)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.fetch_fail_count, 1);
        assert_eq!(fetched.fetched_failed_at, Some(1_700_001_000));

        // Second failure
        mark_fetch_failure(&pool, id, 1_700_002_000).await.unwrap();
        let fetched = get_by_actor_uri(&pool, &acct.actor_uri)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.fetch_fail_count, 2);
        assert_eq!(fetched.fetched_failed_at, Some(1_700_002_000));

        // Success resets counters
        mark_fetch_success(&pool, id, 1_700_003_000).await.unwrap();
        let fetched = get_by_actor_uri(&pool, &acct.actor_uri)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.fetch_fail_count, 0);
        assert!(fetched.fetched_failed_at.is_none());
        assert_eq!(fetched.last_fetched_at, 1_700_003_000);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_id_finds_account() {
        let pool = setup_db().await;
        let acct = sample_account();
        let id = upsert_remote_account(&pool, &acct).await.unwrap();

        let fetched = get_by_id(&pool, id).await.unwrap().expect("should find by id");
        assert_eq!(fetched.actor_uri, acct.actor_uri);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_by_id_returns_none() {
        let pool = setup_db().await;
        assert!(get_by_id(&pool, 999).await.unwrap().is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_inbox_returns_shared() {
        let pool = setup_db().await;
        let acct = sample_account();
        upsert_remote_account(&pool, &acct).await.unwrap();

        let inbox = get_inbox(&pool, &acct.actor_uri).await.unwrap().unwrap();
        assert_eq!(inbox, "https://remote.example/inbox");
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn get_inbox_returns_none_for_unknown() {
        let pool = setup_db().await;
        assert!(get_inbox(&pool, "https://nobody/actor").await.unwrap().is_none());
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn cascade_delete_removes_account() {
        let pool = setup_db().await;
        let acct = sample_account();
        let id = upsert_remote_account(&pool, &acct).await.unwrap();

        cascade_delete(&pool, id).await.unwrap();

        assert!(get_by_id(&pool, id).await.unwrap().is_none());
    }

    #[test]
    fn needs_refetch_logic() {
        let mut acct = sample_account();
        acct.last_fetched_at = 1_000;

        // Exactly at boundary: needs refetch
        assert!(needs_refetch(&acct, 1_600, 600));
        // Past boundary
        assert!(needs_refetch(&acct, 2_000, 600));
        // Before boundary
        assert!(!needs_refetch(&acct, 1_599, 600));
        // Just fetched
        assert!(!needs_refetch(&acct, 1_000, 600));
    }
}
