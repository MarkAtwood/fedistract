//! Canonical database schema and pool abstraction for fediverse servers.
//!
//! Provides the shared schema used by all consumer projects. Simpler projects
//! (broadside, smallhold) just leave unused tables empty.
//!
//! # Migration system
//!
//! On startup, call [`migrate`] (or [`migrate_with`] for project-specific extras).
//! The migrator:
//!
//! 1. Creates the `schema_version` table if missing
//! 2. For fresh databases: runs the full CREATE TABLE schema (version 0)
//! 3. For existing databases: detects current version and runs pending migrations
//! 4. Each migration runs in a transaction
//!
//! # Features
//!
//! - **`postgres`** — PostgreSQL support via sqlx.
//! - **`sqlite`** — SQLite support via sqlx.

/// Re-export sqlx for use by server db_extras modules.
/// Server business logic should NOT use this directly.
pub use sqlx;

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("fieldwork-db requires at least one of `postgres` or `sqlite` features");

/// Database pool abstraction. Servers should use [`connect_sqlite`] or
/// [`connect_postgres`] to create a Pool — never construct the enum directly.
#[derive(Clone)]
pub enum Pool {
    #[cfg(feature = "postgres")]
    #[doc(hidden)]
    Postgres(sqlx::PgPool),
    #[cfg(feature = "sqlite")]
    #[doc(hidden)]
    Sqlite(sqlx::SqlitePool),
}

/// Connect to a SQLite database. Creates the file if it doesn't exist.
/// Sets WAL journal mode and normal synchronous for performance.
#[cfg(feature = "sqlite")]
pub async fn connect_sqlite(path: &str) -> Result<Pool, DbError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
    use std::str::FromStr;

    let options = SqliteConnectOptions::from_str(path)
        .map_err(|e| DbError::Connect(e.to_string()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| DbError::Connect(e.to_string()))?;

    Ok(Pool::Sqlite(pool))
}

/// Connect to a SQLite database with custom max connections.
#[cfg(feature = "sqlite")]
pub async fn connect_sqlite_with(path: &str, max_connections: u32) -> Result<Pool, DbError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
    use std::str::FromStr;

    let options = SqliteConnectOptions::from_str(path)
        .map_err(|e| DbError::Connect(e.to_string()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(|e| DbError::Connect(e.to_string()))?;

    Ok(Pool::Sqlite(pool))
}

/// Connect to a PostgreSQL database.
#[cfg(feature = "postgres")]
pub async fn connect_postgres(url: &str) -> Result<Pool, DbError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(url)
        .await
        .map_err(|e| DbError::Connect(e.to_string()))?;

    Ok(Pool::Postgres(pool))
}

/// Connect to a PostgreSQL database with custom max connections.
#[cfg(feature = "postgres")]
pub async fn connect_postgres_with(url: &str, max_connections: u32) -> Result<Pool, DbError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .map_err(|e| DbError::Connect(e.to_string()))?;

    Ok(Pool::Postgres(pool))
}

/// Database error type that doesn't leak sqlx types.
/// Servers should use this instead of sqlx::Error directly.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Query(String),
    #[error("connection error: {0}")]
    Connect(String),
    #[error("not found")]
    NotFound,
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Query(e.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("database error: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("migration {version} ({description}) failed: {source}")]
    MigrationFailed {
        version: u32,
        description: &'static str,
        source: sqlx::Error,
    },
    #[error("schema version {0} is newer than this binary supports (max {1})")]
    VersionTooNew(u32, u32),
}

/// A single versioned schema migration.
pub struct Migration {
    pub version: u32,
    pub description: &'static str,
    pub sql_sqlite: &'static str,
    pub sql_postgres: &'static str,
}

/// The current schema version after all built-in migrations.
/// Version 0 = fresh CREATE TABLE schema.
/// Version 1+ = incremental migrations.
const CURRENT_VERSION: u32 = 11;

/// Built-in migration chain. Version 0 is the fresh-install schema (handled
/// separately). These run in order for version 1, 2, 3, etc.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "add did:scid columns to users, personas, remote_accounts, followers",
        sql_sqlite: r#"
ALTER TABLE users ADD COLUMN did_scid TEXT;
ALTER TABLE personas ADD COLUMN did_scid TEXT;
ALTER TABLE personas ADD COLUMN did_scid_chain TEXT;
ALTER TABLE remote_accounts ADD COLUMN did_scid TEXT;
ALTER TABLE remote_accounts ADD COLUMN did TEXT;
ALTER TABLE followers ADD COLUMN did TEXT;
CREATE INDEX IF NOT EXISTS idx_remote_accounts_did ON remote_accounts(did) WHERE did IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_followers_did ON followers(did) WHERE did IS NOT NULL;
"#,
        sql_postgres: r#"
ALTER TABLE users ADD COLUMN IF NOT EXISTS did_scid TEXT;
ALTER TABLE personas ADD COLUMN IF NOT EXISTS did_scid TEXT;
ALTER TABLE personas ADD COLUMN IF NOT EXISTS did_scid_chain TEXT;
ALTER TABLE remote_accounts ADD COLUMN IF NOT EXISTS did_scid TEXT;
ALTER TABLE remote_accounts ADD COLUMN IF NOT EXISTS did TEXT;
ALTER TABLE followers ADD COLUMN IF NOT EXISTS did TEXT;
CREATE INDEX IF NOT EXISTS idx_remote_accounts_did ON remote_accounts(did) WHERE did IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_followers_did ON followers(did) WHERE did IS NOT NULL;
"#,
    },
    Migration {
        version: 2,
        description: "add integrity column to media table for content-addressed verification",
        sql_sqlite: "__ADD_COLUMN_IF_NOT_EXISTS:media:integrity:TEXT",
        sql_postgres: "ALTER TABLE media ADD COLUMN IF NOT EXISTS integrity TEXT;",
    },
    Migration {
        version: 3,
        description: "fix follows uniqueness: dedup rows and add partial unique indexes",
        sql_sqlite: r#"
DELETE FROM follows WHERE rowid NOT IN (
    SELECT MIN(rowid) FROM follows GROUP BY persona_id, followee_remote_id, followee_persona_id
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_remote_unique
    ON follows(persona_id, followee_remote_id) WHERE followee_remote_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_local_unique
    ON follows(persona_id, followee_persona_id) WHERE followee_persona_id IS NOT NULL;
"#,
        sql_postgres: r#"
DELETE FROM follows a USING follows b
    WHERE a.ctid < b.ctid
    AND a.persona_id = b.persona_id
    AND a.followee_remote_id IS NOT DISTINCT FROM b.followee_remote_id
    AND a.followee_persona_id IS NOT DISTINCT FROM b.followee_persona_id;
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_remote_unique
    ON follows(persona_id, followee_remote_id) WHERE followee_remote_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_local_unique
    ON follows(persona_id, followee_persona_id) WHERE followee_persona_id IS NOT NULL;
"#,
    },
    Migration {
        version: 4,
        description: "convert users.id and personas.id from TEXT to INTEGER (snowflake i64)",
        sql_sqlite: r#"
-- Disable FK checks during table rebuild
PRAGMA foreign_keys = OFF;

-- Step 1: Rebuild users with INTEGER id
CREATE TABLE users_new (
    id                INTEGER PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,
    oidc_subject      TEXT,
    oidc_issuer       TEXT,
    display_name      TEXT NOT NULL DEFAULT '',
    role              TEXT NOT NULL DEFAULT 'user',
    created_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    suspended_reason  TEXT,
    did_key           TEXT,
    did_plc           TEXT,
    recovery_pubkey   TEXT,
    legal_hold        INTEGER NOT NULL DEFAULT 0,
    quota_media_bytes INTEGER,
    quota_posts_per_day INTEGER,
    quota_followers   INTEGER,
    quota_following   INTEGER,
    did_scid          TEXT,
    UNIQUE (oidc_issuer, oidc_subject)
);
INSERT INTO users_new SELECT
    CASE WHEN typeof(id) = 'integer' OR CAST(id AS INTEGER) = id THEN CAST(id AS INTEGER) ELSE abs(random()) % 9000000000000 + 1000000000000 END,
    email, password_hash, oidc_subject, oidc_issuer,
    display_name, role, created_at, suspended_at, suspended_reason,
    did_key, did_plc, recovery_pubkey, legal_hold,
    quota_media_bytes, quota_posts_per_day, quota_followers, quota_following,
    did_scid
FROM users;

-- Build a mapping table for old TEXT ids -> new INTEGER ids
CREATE TEMP TABLE user_id_map AS
    SELECT u_old.id AS old_id, u_new.id AS new_id
    FROM users u_old
    JOIN users_new u_new ON u_old.email = u_new.email;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- Step 2: Rebuild personas with INTEGER id and user_id
CREATE TABLE personas_new (
    id              INTEGER PRIMARY KEY,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    bio_html        TEXT NOT NULL DEFAULT '',
    private_key_pem TEXT NOT NULL,
    public_key_pem  TEXT NOT NULL,
    avatar_media_id INTEGER,
    header_media_id INTEGER,
    is_locked       INTEGER NOT NULL DEFAULT 0,
    discoverable    INTEGER NOT NULL DEFAULT 1,
    bot             INTEGER NOT NULL DEFAULT 0,
    did_web         TEXT,
    fields_json     TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    last_status_at  INTEGER,
    did_scid        TEXT,
    did_scid_chain  TEXT
);
INSERT INTO personas_new SELECT
    CASE WHEN typeof(p.id) = 'integer' OR CAST(p.id AS INTEGER) = p.id THEN CAST(p.id AS INTEGER) ELSE abs(random()) % 9000000000000 + 1000000000000 END,
    COALESCE(um.new_id, p.user_id),
    p.username, p.display_name, p.bio, p.bio_html,
    p.private_key_pem, p.public_key_pem, p.avatar_media_id, p.header_media_id,
    p.is_locked, p.discoverable, p.bot, p.did_web, p.fields_json,
    p.created_at, p.last_status_at, p.did_scid, p.did_scid_chain
FROM personas p
LEFT JOIN user_id_map um ON p.user_id = um.old_id;

CREATE TEMP TABLE persona_id_map AS
    SELECT p_old.id AS old_id, p_new.id AS new_id
    FROM personas p_old
    JOIN personas_new p_new ON p_old.username = p_new.username;

DROP TABLE personas;
ALTER TABLE personas_new RENAME TO personas;
CREATE INDEX IF NOT EXISTS idx_personas_user ON personas(user_id);

-- Step 3: Update all FK references in dependent tables
-- posts
UPDATE posts SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = posts.user_id), posts.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = posts.persona_id), posts.persona_id);

-- media
UPDATE media SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = media.user_id), media.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = media.persona_id), media.persona_id);

-- followers
UPDATE followers SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = followers.persona_id), followers.persona_id),
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = followers.user_id), followers.user_id);

-- follow_requests
UPDATE follow_requests SET
    target_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = follow_requests.target_persona_id), follow_requests.target_persona_id);

-- follows
UPDATE follows SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = follows.persona_id), follows.persona_id),
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = follows.user_id), follows.user_id),
    followee_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = follows.followee_persona_id), follows.followee_persona_id);

-- favourites
UPDATE favourites SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = favourites.user_id), favourites.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = favourites.persona_id), favourites.persona_id);

-- bookmarks
UPDATE bookmarks SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = bookmarks.user_id), bookmarks.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = bookmarks.persona_id), bookmarks.persona_id);

-- pinned_posts
UPDATE pinned_posts SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = pinned_posts.persona_id), pinned_posts.persona_id);

-- notifications
UPDATE notifications SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = notifications.user_id), notifications.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = notifications.persona_id), notifications.persona_id),
    from_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = notifications.from_persona_id), notifications.from_persona_id);

-- mentions
UPDATE mentions SET
    mentioned_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = mentions.mentioned_persona_id), mentions.mentioned_persona_id);

-- oauth_tokens
UPDATE oauth_tokens SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = oauth_tokens.user_id), oauth_tokens.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = oauth_tokens.persona_id), oauth_tokens.persona_id);

-- oauth_authz_codes
UPDATE oauth_authz_codes SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = oauth_authz_codes.user_id), oauth_authz_codes.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = oauth_authz_codes.persona_id), oauth_authz_codes.persona_id);

-- delivery_queue
UPDATE delivery_queue SET
    sender_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = delivery_queue.sender_persona_id), delivery_queue.sender_persona_id);

-- relays
UPDATE relays SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = relays.persona_id), relays.persona_id);

-- blocks
UPDATE blocks SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = blocks.user_id), blocks.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = blocks.persona_id), blocks.persona_id),
    target_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = blocks.target_persona_id), blocks.target_persona_id);

-- mutes
UPDATE mutes SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = mutes.user_id), mutes.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = mutes.persona_id), mutes.persona_id),
    target_persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = mutes.target_persona_id), mutes.target_persona_id);

-- idempotency_keys
UPDATE idempotency_keys SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = idempotency_keys.user_id), idempotency_keys.user_id);

-- push_subscriptions
UPDATE push_subscriptions SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = push_subscriptions.user_id), push_subscriptions.user_id);

-- invite_codes
UPDATE invite_codes SET
    created_by = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = invite_codes.created_by), invite_codes.created_by),
    used_by = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = invite_codes.used_by), invite_codes.used_by);

-- lists
UPDATE lists SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = lists.user_id), lists.user_id);

-- list_accounts
UPDATE list_accounts SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = list_accounts.persona_id), list_accounts.persona_id);

-- followed_tags
UPDATE followed_tags SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = followed_tags.user_id), followed_tags.user_id);

-- filters
UPDATE filters SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = filters.user_id), filters.user_id);

-- scheduled_statuses
UPDATE scheduled_statuses SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = scheduled_statuses.user_id), scheduled_statuses.user_id),
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = scheduled_statuses.persona_id), scheduled_statuses.persona_id);

-- markers
UPDATE markers SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = markers.user_id), markers.user_id);

-- webauthn_credentials
UPDATE webauthn_credentials SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = webauthn_credentials.user_id), webauthn_credentials.user_id);

-- also_known_as
UPDATE also_known_as SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = also_known_as.persona_id), also_known_as.persona_id);

-- conversation_read_markers
UPDATE conversation_read_markers SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = conversation_read_markers.user_id), conversation_read_markers.user_id);

-- conversation_hidden
UPDATE conversation_hidden SET
    user_id = COALESCE((SELECT new_id FROM user_id_map WHERE old_id = conversation_hidden.user_id), conversation_hidden.user_id);

-- feed_state
UPDATE feed_state SET
    persona_id = COALESCE((SELECT new_id FROM persona_id_map WHERE old_id = feed_state.persona_id), feed_state.persona_id);

-- Clean up temp tables
DROP TABLE IF EXISTS user_id_map;
DROP TABLE IF EXISTS persona_id_map;

PRAGMA foreign_keys = ON;
"#,
        sql_postgres: r#"
ALTER TABLE users ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE personas ALTER COLUMN id TYPE BIGINT USING id::bigint;
ALTER TABLE personas ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE posts ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE posts ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE media ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE media ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE followers ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE followers ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE follow_requests ALTER COLUMN target_persona_id TYPE BIGINT USING target_persona_id::bigint;
ALTER TABLE follows ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE follows ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE follows ALTER COLUMN followee_persona_id TYPE BIGINT USING followee_persona_id::bigint;
ALTER TABLE favourites ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE favourites ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE bookmarks ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE bookmarks ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE pinned_posts ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE notifications ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE notifications ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE notifications ALTER COLUMN from_persona_id TYPE BIGINT USING from_persona_id::bigint;
ALTER TABLE mentions ALTER COLUMN mentioned_persona_id TYPE BIGINT USING mentioned_persona_id::bigint;
ALTER TABLE oauth_tokens ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE oauth_tokens ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE oauth_authz_codes ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE oauth_authz_codes ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE delivery_queue ALTER COLUMN sender_persona_id TYPE BIGINT USING sender_persona_id::bigint;
ALTER TABLE relays ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE blocks ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE blocks ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE blocks ALTER COLUMN target_persona_id TYPE BIGINT USING target_persona_id::bigint;
ALTER TABLE mutes ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE mutes ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE mutes ALTER COLUMN target_persona_id TYPE BIGINT USING target_persona_id::bigint;
ALTER TABLE idempotency_keys ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE push_subscriptions ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE invite_codes ALTER COLUMN created_by TYPE BIGINT USING created_by::bigint;
ALTER TABLE invite_codes ALTER COLUMN used_by TYPE BIGINT USING used_by::bigint;
ALTER TABLE lists ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE list_accounts ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE followed_tags ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE filters ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE scheduled_statuses ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE scheduled_statuses ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE markers ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE webauthn_credentials ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE also_known_as ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
ALTER TABLE conversation_read_markers ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE conversation_hidden ALTER COLUMN user_id TYPE BIGINT USING user_id::bigint;
ALTER TABLE feed_state ALTER COLUMN persona_id TYPE BIGINT USING persona_id::bigint;
"#,
    },
    Migration {
        version: 5,
        description: "add albums and album_items tables for Pixelfed collections",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS albums (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    cover_media_id INTEGER REFERENCES media(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS album_items (
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    media_id INTEGER NOT NULL REFERENCES media(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (album_id, media_id)
);
CREATE INDEX IF NOT EXISTS idx_album_items_album ON album_items(album_id, position);
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS albums (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    cover_media_id BIGINT REFERENCES media(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS album_items (
    album_id BIGINT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    media_id BIGINT NOT NULL REFERENCES media(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (album_id, media_id)
);
CREATE INDEX IF NOT EXISTS idx_album_items_album ON album_items(album_id, position);
"#,
    },
    Migration {
        version: 6,
        description: "add Lemmy-style communities, posts, comments, votes, bans",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS communities (
    persona_id INTEGER PRIMARY KEY REFERENCES personas(id),
    title TEXT NOT NULL,
    sidebar_html TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    icon_media_id INTEGER REFERENCES media(id),
    banner_media_id INTEGER REFERENCES media(id),
    nsfw INTEGER NOT NULL DEFAULT 0,
    posting_restricted INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS community_members (
    community_id INTEGER NOT NULL REFERENCES communities(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    role TEXT NOT NULL DEFAULT 'member',
    created_at INTEGER NOT NULL,
    UNIQUE (community_id, persona_id)
);
CREATE INDEX IF NOT EXISTS idx_community_members_community ON community_members(community_id);

CREATE TABLE IF NOT EXISTS community_posts (
    id INTEGER PRIMARY KEY,
    community_id INTEGER NOT NULL REFERENCES communities(persona_id),
    author_persona_id INTEGER NOT NULL REFERENCES personas(id),
    author_user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    body TEXT,
    body_html TEXT,
    url TEXT,
    ap_id TEXT NOT NULL UNIQUE,
    locked INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_community_posts_community ON community_posts(community_id, created_at DESC);

CREATE TABLE IF NOT EXISTS community_comments (
    id INTEGER PRIMARY KEY,
    post_id INTEGER NOT NULL REFERENCES community_posts(id),
    parent_comment_id INTEGER REFERENCES community_comments(id),
    author_persona_id INTEGER NOT NULL REFERENCES personas(id),
    author_user_id INTEGER NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_community_comments_post ON community_comments(post_id);
CREATE INDEX IF NOT EXISTS idx_community_comments_parent ON community_comments(parent_comment_id);

CREATE TABLE IF NOT EXISTS votes (
    user_id INTEGER NOT NULL REFERENCES users(id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    post_id INTEGER REFERENCES community_posts(id),
    comment_id INTEGER REFERENCES community_comments(id),
    score INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (comment_id IS NOT NULL)),
    CHECK (score IN (-1, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_votes_post ON votes(persona_id, post_id) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_votes_comment ON votes(persona_id, comment_id) WHERE comment_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS community_bans (
    community_id INTEGER NOT NULL REFERENCES communities(persona_id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    reason TEXT NOT NULL DEFAULT '',
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (community_id, persona_id)
);
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS communities (
    persona_id BIGINT PRIMARY KEY REFERENCES personas(id),
    title TEXT NOT NULL,
    sidebar_html TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    icon_media_id BIGINT REFERENCES media(id),
    banner_media_id BIGINT REFERENCES media(id),
    nsfw INTEGER NOT NULL DEFAULT 0,
    posting_restricted INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS community_members (
    community_id BIGINT NOT NULL REFERENCES communities(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    role TEXT NOT NULL DEFAULT 'member',
    created_at INTEGER NOT NULL,
    UNIQUE (community_id, persona_id)
);
CREATE INDEX IF NOT EXISTS idx_community_members_community ON community_members(community_id);

CREATE TABLE IF NOT EXISTS community_posts (
    id BIGINT PRIMARY KEY,
    community_id BIGINT NOT NULL REFERENCES communities(persona_id),
    author_persona_id BIGINT NOT NULL REFERENCES personas(id),
    author_user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    body TEXT,
    body_html TEXT,
    url TEXT,
    ap_id TEXT NOT NULL UNIQUE,
    locked INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_community_posts_community ON community_posts(community_id, created_at DESC);

CREATE TABLE IF NOT EXISTS community_comments (
    id BIGINT PRIMARY KEY,
    post_id BIGINT NOT NULL REFERENCES community_posts(id),
    parent_comment_id BIGINT REFERENCES community_comments(id),
    author_persona_id BIGINT NOT NULL REFERENCES personas(id),
    author_user_id BIGINT NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_community_comments_post ON community_comments(post_id);
CREATE INDEX IF NOT EXISTS idx_community_comments_parent ON community_comments(parent_comment_id);

CREATE TABLE IF NOT EXISTS votes (
    user_id BIGINT NOT NULL REFERENCES users(id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    post_id BIGINT REFERENCES community_posts(id),
    comment_id BIGINT REFERENCES community_comments(id),
    score INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (comment_id IS NOT NULL)),
    CHECK (score IN (-1, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_votes_post ON votes(persona_id, post_id) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_votes_comment ON votes(persona_id, comment_id) WHERE comment_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS community_bans (
    community_id BIGINT NOT NULL REFERENCES communities(persona_id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    reason TEXT NOT NULL DEFAULT '',
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (community_id, persona_id)
);
"#,
    },
    Migration {
        version: 7,
        description: "add PeerTube-style video hosting tables",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS video_channels (
    persona_id INTEGER PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    banner_media_id INTEGER REFERENCES media(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS videos (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES video_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    duration INTEGER,
    original_file_path TEXT,
    original_file_size INTEGER,
    original_mime_type TEXT,
    thumbnail_path TEXT,
    preview_path TEXT,
    visibility TEXT NOT NULL DEFAULT 'public',
    nsfw INTEGER NOT NULL DEFAULT 0,
    language TEXT,
    licence TEXT,
    category TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    views INTEGER NOT NULL DEFAULT 0,
    likes INTEGER NOT NULL DEFAULT 0,
    dislikes INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    published_at INTEGER,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_videos_channel ON videos(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_videos_state ON videos(state);

CREATE TABLE IF NOT EXISTS transcoding_jobs (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER NOT NULL DEFAULT 0,
    output_path TEXT,
    file_size INTEGER,
    error TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_state ON transcoding_jobs(state);

CREATE TABLE IF NOT EXISTS video_files (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    fps INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (video_id, resolution)
);

CREATE TABLE IF NOT EXISTS video_comments (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    parent_comment_id INTEGER REFERENCES video_comments(id),
    author_persona_id INTEGER NOT NULL REFERENCES personas(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_comments_video ON video_comments(video_id);

CREATE TABLE IF NOT EXISTS video_playlists (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS video_playlist_items (
    playlist_id INTEGER NOT NULL REFERENCES video_playlists(id) ON DELETE CASCADE,
    video_id INTEGER NOT NULL REFERENCES videos(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, video_id)
);
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS video_channels (
    persona_id BIGINT PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    banner_media_id BIGINT REFERENCES media(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS videos (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES video_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    duration INTEGER,
    original_file_path TEXT,
    original_file_size BIGINT,
    original_mime_type TEXT,
    thumbnail_path TEXT,
    preview_path TEXT,
    visibility TEXT NOT NULL DEFAULT 'public',
    nsfw BOOLEAN NOT NULL DEFAULT false,
    language TEXT,
    licence TEXT,
    category TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    views INTEGER NOT NULL DEFAULT 0,
    likes INTEGER NOT NULL DEFAULT 0,
    dislikes INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    published_at INTEGER,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_videos_channel ON videos(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_videos_state ON videos(state);

CREATE TABLE IF NOT EXISTS transcoding_jobs (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER NOT NULL DEFAULT 0,
    output_path TEXT,
    file_size BIGINT,
    error TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_state ON transcoding_jobs(state);

CREATE TABLE IF NOT EXISTS video_files (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    fps INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (video_id, resolution)
);

CREATE TABLE IF NOT EXISTS video_comments (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    parent_comment_id BIGINT REFERENCES video_comments(id),
    author_persona_id BIGINT NOT NULL REFERENCES personas(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed BOOLEAN NOT NULL DEFAULT false,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_comments_video ON video_comments(video_id);

CREATE TABLE IF NOT EXISTS video_playlists (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS video_playlist_items (
    playlist_id BIGINT NOT NULL REFERENCES video_playlists(id) ON DELETE CASCADE,
    video_id BIGINT NOT NULL REFERENCES videos(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, video_id)
);
"#,
    },
    Migration {
        version: 8,
        description: "add Funkwhale-style audio hosting and podcast tables",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS audio_channels (
    persona_id INTEGER PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'music',
    language TEXT,
    rss_enabled INTEGER NOT NULL DEFAULT 0,
    itunes_category TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES audio_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    track_number INTEGER,
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    cover_media_id INTEGER REFERENCES media(id),
    description TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tracks_channel ON tracks(channel_id, created_at DESC);

CREATE TABLE IF NOT EXISTS audio_playlists (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_playlist_items (
    playlist_id INTEGER NOT NULL REFERENCES audio_playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_audio_playlist_items ON audio_playlist_items(playlist_id, position);

CREATE TABLE IF NOT EXISTS podcast_series (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES audio_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    language TEXT,
    category TEXT,
    itunes_category TEXT,
    cover_media_id INTEGER REFERENCES media(id),
    explicit INTEGER NOT NULL DEFAULT 0,
    rss_link TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id INTEGER PRIMARY KEY,
    series_id INTEGER NOT NULL REFERENCES podcast_series(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    episode_number INTEGER,
    season_number INTEGER,
    explicit INTEGER NOT NULL DEFAULT 0,
    ap_id TEXT NOT NULL UNIQUE,
    published_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_series ON podcast_episodes(series_id, published_at DESC);
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS audio_channels (
    persona_id BIGINT PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'music',
    language TEXT,
    rss_enabled BOOLEAN NOT NULL DEFAULT false,
    itunes_category TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES audio_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    track_number INTEGER,
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    cover_media_id BIGINT REFERENCES media(id),
    description TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tracks_channel ON tracks(channel_id, created_at DESC);

CREATE TABLE IF NOT EXISTS audio_playlists (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_playlist_items (
    playlist_id BIGINT NOT NULL REFERENCES audio_playlists(id) ON DELETE CASCADE,
    track_id BIGINT NOT NULL REFERENCES tracks(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_audio_playlist_items ON audio_playlist_items(playlist_id, position);

CREATE TABLE IF NOT EXISTS podcast_series (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES audio_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    language TEXT,
    category TEXT,
    itunes_category TEXT,
    cover_media_id BIGINT REFERENCES media(id),
    explicit BOOLEAN NOT NULL DEFAULT false,
    rss_link TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id BIGINT PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES podcast_series(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    episode_number INTEGER,
    season_number INTEGER,
    explicit BOOLEAN NOT NULL DEFAULT false,
    ap_id TEXT NOT NULL UNIQUE,
    published_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_series ON podcast_episodes(series_id, published_at DESC);
"#,
    },
    Migration {
        version: 9,
        description: "add Misskey-style emoji reactions table",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS reactions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    post_id INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    emoji TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_unique
    ON reactions(persona_id, post_id, emoji) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_remote_unique
    ON reactions(persona_id, remote_post_id, emoji) WHERE remote_post_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id) WHERE post_id IS NOT NULL;
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS reactions (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    post_id BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    emoji TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_unique
    ON reactions(persona_id, post_id, emoji) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_remote_unique
    ON reactions(persona_id, remote_post_id, emoji) WHERE remote_post_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id) WHERE post_id IS NOT NULL;
"#,
    },
    Migration {
        version: 10,
        description: "add Bookwyrm-style books, editions, shelves, and reviews tables",
        sql_sqlite: r#"
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    isbn TEXT,
    isbn13 TEXT,
    openlibrary_id TEXT,
    cover_url TEXT,
    description TEXT NOT NULL DEFAULT '',
    pages INTEGER,
    published_year INTEGER,
    language TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_books_isbn ON books(isbn) WHERE isbn IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_books_title ON books(title);

CREATE TABLE IF NOT EXISTS book_editions (
    id INTEGER PRIMARY KEY,
    book_id INTEGER NOT NULL REFERENCES books(id),
    title TEXT NOT NULL,
    isbn TEXT,
    isbn13 TEXT,
    publisher TEXT,
    published_year INTEGER,
    pages INTEGER,
    cover_url TEXT,
    format TEXT,
    language TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS reading_shelves (
    user_id INTEGER NOT NULL REFERENCES users(id),
    book_id INTEGER NOT NULL REFERENCES books(id),
    status TEXT NOT NULL,
    started_at INTEGER,
    finished_at INTEGER,
    rating INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER,
    UNIQUE (user_id, book_id),
    CHECK (status IN ('to-read', 'reading', 'read')),
    CHECK (rating IS NULL OR (rating >= 1 AND rating <= 5))
);
CREATE INDEX IF NOT EXISTS idx_shelves_user ON reading_shelves(user_id, status);

CREATE TABLE IF NOT EXISTS book_reviews (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    book_id INTEGER NOT NULL REFERENCES books(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    rating INTEGER,
    spoiler INTEGER NOT NULL DEFAULT 0,
    ap_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    CHECK (rating IS NULL OR (rating >= 1 AND rating <= 5))
);
CREATE INDEX IF NOT EXISTS idx_reviews_book ON book_reviews(book_id, created_at DESC);
"#,
        sql_postgres: r#"
CREATE TABLE IF NOT EXISTS books (
    id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    isbn TEXT,
    isbn13 TEXT,
    openlibrary_id TEXT,
    cover_url TEXT,
    description TEXT NOT NULL DEFAULT '',
    pages INTEGER,
    published_year INTEGER,
    language TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_books_isbn ON books(isbn) WHERE isbn IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_books_title ON books(title);

CREATE TABLE IF NOT EXISTS book_editions (
    id BIGINT PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    title TEXT NOT NULL,
    isbn TEXT,
    isbn13 TEXT,
    publisher TEXT,
    published_year INTEGER,
    pages INTEGER,
    cover_url TEXT,
    format TEXT,
    language TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS reading_shelves (
    user_id BIGINT NOT NULL REFERENCES users(id),
    book_id BIGINT NOT NULL REFERENCES books(id),
    status TEXT NOT NULL,
    started_at INTEGER,
    finished_at INTEGER,
    rating INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER,
    UNIQUE (user_id, book_id),
    CHECK (status IN ('to-read', 'reading', 'read')),
    CHECK (rating IS NULL OR (rating >= 1 AND rating <= 5))
);
CREATE INDEX IF NOT EXISTS idx_shelves_user ON reading_shelves(user_id, status);

CREATE TABLE IF NOT EXISTS book_reviews (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    book_id BIGINT NOT NULL REFERENCES books(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    rating INTEGER,
    spoiler BOOLEAN NOT NULL DEFAULT false,
    ap_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    CHECK (rating IS NULL OR (rating >= 1 AND rating <= 5))
);
CREATE INDEX IF NOT EXISTS idx_reviews_book ON book_reviews(book_id, created_at DESC);
"#,
    },
    Migration {
        version: 11,
        description: "add abstract column to posts table",
        sql_sqlite: "__ADD_COLUMN_IF_NOT_EXISTS:posts:abstract:TEXT",
        sql_postgres: "ALTER TABLE posts ADD COLUMN IF NOT EXISTS abstract TEXT;",
    },
];

/// SQL to transform a legacy (pre-versioned) database to canonical schema.
/// Each project provides its own upgrade SQL. The framework runs it when
/// it detects existing tables but no schema version.
pub struct LegacyUpgrade {
    pub from: &'static str,
    pub sql_sqlite: &'static str,
    pub sql_postgres: &'static str,
}

/// Run schema migrations. Call at startup.
///
/// For fresh databases, creates the full schema (version 0) then runs all
/// incremental migrations. For existing databases, runs only pending migrations.
pub async fn migrate(pool: &Pool) -> Result<(), MigrateError> {
    migrate_full(pool, None, &[]).await
}

/// Run schema migrations with a legacy upgrade and/or project-specific extras.
///
/// If `legacy_upgrade` is provided and the database has tables but no version,
/// the legacy SQL runs first to transform the schema to canonical before the
/// base schema and incremental migrations proceed.
///
/// Extra migrations are interleaved by version number with the built-in chain.
pub async fn migrate_with(
    pool: &Pool,
    extras: &[Migration],
) -> Result<(), MigrateError> {
    migrate_full(pool, None, extras).await
}

/// Full migration with optional legacy upgrade and extras.
pub async fn migrate_full(
    pool: &Pool,
    legacy_upgrade: Option<&LegacyUpgrade>,
    extras: &[Migration],
) -> Result<(), MigrateError> {
    ensure_version_table(pool).await?;

    let current = get_version(pool).await?;

    if current > max_version(extras) {
        return Err(MigrateError::VersionTooNew(current, max_version(extras)));
    }

    if current == 0 {
        let has_tables = has_any_tables(pool).await?;

        if has_tables {
            // Existing legacy database — transform to canonical
            if let Some(upgrade) = legacy_upgrade {
                run_legacy_upgrade(pool, upgrade).await?;
                tracing::info!(from = upgrade.from, "applied legacy schema upgrade");
            }
            // Run base schema with IF NOT EXISTS to fill any gaps
            run_base_schema(pool).await?;
            set_version(pool, 0).await?;
        } else {
            // Fresh database — create everything from scratch
            run_base_schema(pool).await?;
            set_version(pool, 0).await?;
        }
    }

    // Run pending migrations in version order
    let mut all_migrations: Vec<&Migration> = MIGRATIONS.iter().chain(extras.iter()).collect();
    all_migrations.sort_by_key(|m| m.version);

    let start_version = get_version(pool).await?;
    for migration in &all_migrations {
        if migration.version <= start_version {
            continue;
        }
        run_migration(pool, migration).await?;
        set_version(pool, migration.version).await?;
        tracing::info!(
            version = migration.version,
            description = migration.description,
            "applied migration"
        );
    }

    Ok(())
}

/// Get current schema version. Returns 0 if no version has been set.
pub async fn schema_version(pool: &Pool) -> Result<u32, MigrateError> {
    ensure_version_table(pool).await?;
    get_version(pool).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Internal migration machinery
// ---------------------------------------------------------------------------

async fn ensure_version_table(pool: &Pool) -> Result<(), sqlx::Error> {
    let sql = "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)";
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => { sqlx::raw_sql(sql).execute(pg).await?; }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => { sqlx::raw_sql(sql).execute(sq).await?; }
    }
    Ok(())
}

async fn get_version(pool: &Pool) -> Result<u32, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT version FROM schema_version LIMIT 1")
                .fetch_optional(pg)
                .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT version FROM schema_version LIMIT 1")
                .fetch_optional(sq)
                .await?
        }
    };
    Ok(row.map(|r| r.0 as u32).unwrap_or(0))
}

async fn set_version(pool: &Pool, version: u32) -> Result<(), sqlx::Error> {
    // Upsert: delete all rows then insert. schema_version has at most one row.
    let delete = "DELETE FROM schema_version";
    let insert = format!("INSERT INTO schema_version (version) VALUES ({version})");
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(delete).execute(pg).await?;
            sqlx::raw_sql(&insert).execute(pg).await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::raw_sql(delete).execute(sq).await?;
            sqlx::raw_sql(&insert).execute(sq).await?;
        }
    }
    Ok(())
}

async fn has_any_tables(pool: &Pool) -> Result<bool, sqlx::Error> {
    let count: (i32,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::int FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name != 'schema_version'"
            ).fetch_one(pg).await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name != 'schema_version'"
            ).fetch_one(sq).await?
        }
    };
    Ok(count.0 > 0)
}

/// Detect which legacy schema is present (for unversioned existing databases).
/// Returns a string identifying the source project.
#[allow(dead_code)]
pub async fn detect_legacy_schema(pool: &Pool) -> Result<Option<&'static str>, sqlx::Error> {
    let has_users = table_exists(pool, "users").await?;
    let has_personas = table_exists(pool, "personas").await?;
    let has_accounts = table_exists(pool, "accounts").await?;

    Ok(match (has_users, has_personas, has_accounts) {
        (true, true, _) => Some("canonical"),  // already canonical (gaja or migrated)
        (false, true, _) => Some("broadside"), // has personas but no users table
        (false, false, true) => Some("smallhold"), // has accounts, not yet split
        _ => None, // empty or unknown
    })
}

async fn table_exists(pool: &Pool, table: &str) -> Result<bool, sqlx::Error> {
    let count: (i32,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::int FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name = $1"
            ).bind(table).fetch_one(pg).await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1"
            ).bind(table).fetch_one(sq).await?
        }
    };
    Ok(count.0 > 0)
}

async fn run_legacy_upgrade(pool: &Pool, upgrade: &LegacyUpgrade) -> Result<(), MigrateError> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(upgrade.sql_postgres).execute(pg).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: 0, description: "legacy upgrade", source: e
                })?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::raw_sql(upgrade.sql_sqlite).execute(sq).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: 0, description: "legacy upgrade", source: e
                })?;
        }
    }
    Ok(())
}

async fn run_base_schema(pool: &Pool) -> Result<(), MigrateError> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(SCHEMA_POSTGRES).execute(pg).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: 0, description: "base schema", source: e
                })?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::raw_sql(SCHEMA_SQLITE).execute(sq).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: 0, description: "base schema", source: e
                })?;
        }
    }
    Ok(())
}

async fn run_migration(pool: &Pool, migration: &Migration) -> Result<(), MigrateError> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(migration.sql_postgres).execute(pg).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: migration.version,
                    description: migration.description,
                    source: e,
                })?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            // SQLite lacks ALTER TABLE ADD COLUMN IF NOT EXISTS.
            // Handle our directive format: __ADD_COLUMN_IF_NOT_EXISTS:table:column:type
            if let Some(rest) = migration.sql_sqlite.strip_prefix("__ADD_COLUMN_IF_NOT_EXISTS:") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() == 3 {
                    sqlite_add_column_if_not_exists(sq, parts[0], parts[1], parts[2]).await
                        .map_err(|e| MigrateError::MigrationFailed {
                            version: migration.version,
                            description: migration.description,
                            source: e,
                        })?;
                }
            } else {
                sqlx::raw_sql(migration.sql_sqlite).execute(sq).await
                    .map_err(|e| MigrateError::MigrationFailed {
                        version: migration.version,
                        description: migration.description,
                        source: e,
                    })?;
            }
        }
    }
    Ok(())
}

/// Add a column to a SQLite table only if it doesn't already exist.
#[cfg(feature = "sqlite")]
async fn sqlite_add_column_if_not_exists(
    pool: &sqlx::SqlitePool,
    table: &str,
    column: &str,
    col_type: &str,
) -> Result<(), sqlx::Error> {
    let has_col: (i32,) = sqlx::query_as(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"),
    )
    .fetch_one(pool)
    .await?;

    if has_col.0 == 0 {
        sqlx::raw_sql(&format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}"))
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn max_version(extras: &[Migration]) -> u32 {
    let extras_max = extras.iter().map(|m| m.version).max().unwrap_or(0);
    CURRENT_VERSION.max(extras_max)
}

// ---------------------------------------------------------------------------
// PostgreSQL schema
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
const SCHEMA_POSTGRES: &str = r#"
-- Users: identity + auth. One user can own multiple personas.
CREATE TABLE IF NOT EXISTS users (
    id                BIGINT PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,
    oidc_subject      TEXT,
    oidc_issuer       TEXT,
    display_name      TEXT NOT NULL DEFAULT '',
    role              TEXT NOT NULL DEFAULT 'user',
    created_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    suspended_reason  TEXT,
    did_key           TEXT,
    did_plc           TEXT,
    recovery_pubkey   TEXT,
    legal_hold        BOOLEAN NOT NULL DEFAULT false,
    quota_media_bytes BIGINT,
    quota_posts_per_day INTEGER,
    quota_followers   INTEGER,
    quota_following   INTEGER,
    UNIQUE (oidc_issuer, oidc_subject)
);

-- Personas: public-facing ActivityPub actors.
CREATE TABLE IF NOT EXISTS personas (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    bio_html        TEXT NOT NULL DEFAULT '',
    private_key_pem TEXT NOT NULL,
    public_key_pem  TEXT NOT NULL,
    avatar_media_id BIGINT,
    header_media_id BIGINT,
    is_locked       BOOLEAN NOT NULL DEFAULT false,
    discoverable    BOOLEAN NOT NULL DEFAULT true,
    bot             BOOLEAN NOT NULL DEFAULT false,
    did_web         TEXT,
    fields_json     JSONB NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    last_status_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_personas_user ON personas(user_id);

-- Remote accounts: cached copies of actors on other servers.
CREATE TABLE IF NOT EXISTS remote_accounts (
    id                BIGINT PRIMARY KEY,
    actor_uri         TEXT NOT NULL UNIQUE,
    username          TEXT NOT NULL,
    domain            TEXT NOT NULL,
    display_name      TEXT NOT NULL,
    bio_html          TEXT NOT NULL DEFAULT '',
    avatar_url        TEXT,
    header_url        TEXT,
    public_key_pem    TEXT NOT NULL,
    public_key_id     TEXT NOT NULL UNIQUE,
    inbox_url         TEXT NOT NULL,
    shared_inbox_url  TEXT,
    followers_url     TEXT,
    is_locked         BOOLEAN NOT NULL DEFAULT false,
    bot               BOOLEAN NOT NULL DEFAULT false,
    last_fetched_at   INTEGER NOT NULL,
    fetched_failed_at INTEGER,
    fetch_fail_count  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_remote_accounts_webfinger ON remote_accounts(username, domain);

-- Posts: local statuses.
CREATE TABLE IF NOT EXISTS posts (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    persona_id      BIGINT NOT NULL REFERENCES personas(id),
    ap_id           TEXT NOT NULL UNIQUE,
    in_reply_to_id  BIGINT,
    in_reply_to_uri TEXT,
    boost_of_id     BIGINT,
    boost_of_uri    TEXT,
    content         TEXT NOT NULL,
    content_html    TEXT NOT NULL,
    spoiler_text    TEXT NOT NULL DEFAULT '',
    visibility      TEXT NOT NULL,
    sensitive       BOOLEAN NOT NULL DEFAULT false,
    language        TEXT,
    context_url     TEXT,
    created_at      INTEGER NOT NULL,
    edited_at       INTEGER,
    deleted_at      INTEGER,
    deleted_reason  TEXT,
    abstract        TEXT
);
CREATE INDEX IF NOT EXISTS idx_posts_user ON posts(user_id);
CREATE INDEX IF NOT EXISTS idx_posts_persona_created ON posts(persona_id, created_at DESC);

-- Remote posts: cached copies of statuses from other servers.
CREATE TABLE IF NOT EXISTS remote_posts (
    id                BIGINT PRIMARY KEY,
    ap_uri            TEXT NOT NULL UNIQUE,
    remote_account_id BIGINT NOT NULL REFERENCES remote_accounts(id),
    in_reply_to_uri   TEXT,
    content_html      TEXT NOT NULL,
    spoiler_text      TEXT NOT NULL DEFAULT '',
    visibility        TEXT NOT NULL,
    sensitive         BOOLEAN NOT NULL DEFAULT false,
    language          TEXT,
    context_url       TEXT,
    created_at        INTEGER NOT NULL,
    fetched_at        INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_remote_posts_account_created ON remote_posts(remote_account_id, created_at DESC);

-- Post edits: history of edits to posts.
CREATE TABLE IF NOT EXISTS post_edits (
    id            BIGINT PRIMARY KEY,
    post_id       BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    content       TEXT NOT NULL,
    content_html  TEXT NOT NULL,
    spoiler_text  TEXT NOT NULL DEFAULT '',
    sensitive     BOOLEAN NOT NULL DEFAULT false,
    created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_post_edits_post ON post_edits(post_id, created_at);

-- Media attachments.
CREATE TABLE IF NOT EXISTS media (
    id          BIGINT PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id),
    persona_id  BIGINT NOT NULL REFERENCES personas(id),
    post_id     BIGINT REFERENCES posts(id),
    file_path   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    file_size   BIGINT NOT NULL,
    width       INTEGER,
    height      INTEGER,
    blurhash    TEXT,
    integrity   TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_user ON media(user_id);

-- Inbound followers (remote actors following local personas).
CREATE TABLE IF NOT EXISTS followers (
    persona_id        BIGINT NOT NULL REFERENCES personas(id),
    user_id           BIGINT NOT NULL REFERENCES users(id),
    remote_account_id BIGINT NOT NULL REFERENCES remote_accounts(id),
    accepted_at       INTEGER NOT NULL,
    UNIQUE (persona_id, remote_account_id)
);
CREATE INDEX IF NOT EXISTS idx_followers_persona ON followers(persona_id);
CREATE INDEX IF NOT EXISTS idx_followers_user ON followers(user_id);

-- Follow requests (pending approval for locked accounts).
CREATE TABLE IF NOT EXISTS follow_requests (
    id                  BIGINT PRIMARY KEY,
    requester_remote_id BIGINT NOT NULL REFERENCES remote_accounts(id),
    target_persona_id   BIGINT NOT NULL REFERENCES personas(id),
    ap_id               TEXT NOT NULL UNIQUE,
    created_at          INTEGER NOT NULL
);

-- Outbound follows (local personas following others).
CREATE TABLE IF NOT EXISTS follows (
    persona_id          BIGINT NOT NULL REFERENCES personas(id),
    user_id             BIGINT NOT NULL REFERENCES users(id),
    followee_remote_id  BIGINT REFERENCES remote_accounts(id),
    followee_persona_id BIGINT REFERENCES personas(id),
    created_at          INTEGER NOT NULL,
    show_reblogs        BOOLEAN NOT NULL DEFAULT true,
    notify              BOOLEAN NOT NULL DEFAULT false,
    CHECK ((followee_remote_id IS NOT NULL) != (followee_persona_id IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_follows_persona ON follows(persona_id);
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows(user_id);
-- Partial unique indexes: Postgres/SQLite UNIQUE doesn't enforce uniqueness when columns are NULL
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_remote_unique
    ON follows(persona_id, followee_remote_id) WHERE followee_remote_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_local_unique
    ON follows(persona_id, followee_persona_id) WHERE followee_persona_id IS NOT NULL;

-- Favourites / likes.
CREATE TABLE IF NOT EXISTS favourites (
    user_id        BIGINT NOT NULL REFERENCES users(id),
    persona_id     BIGINT NOT NULL REFERENCES personas(id),
    post_id        BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL)),
    UNIQUE (persona_id, post_id, remote_post_id)
);

-- Misskey-style emoji reactions.
CREATE TABLE IF NOT EXISTS reactions (
    id             BIGINT PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users(id),
    persona_id     BIGINT NOT NULL REFERENCES personas(id),
    post_id        BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    emoji          TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_unique
    ON reactions(persona_id, post_id, emoji) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_remote_unique
    ON reactions(persona_id, remote_post_id, emoji) WHERE remote_post_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id) WHERE post_id IS NOT NULL;

-- Bookmarks.
CREATE TABLE IF NOT EXISTS bookmarks (
    user_id        BIGINT NOT NULL REFERENCES users(id),
    persona_id     BIGINT NOT NULL REFERENCES personas(id),
    post_id        BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    UNIQUE (persona_id, post_id, remote_post_id)
);

-- Pinned posts.
CREATE TABLE IF NOT EXISTS pinned_posts (
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    post_id    BIGINT NOT NULL REFERENCES posts(id),
    pinned_at  INTEGER NOT NULL,
    UNIQUE (persona_id, post_id)
);

-- Notifications.
CREATE TABLE IF NOT EXISTS notifications (
    id                     BIGINT PRIMARY KEY,
    user_id                BIGINT NOT NULL REFERENCES users(id),
    persona_id             BIGINT NOT NULL REFERENCES personas(id),
    kind                   TEXT NOT NULL,
    from_persona_id        BIGINT REFERENCES personas(id),
    from_remote_account_id BIGINT REFERENCES remote_accounts(id),
    post_id                BIGINT REFERENCES posts(id),
    remote_post_id         BIGINT REFERENCES remote_posts(id),
    created_at             INTEGER NOT NULL,
    read_at                INTEGER
);
CREATE INDEX IF NOT EXISTS idx_notifications_user_created ON notifications(user_id, created_at DESC);

-- Post tags (hashtags).
CREATE TABLE IF NOT EXISTS post_tags (
    post_id BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    UNIQUE (post_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag);

-- Mentions.
CREATE TABLE IF NOT EXISTS mentions (
    post_id              BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    mentioned_remote_id  BIGINT REFERENCES remote_accounts(id),
    mentioned_persona_id BIGINT REFERENCES personas(id)
);
CREATE INDEX IF NOT EXISTS idx_mentions_post ON mentions(post_id);

-- Link preview cards.
CREATE TABLE IF NOT EXISTS link_cards (
    id            BIGSERIAL PRIMARY KEY,
    url           TEXT NOT NULL UNIQUE,
    card_type     TEXT NOT NULL DEFAULT 'link',
    title         TEXT NOT NULL DEFAULT '',
    description   TEXT NOT NULL DEFAULT '',
    image_url     TEXT,
    author_name   TEXT NOT NULL DEFAULT '',
    author_url    TEXT NOT NULL DEFAULT '',
    provider_name TEXT NOT NULL DEFAULT '',
    provider_url  TEXT NOT NULL DEFAULT '',
    html          TEXT NOT NULL DEFAULT '',
    width         INTEGER NOT NULL DEFAULT 0,
    height        INTEGER NOT NULL DEFAULT 0,
    fetched_at    INTEGER NOT NULL,
    failed        BOOLEAN NOT NULL DEFAULT false
);

-- Junction: post <-> card.
CREATE TABLE IF NOT EXISTS post_cards (
    post_id  BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    card_url TEXT NOT NULL REFERENCES link_cards(url) ON DELETE CASCADE,
    UNIQUE (post_id, card_url)
);

-- OAuth applications.
CREATE TABLE IF NOT EXISTS oauth_apps (
    id            BIGINT PRIMARY KEY,
    client_id     TEXT NOT NULL UNIQUE,
    client_secret TEXT NOT NULL,
    name          TEXT NOT NULL,
    website       TEXT,
    redirect_uri  TEXT NOT NULL,
    scopes        TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

-- OAuth bearer tokens.
CREATE TABLE IF NOT EXISTS oauth_tokens (
    id           BIGINT PRIMARY KEY,
    token_hash   TEXT NOT NULL UNIQUE,
    app_id       BIGINT NOT NULL REFERENCES oauth_apps(id),
    user_id      BIGINT REFERENCES users(id),
    persona_id   BIGINT REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

-- OAuth authorization codes.
CREATE TABLE IF NOT EXISTS oauth_authz_codes (
    code_hash    TEXT PRIMARY KEY,
    app_id       BIGINT NOT NULL REFERENCES oauth_apps(id),
    user_id      BIGINT NOT NULL REFERENCES users(id),
    persona_id   BIGINT NOT NULL REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

-- Delivery queue for outbound ActivityPub activities.
CREATE TABLE IF NOT EXISTS delivery_queue (
    id                 BIGINT PRIMARY KEY,
    target_inbox       TEXT NOT NULL,
    sender_persona_id  BIGINT NOT NULL REFERENCES personas(id),
    activity_json      TEXT NOT NULL,
    attempts           INTEGER NOT NULL DEFAULT 0,
    next_attempt_at    INTEGER NOT NULL,
    last_error         TEXT,
    created_at         INTEGER NOT NULL,
    delivered_at       INTEGER,
    dead_at            INTEGER
);
CREATE INDEX IF NOT EXISTS idx_delivery_pending
    ON delivery_queue(next_attempt_at) WHERE delivered_at IS NULL AND dead_at IS NULL;

-- Relay subscriptions.
CREATE TABLE IF NOT EXISTS relays (
    id         BIGINT PRIMARY KEY,
    actor_uri  TEXT NOT NULL UNIQUE,
    inbox_url  TEXT NOT NULL,
    persona_id BIGINT REFERENCES personas(id),
    follow_id  TEXT NOT NULL DEFAULT '',
    state      TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL
);

-- Domain blocks.
CREATE TABLE IF NOT EXISTS domain_blocks (
    domain         TEXT PRIMARY KEY,
    severity       TEXT NOT NULL,
    reject_media   BOOLEAN NOT NULL DEFAULT false,
    reject_reports BOOLEAN NOT NULL DEFAULT false,
    reason         TEXT NOT NULL DEFAULT '',
    created_at     INTEGER NOT NULL
);

-- User blocks.
CREATE TABLE IF NOT EXISTS blocks (
    user_id           BIGINT NOT NULL REFERENCES users(id),
    persona_id        BIGINT NOT NULL REFERENCES personas(id),
    target_persona_id BIGINT REFERENCES personas(id),
    target_remote_id  BIGINT REFERENCES remote_accounts(id),
    created_at        INTEGER NOT NULL
);

-- User mutes.
CREATE TABLE IF NOT EXISTS mutes (
    user_id           BIGINT NOT NULL REFERENCES users(id),
    persona_id        BIGINT NOT NULL REFERENCES personas(id),
    target_persona_id BIGINT REFERENCES personas(id),
    target_remote_id  BIGINT REFERENCES remote_accounts(id),
    expires_at        INTEGER,
    created_at        INTEGER NOT NULL
);

-- Idempotency keys for Mastodon Client API.
CREATE TABLE IF NOT EXISTS idempotency_keys (
    key_hash   TEXT PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users(id),
    post_id    BIGINT NOT NULL REFERENCES posts(id),
    created_at INTEGER NOT NULL
);

-- Push notification subscriptions (Web Push).
CREATE TABLE IF NOT EXISTS push_subscriptions (
    id               BIGINT PRIMARY KEY,
    user_id          BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint         TEXT NOT NULL,
    key_p256dh       TEXT NOT NULL,
    key_auth         TEXT NOT NULL,
    alerts_mention   BOOLEAN NOT NULL DEFAULT true,
    alerts_favourite BOOLEAN NOT NULL DEFAULT true,
    alerts_reblog    BOOLEAN NOT NULL DEFAULT true,
    alerts_follow    BOOLEAN NOT NULL DEFAULT true,
    alerts_poll      BOOLEAN NOT NULL DEFAULT false,
    policy           TEXT NOT NULL DEFAULT 'all',
    created_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user ON push_subscriptions(user_id);

-- VAPID key pair for Web Push.
CREATE TABLE IF NOT EXISTS vapid_keys (
    id                INTEGER PRIMARY KEY,
    private_key_pem   TEXT NOT NULL,
    public_key_base64 TEXT NOT NULL
);

-- Invite codes.
CREATE TABLE IF NOT EXISTS invite_codes (
    code       TEXT PRIMARY KEY,
    created_by BIGINT REFERENCES users(id),
    used_by    BIGINT REFERENCES users(id),
    max_uses   INTEGER NOT NULL DEFAULT 1,
    uses       INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER,
    created_at INTEGER NOT NULL
);

-- Admin API tokens.
CREATE TABLE IF NOT EXISTS admin_tokens (
    id         BIGINT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER,
    revoked_at INTEGER
);

-- Lists (curated follow lists).
CREATE TABLE IF NOT EXISTS lists (
    id             BIGINT PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users(id),
    title          TEXT NOT NULL,
    replies_policy TEXT NOT NULL DEFAULT 'list',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_accounts (
    list_id      BIGINT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    persona_id   BIGINT,
    remote_id    BIGINT,
    UNIQUE (list_id, persona_id, remote_id)
);

-- Followed tags.
CREATE TABLE IF NOT EXISTS followed_tags (
    user_id    BIGINT NOT NULL REFERENCES users(id),
    tag        TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (user_id, tag)
);

-- Content filters.
CREATE TABLE IF NOT EXISTS filters (
    id            BIGINT PRIMARY KEY,
    user_id       BIGINT NOT NULL REFERENCES users(id),
    title         TEXT NOT NULL,
    context       TEXT NOT NULL DEFAULT '[]',
    filter_action TEXT NOT NULL DEFAULT 'warn',
    expires_at    INTEGER,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS filter_keywords (
    id         BIGINT PRIMARY KEY,
    filter_id  BIGINT NOT NULL REFERENCES filters(id) ON DELETE CASCADE,
    keyword    TEXT NOT NULL,
    whole_word BOOLEAN NOT NULL DEFAULT true
);

-- Scheduled statuses.
CREATE TABLE IF NOT EXISTS scheduled_statuses (
    id           BIGINT PRIMARY KEY,
    user_id      BIGINT NOT NULL REFERENCES users(id),
    persona_id   BIGINT NOT NULL REFERENCES personas(id),
    scheduled_at INTEGER NOT NULL,
    params_json  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scheduled_due ON scheduled_statuses(scheduled_at);

-- Timeline markers (Mastodon Client API).
CREATE TABLE IF NOT EXISTS markers (
    user_id      BIGINT NOT NULL REFERENCES users(id),
    timeline     TEXT NOT NULL,
    last_read_id TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL,
    UNIQUE (user_id, timeline)
);

-- WebAuthn credentials.
CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    credential_json TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);

-- WebAuthn challenges (ephemeral).
CREATE TABLE IF NOT EXISTS webauthn_challenges (
    challenge_id TEXT PRIMARY KEY,
    state_json   TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

-- Also-known-as URIs for account migration.
CREATE TABLE IF NOT EXISTS also_known_as (
    persona_id BIGINT NOT NULL REFERENCES personas(id),
    uri        TEXT NOT NULL,
    UNIQUE (persona_id, uri)
);

-- Conversation tracking.
CREATE TABLE IF NOT EXISTS conversation_read_markers (
    user_id BIGINT NOT NULL REFERENCES users(id),
    post_id BIGINT NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS conversation_hidden (
    user_id BIGINT NOT NULL REFERENCES users(id),
    post_id BIGINT NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

-- Feed state (RSS/Atom feed polling, used by broadside).
CREATE TABLE IF NOT EXISTS feed_state (
    feed_url     TEXT PRIMARY KEY,
    persona_id   BIGINT NOT NULL REFERENCES personas(id),
    last_seen_id TEXT,
    last_poll    INTEGER
);

-- Albums / collections (Pixelfed-compatible).
CREATE TABLE IF NOT EXISTS albums (
    id               BIGINT PRIMARY KEY,
    user_id          BIGINT NOT NULL REFERENCES users(id),
    persona_id       BIGINT NOT NULL REFERENCES personas(id),
    title            TEXT NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    visibility       TEXT NOT NULL DEFAULT 'public',
    cover_media_id   BIGINT REFERENCES media(id),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER
);

CREATE TABLE IF NOT EXISTS album_items (
    album_id BIGINT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    media_id BIGINT NOT NULL REFERENCES media(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (album_id, media_id)
);
CREATE INDEX IF NOT EXISTS idx_album_items_album ON album_items(album_id, position);

-- Video hosting (PeerTube-compatible).
CREATE TABLE IF NOT EXISTS video_channels (
    persona_id BIGINT PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    banner_media_id BIGINT REFERENCES media(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS videos (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES video_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    duration INTEGER,
    original_file_path TEXT,
    original_file_size BIGINT,
    original_mime_type TEXT,
    thumbnail_path TEXT,
    preview_path TEXT,
    visibility TEXT NOT NULL DEFAULT 'public',
    nsfw BOOLEAN NOT NULL DEFAULT false,
    language TEXT,
    licence TEXT,
    category TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    views INTEGER NOT NULL DEFAULT 0,
    likes INTEGER NOT NULL DEFAULT 0,
    dislikes INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    published_at INTEGER,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_videos_channel ON videos(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_videos_state ON videos(state);

CREATE TABLE IF NOT EXISTS transcoding_jobs (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER NOT NULL DEFAULT 0,
    output_path TEXT,
    file_size BIGINT,
    error TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_state ON transcoding_jobs(state);

CREATE TABLE IF NOT EXISTS video_files (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    fps INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (video_id, resolution)
);

CREATE TABLE IF NOT EXISTS video_comments (
    id BIGINT PRIMARY KEY,
    video_id BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    parent_comment_id BIGINT REFERENCES video_comments(id),
    author_persona_id BIGINT NOT NULL REFERENCES personas(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed BOOLEAN NOT NULL DEFAULT false,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_comments_video ON video_comments(video_id);

CREATE TABLE IF NOT EXISTS video_playlists (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS video_playlist_items (
    playlist_id BIGINT NOT NULL REFERENCES video_playlists(id) ON DELETE CASCADE,
    video_id BIGINT NOT NULL REFERENCES videos(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, video_id)
);

-- Funkwhale-style audio hosting
CREATE TABLE IF NOT EXISTS audio_channels (
    persona_id BIGINT PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'music',
    language TEXT,
    rss_enabled BOOLEAN NOT NULL DEFAULT false,
    itunes_category TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES audio_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    track_number INTEGER,
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    cover_media_id BIGINT REFERENCES media(id),
    description TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tracks_channel ON tracks(channel_id, created_at DESC);

CREATE TABLE IF NOT EXISTS audio_playlists (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_playlist_items (
    playlist_id BIGINT NOT NULL REFERENCES audio_playlists(id) ON DELETE CASCADE,
    track_id BIGINT NOT NULL REFERENCES tracks(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_audio_playlist_items ON audio_playlist_items(playlist_id, position);

CREATE TABLE IF NOT EXISTS podcast_series (
    id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES audio_channels(persona_id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    language TEXT,
    category TEXT,
    itunes_category TEXT,
    cover_media_id BIGINT REFERENCES media(id),
    explicit BOOLEAN NOT NULL DEFAULT false,
    rss_link TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id BIGINT PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES podcast_series(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    episode_number INTEGER,
    season_number INTEGER,
    explicit BOOLEAN NOT NULL DEFAULT false,
    ap_id TEXT NOT NULL UNIQUE,
    published_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_series ON podcast_episodes(series_id, published_at DESC);
"#;

// ---------------------------------------------------------------------------
// SQLite schema
// ---------------------------------------------------------------------------

#[cfg(feature = "sqlite")]
const SCHEMA_SQLITE: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id                INTEGER PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,
    oidc_subject      TEXT,
    oidc_issuer       TEXT,
    display_name      TEXT NOT NULL DEFAULT '',
    role              TEXT NOT NULL DEFAULT 'user',
    created_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    suspended_reason  TEXT,
    did_key           TEXT,
    did_plc           TEXT,
    recovery_pubkey   TEXT,
    legal_hold        INTEGER NOT NULL DEFAULT 0,
    quota_media_bytes INTEGER,
    quota_posts_per_day INTEGER,
    quota_followers   INTEGER,
    quota_following   INTEGER,
    UNIQUE (oidc_issuer, oidc_subject)
);

CREATE TABLE IF NOT EXISTS personas (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    bio_html        TEXT NOT NULL DEFAULT '',
    private_key_pem TEXT NOT NULL,
    public_key_pem  TEXT NOT NULL,
    avatar_media_id INTEGER,
    header_media_id INTEGER,
    is_locked       INTEGER NOT NULL DEFAULT 0,
    discoverable    INTEGER NOT NULL DEFAULT 1,
    bot             INTEGER NOT NULL DEFAULT 0,
    did_web         TEXT,
    fields_json     TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    last_status_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_personas_user ON personas(user_id);

CREATE TABLE IF NOT EXISTS remote_accounts (
    id                INTEGER PRIMARY KEY,
    actor_uri         TEXT NOT NULL UNIQUE,
    username          TEXT NOT NULL,
    domain            TEXT NOT NULL,
    display_name      TEXT NOT NULL,
    bio_html          TEXT NOT NULL DEFAULT '',
    avatar_url        TEXT,
    header_url        TEXT,
    public_key_pem    TEXT NOT NULL,
    public_key_id     TEXT NOT NULL UNIQUE,
    inbox_url         TEXT NOT NULL,
    shared_inbox_url  TEXT,
    followers_url     TEXT,
    is_locked         INTEGER NOT NULL DEFAULT 0,
    bot               INTEGER NOT NULL DEFAULT 0,
    last_fetched_at   INTEGER NOT NULL,
    fetched_failed_at INTEGER,
    fetch_fail_count  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_remote_accounts_webfinger ON remote_accounts(username, domain);

CREATE TABLE IF NOT EXISTS posts (
    id              INTEGER PRIMARY KEY,
    user_id         INTEGER NOT NULL REFERENCES users(id),
    persona_id      INTEGER NOT NULL REFERENCES personas(id),
    ap_id           TEXT NOT NULL UNIQUE,
    in_reply_to_id  INTEGER,
    in_reply_to_uri TEXT,
    boost_of_id     INTEGER,
    boost_of_uri    TEXT,
    content         TEXT NOT NULL,
    content_html    TEXT NOT NULL,
    spoiler_text    TEXT NOT NULL DEFAULT '',
    visibility      TEXT NOT NULL,
    sensitive       INTEGER NOT NULL DEFAULT 0,
    language        TEXT,
    context_url     TEXT,
    created_at      INTEGER NOT NULL,
    edited_at       INTEGER,
    deleted_at      INTEGER,
    deleted_reason  TEXT,
    abstract        TEXT
);
CREATE INDEX IF NOT EXISTS idx_posts_user ON posts(user_id);
CREATE INDEX IF NOT EXISTS idx_posts_persona_created ON posts(persona_id, created_at DESC);

CREATE TABLE IF NOT EXISTS remote_posts (
    id                INTEGER PRIMARY KEY,
    ap_uri            TEXT NOT NULL UNIQUE,
    remote_account_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    in_reply_to_uri   TEXT,
    content_html      TEXT NOT NULL,
    spoiler_text      TEXT NOT NULL DEFAULT '',
    visibility        TEXT NOT NULL,
    sensitive         INTEGER NOT NULL DEFAULT 0,
    language          TEXT,
    context_url       TEXT,
    created_at        INTEGER NOT NULL,
    fetched_at        INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_remote_posts_account_created ON remote_posts(remote_account_id, created_at DESC);

CREATE TABLE IF NOT EXISTS post_edits (
    id            INTEGER PRIMARY KEY,
    post_id       INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    content       TEXT NOT NULL,
    content_html  TEXT NOT NULL,
    spoiler_text  TEXT NOT NULL DEFAULT '',
    sensitive     INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_post_edits_post ON post_edits(post_id, created_at);

CREATE TABLE IF NOT EXISTS media (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    persona_id  INTEGER NOT NULL REFERENCES personas(id),
    post_id     INTEGER REFERENCES posts(id),
    file_path   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    file_size   INTEGER NOT NULL,
    width       INTEGER,
    height      INTEGER,
    blurhash    TEXT,
    integrity   TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_user ON media(user_id);

CREATE TABLE IF NOT EXISTS followers (
    persona_id        INTEGER NOT NULL REFERENCES personas(id),
    user_id           INTEGER NOT NULL REFERENCES users(id),
    remote_account_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    accepted_at       INTEGER NOT NULL,
    UNIQUE (persona_id, remote_account_id)
);
CREATE INDEX IF NOT EXISTS idx_followers_persona ON followers(persona_id);
CREATE INDEX IF NOT EXISTS idx_followers_user ON followers(user_id);

CREATE TABLE IF NOT EXISTS follow_requests (
    id                  INTEGER PRIMARY KEY,
    requester_remote_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    target_persona_id   INTEGER NOT NULL REFERENCES personas(id),
    ap_id               TEXT NOT NULL UNIQUE,
    created_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS follows (
    persona_id          INTEGER NOT NULL REFERENCES personas(id),
    user_id             INTEGER NOT NULL REFERENCES users(id),
    followee_remote_id  INTEGER REFERENCES remote_accounts(id),
    followee_persona_id INTEGER REFERENCES personas(id),
    created_at          INTEGER NOT NULL,
    show_reblogs        INTEGER NOT NULL DEFAULT 1,
    notify              INTEGER NOT NULL DEFAULT 0,
    CHECK ((followee_remote_id IS NOT NULL) != (followee_persona_id IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_follows_persona ON follows(persona_id);
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_remote_unique
    ON follows(persona_id, followee_remote_id) WHERE followee_remote_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_follows_local_unique
    ON follows(persona_id, followee_persona_id) WHERE followee_persona_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS favourites (
    user_id        INTEGER NOT NULL REFERENCES users(id),
    persona_id     INTEGER NOT NULL REFERENCES personas(id),
    post_id        INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL)),
    UNIQUE (persona_id, post_id, remote_post_id)
);

CREATE TABLE IF NOT EXISTS reactions (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users(id),
    persona_id     INTEGER NOT NULL REFERENCES personas(id),
    post_id        INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    emoji          TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_unique
    ON reactions(persona_id, post_id, emoji) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_remote_unique
    ON reactions(persona_id, remote_post_id, emoji) WHERE remote_post_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id) WHERE post_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS bookmarks (
    user_id        INTEGER NOT NULL REFERENCES users(id),
    persona_id     INTEGER NOT NULL REFERENCES personas(id),
    post_id        INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    UNIQUE (persona_id, post_id, remote_post_id)
);

CREATE TABLE IF NOT EXISTS pinned_posts (
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    post_id    INTEGER NOT NULL REFERENCES posts(id),
    pinned_at  INTEGER NOT NULL,
    UNIQUE (persona_id, post_id)
);

CREATE TABLE IF NOT EXISTS notifications (
    id                     INTEGER PRIMARY KEY,
    user_id                INTEGER NOT NULL REFERENCES users(id),
    persona_id             INTEGER NOT NULL REFERENCES personas(id),
    kind                   TEXT NOT NULL,
    from_persona_id        INTEGER REFERENCES personas(id),
    from_remote_account_id INTEGER REFERENCES remote_accounts(id),
    post_id                INTEGER REFERENCES posts(id),
    remote_post_id         INTEGER REFERENCES remote_posts(id),
    created_at             INTEGER NOT NULL,
    read_at                INTEGER
);
CREATE INDEX IF NOT EXISTS idx_notifications_user_created ON notifications(user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS post_tags (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    UNIQUE (post_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag);

CREATE TABLE IF NOT EXISTS mentions (
    post_id              INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    mentioned_remote_id  INTEGER REFERENCES remote_accounts(id),
    mentioned_persona_id INTEGER REFERENCES personas(id)
);
CREATE INDEX IF NOT EXISTS idx_mentions_post ON mentions(post_id);

CREATE TABLE IF NOT EXISTS link_cards (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    url           TEXT NOT NULL UNIQUE,
    card_type     TEXT NOT NULL DEFAULT 'link',
    title         TEXT NOT NULL DEFAULT '',
    description   TEXT NOT NULL DEFAULT '',
    image_url     TEXT,
    author_name   TEXT NOT NULL DEFAULT '',
    author_url    TEXT NOT NULL DEFAULT '',
    provider_name TEXT NOT NULL DEFAULT '',
    provider_url  TEXT NOT NULL DEFAULT '',
    html          TEXT NOT NULL DEFAULT '',
    width         INTEGER NOT NULL DEFAULT 0,
    height        INTEGER NOT NULL DEFAULT 0,
    fetched_at    INTEGER NOT NULL,
    failed        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS post_cards (
    post_id  INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    card_url TEXT NOT NULL REFERENCES link_cards(url) ON DELETE CASCADE,
    UNIQUE (post_id, card_url)
);

CREATE TABLE IF NOT EXISTS oauth_apps (
    id            INTEGER PRIMARY KEY,
    client_id     TEXT NOT NULL UNIQUE,
    client_secret TEXT NOT NULL,
    name          TEXT NOT NULL,
    website       TEXT,
    redirect_uri  TEXT NOT NULL,
    scopes        TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS oauth_tokens (
    id           INTEGER PRIMARY KEY,
    token_hash   TEXT NOT NULL UNIQUE,
    app_id       INTEGER NOT NULL REFERENCES oauth_apps(id),
    user_id      INTEGER REFERENCES users(id),
    persona_id   INTEGER REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

CREATE TABLE IF NOT EXISTS oauth_authz_codes (
    code_hash    TEXT PRIMARY KEY,
    app_id       INTEGER NOT NULL REFERENCES oauth_apps(id),
    user_id      INTEGER NOT NULL REFERENCES users(id),
    persona_id   INTEGER NOT NULL REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS delivery_queue (
    id                 INTEGER PRIMARY KEY,
    target_inbox       TEXT NOT NULL,
    sender_persona_id  INTEGER NOT NULL REFERENCES personas(id),
    activity_json      TEXT NOT NULL,
    attempts           INTEGER NOT NULL DEFAULT 0,
    next_attempt_at    INTEGER NOT NULL,
    last_error         TEXT,
    created_at         INTEGER NOT NULL,
    delivered_at       INTEGER,
    dead_at            INTEGER
);
CREATE INDEX IF NOT EXISTS idx_delivery_pending
    ON delivery_queue(next_attempt_at) WHERE delivered_at IS NULL AND dead_at IS NULL;

CREATE TABLE IF NOT EXISTS relays (
    id         INTEGER PRIMARY KEY,
    actor_uri  TEXT NOT NULL UNIQUE,
    inbox_url  TEXT NOT NULL,
    persona_id INTEGER REFERENCES personas(id),
    follow_id  TEXT NOT NULL DEFAULT '',
    state      TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS domain_blocks (
    domain         TEXT PRIMARY KEY,
    severity       TEXT NOT NULL,
    reject_media   INTEGER NOT NULL DEFAULT 0,
    reject_reports INTEGER NOT NULL DEFAULT 0,
    reason         TEXT NOT NULL DEFAULT '',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS blocks (
    user_id           INTEGER NOT NULL REFERENCES users(id),
    persona_id        INTEGER NOT NULL REFERENCES personas(id),
    target_persona_id INTEGER REFERENCES personas(id),
    target_remote_id  INTEGER REFERENCES remote_accounts(id),
    created_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mutes (
    user_id           INTEGER NOT NULL REFERENCES users(id),
    persona_id        INTEGER NOT NULL REFERENCES personas(id),
    target_persona_id INTEGER REFERENCES personas(id),
    target_remote_id  INTEGER REFERENCES remote_accounts(id),
    expires_at        INTEGER,
    created_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key_hash   TEXT PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    post_id    INTEGER NOT NULL REFERENCES posts(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id               INTEGER PRIMARY KEY,
    user_id          INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint         TEXT NOT NULL,
    key_p256dh       TEXT NOT NULL,
    key_auth         TEXT NOT NULL,
    alerts_mention   INTEGER NOT NULL DEFAULT 1,
    alerts_favourite INTEGER NOT NULL DEFAULT 1,
    alerts_reblog    INTEGER NOT NULL DEFAULT 1,
    alerts_follow    INTEGER NOT NULL DEFAULT 1,
    alerts_poll      INTEGER NOT NULL DEFAULT 0,
    policy           TEXT NOT NULL DEFAULT 'all',
    created_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user ON push_subscriptions(user_id);

CREATE TABLE IF NOT EXISTS vapid_keys (
    id                INTEGER PRIMARY KEY,
    private_key_pem   TEXT NOT NULL,
    public_key_base64 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invite_codes (
    code       TEXT PRIMARY KEY,
    created_by INTEGER REFERENCES users(id),
    used_by    INTEGER REFERENCES users(id),
    max_uses   INTEGER NOT NULL DEFAULT 1,
    uses       INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS admin_tokens (
    id         INTEGER PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER,
    revoked_at INTEGER
);

CREATE TABLE IF NOT EXISTS lists (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users(id),
    title          TEXT NOT NULL,
    replies_policy TEXT NOT NULL DEFAULT 'list',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_accounts (
    list_id    INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    persona_id INTEGER,
    remote_id  INTEGER,
    UNIQUE (list_id, persona_id, remote_id)
);

CREATE TABLE IF NOT EXISTS followed_tags (
    user_id    INTEGER NOT NULL REFERENCES users(id),
    tag        TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (user_id, tag)
);

CREATE TABLE IF NOT EXISTS filters (
    id            INTEGER PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id),
    title         TEXT NOT NULL,
    context       TEXT NOT NULL DEFAULT '[]',
    filter_action TEXT NOT NULL DEFAULT 'warn',
    expires_at    INTEGER,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS filter_keywords (
    id         INTEGER PRIMARY KEY,
    filter_id  INTEGER NOT NULL REFERENCES filters(id) ON DELETE CASCADE,
    keyword    TEXT NOT NULL,
    whole_word INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS scheduled_statuses (
    id           INTEGER PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES users(id),
    persona_id   INTEGER NOT NULL REFERENCES personas(id),
    scheduled_at INTEGER NOT NULL,
    params_json  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scheduled_due ON scheduled_statuses(scheduled_at);

CREATE TABLE IF NOT EXISTS markers (
    user_id      INTEGER NOT NULL REFERENCES users(id),
    timeline     TEXT NOT NULL,
    last_read_id TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL,
    UNIQUE (user_id, timeline)
);

CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id              INTEGER PRIMARY KEY,
    user_id         INTEGER NOT NULL REFERENCES users(id),
    credential_json TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS webauthn_challenges (
    challenge_id TEXT PRIMARY KEY,
    state_json   TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS also_known_as (
    persona_id INTEGER NOT NULL REFERENCES personas(id),
    uri        TEXT NOT NULL,
    UNIQUE (persona_id, uri)
);

CREATE TABLE IF NOT EXISTS conversation_read_markers (
    user_id INTEGER NOT NULL REFERENCES users(id),
    post_id INTEGER NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS conversation_hidden (
    user_id INTEGER NOT NULL REFERENCES users(id),
    post_id INTEGER NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS feed_state (
    feed_url     TEXT PRIMARY KEY,
    persona_id   INTEGER NOT NULL REFERENCES personas(id),
    last_seen_id TEXT,
    last_poll    INTEGER
);

-- Albums / collections (Pixelfed-compatible).
CREATE TABLE IF NOT EXISTS albums (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users(id),
    persona_id     INTEGER NOT NULL REFERENCES personas(id),
    title          TEXT NOT NULL,
    description    TEXT NOT NULL DEFAULT '',
    visibility     TEXT NOT NULL DEFAULT 'public',
    cover_media_id INTEGER REFERENCES media(id),
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER
);

CREATE TABLE IF NOT EXISTS album_items (
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    media_id INTEGER NOT NULL REFERENCES media(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (album_id, media_id)
);
CREATE INDEX IF NOT EXISTS idx_album_items_album ON album_items(album_id, position);

-- Video hosting (PeerTube-compatible).
CREATE TABLE IF NOT EXISTS video_channels (
    persona_id INTEGER PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    banner_media_id INTEGER REFERENCES media(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS videos (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES video_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    duration INTEGER,
    original_file_path TEXT,
    original_file_size INTEGER,
    original_mime_type TEXT,
    thumbnail_path TEXT,
    preview_path TEXT,
    visibility TEXT NOT NULL DEFAULT 'public',
    nsfw INTEGER NOT NULL DEFAULT 0,
    language TEXT,
    licence TEXT,
    category TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    views INTEGER NOT NULL DEFAULT 0,
    likes INTEGER NOT NULL DEFAULT 0,
    dislikes INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    published_at INTEGER,
    updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_videos_channel ON videos(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_videos_state ON videos(state);

CREATE TABLE IF NOT EXISTS transcoding_jobs (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER NOT NULL DEFAULT 0,
    output_path TEXT,
    file_size INTEGER,
    error TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_state ON transcoding_jobs(state);

CREATE TABLE IF NOT EXISTS video_files (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    resolution INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    fps INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (video_id, resolution)
);

CREATE TABLE IF NOT EXISTS video_comments (
    id INTEGER PRIMARY KEY,
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    parent_comment_id INTEGER REFERENCES video_comments(id),
    author_persona_id INTEGER NOT NULL REFERENCES personas(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    ap_id TEXT NOT NULL UNIQUE,
    removed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_comments_video ON video_comments(video_id);

CREATE TABLE IF NOT EXISTS video_playlists (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS video_playlist_items (
    playlist_id INTEGER NOT NULL REFERENCES video_playlists(id) ON DELETE CASCADE,
    video_id INTEGER NOT NULL REFERENCES videos(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, video_id)
);

-- Funkwhale-style audio hosting
CREATE TABLE IF NOT EXISTS audio_channels (
    persona_id INTEGER PRIMARY KEY REFERENCES personas(id),
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'music',
    language TEXT,
    rss_enabled INTEGER NOT NULL DEFAULT 0,
    itunes_category TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES audio_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    track_number INTEGER,
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    cover_media_id INTEGER REFERENCES media(id),
    description TEXT NOT NULL DEFAULT '',
    ap_id TEXT NOT NULL UNIQUE,
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tracks_channel ON tracks(channel_id, created_at DESC);

CREATE TABLE IF NOT EXISTS audio_playlists (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_playlist_items (
    playlist_id INTEGER NOT NULL REFERENCES audio_playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id),
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_audio_playlist_items ON audio_playlist_items(playlist_id, position);

CREATE TABLE IF NOT EXISTS podcast_series (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES audio_channels(persona_id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    language TEXT,
    category TEXT,
    itunes_category TEXT,
    cover_media_id INTEGER REFERENCES media(id),
    explicit INTEGER NOT NULL DEFAULT 0,
    rss_link TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id INTEGER PRIMARY KEY,
    series_id INTEGER NOT NULL REFERENCES podcast_series(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    description_html TEXT NOT NULL DEFAULT '',
    duration INTEGER,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    episode_number INTEGER,
    season_number INTEGER,
    explicit INTEGER NOT NULL DEFAULT 0,
    ap_id TEXT NOT NULL UNIQUE,
    published_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_series ON podcast_episodes(series_id, published_at DESC);
"#;

// ---------------------------------------------------------------------------
// Legacy upgrade SQL for consumer projects
// ---------------------------------------------------------------------------

/// Legacy upgrade for broadside databases.
///
/// Broadside has 7 tables with TEXT timestamps, inline follower data,
/// and no users table. This upgrade:
/// 1. Creates a users table with a single default user
/// 2. Restructures personas (rename columns, add user_id, convert timestamps)
/// 3. Normalizes followers into remote_accounts
/// 4. Restructures posts and delivery_queue
/// 5. Missing tables are created by the subsequent base schema run (IF NOT EXISTS)
pub const LEGACY_BROADSIDE_SQLITE: &str = r#"
-- Step 1: Create users table with a single operator user
CREATE TABLE IF NOT EXISTS users (
    id                INTEGER PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,
    oidc_subject      TEXT,
    oidc_issuer       TEXT,
    display_name      TEXT NOT NULL DEFAULT '',
    role              TEXT NOT NULL DEFAULT 'admin',
    created_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    suspended_reason  TEXT,
    did_key           TEXT,
    did_plc           TEXT,
    recovery_pubkey   TEXT,
    legal_hold        INTEGER NOT NULL DEFAULT 0,
    quota_media_bytes INTEGER,
    quota_posts_per_day INTEGER,
    quota_followers   INTEGER,
    quota_following   INTEGER,
    UNIQUE (oidc_issuer, oidc_subject)
);
INSERT OR IGNORE INTO users (id, email, display_name, role, created_at)
    SELECT 1000000000000, 'admin@localhost', 'Operator', 'admin',
           CAST(strftime('%s', 'now') AS INTEGER);

-- Step 2: Restructure personas (SQLite 12-step)
CREATE TABLE personas_new (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    bio_html        TEXT NOT NULL DEFAULT '',
    private_key_pem TEXT NOT NULL,
    public_key_pem  TEXT NOT NULL,
    avatar_media_id INTEGER,
    header_media_id INTEGER,
    is_locked       INTEGER NOT NULL DEFAULT 0,
    discoverable    INTEGER NOT NULL DEFAULT 1,
    bot             INTEGER NOT NULL DEFAULT 0,
    did_web         TEXT,
    fields_json     TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    last_status_at  INTEGER
);
INSERT INTO personas_new (id, user_id, username, display_name, bio, bio_html,
    private_key_pem, public_key_pem, fields_json, created_at)
    SELECT id, 1000000000000, username, display_name, bio, '',
           private_key, public_key,
           COALESCE(metadata, '[]'),
           CAST(strftime('%s', created_at) AS INTEGER)
    FROM personas;
DROP TABLE personas;
ALTER TABLE personas_new RENAME TO personas;
CREATE INDEX IF NOT EXISTS idx_personas_user ON personas(user_id);

-- Step 3: Create remote_accounts from follower data
CREATE TABLE IF NOT EXISTS remote_accounts (
    id                INTEGER PRIMARY KEY,
    actor_uri         TEXT NOT NULL UNIQUE,
    username          TEXT NOT NULL DEFAULT '',
    domain            TEXT NOT NULL DEFAULT '',
    display_name      TEXT NOT NULL DEFAULT '',
    bio_html          TEXT NOT NULL DEFAULT '',
    avatar_url        TEXT,
    header_url        TEXT,
    public_key_pem    TEXT NOT NULL DEFAULT '',
    public_key_id     TEXT NOT NULL DEFAULT '',
    inbox_url         TEXT NOT NULL,
    shared_inbox_url  TEXT,
    followers_url     TEXT,
    is_locked         INTEGER NOT NULL DEFAULT 0,
    bot               INTEGER NOT NULL DEFAULT 0,
    last_fetched_at   INTEGER NOT NULL,
    fetched_failed_at INTEGER,
    fetch_fail_count  INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO remote_accounts (actor_uri, inbox_url, shared_inbox_url, last_fetched_at)
    SELECT actor_uri, inbox_uri, shared_inbox_uri,
           CAST(strftime('%s', 'now') AS INTEGER)
    FROM followers;

-- Step 4: Rebuild followers with FK to remote_accounts
CREATE TABLE followers_new (
    persona_id        INTEGER NOT NULL REFERENCES personas(id),
    user_id           INTEGER NOT NULL REFERENCES users(id),
    remote_account_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    accepted_at       INTEGER NOT NULL,
    UNIQUE (persona_id, remote_account_id)
);
INSERT INTO followers_new (persona_id, user_id, remote_account_id, accepted_at)
    SELECT f.persona_id, 1000000000000, ra.id,
           CAST(strftime('%s', f.followed_at) AS INTEGER)
    FROM followers f
    JOIN remote_accounts ra ON ra.actor_uri = f.actor_uri;
DROP TABLE followers;
ALTER TABLE followers_new RENAME TO followers;
CREATE INDEX IF NOT EXISTS idx_followers_persona ON followers(persona_id);
CREATE INDEX IF NOT EXISTS idx_followers_user ON followers(user_id);

-- Step 5: Restructure posts
CREATE TABLE posts_new (
    id              INTEGER PRIMARY KEY,
    user_id         INTEGER NOT NULL REFERENCES users(id),
    persona_id      INTEGER NOT NULL REFERENCES personas(id),
    ap_id           TEXT NOT NULL UNIQUE,
    in_reply_to_id  INTEGER,
    in_reply_to_uri TEXT,
    boost_of_id     INTEGER,
    boost_of_uri    TEXT,
    content         TEXT NOT NULL,
    content_html    TEXT NOT NULL,
    spoiler_text    TEXT NOT NULL DEFAULT '',
    visibility      TEXT NOT NULL DEFAULT 'public',
    sensitive       INTEGER NOT NULL DEFAULT 0,
    language        TEXT,
    context_url     TEXT,
    created_at      INTEGER NOT NULL,
    edited_at       INTEGER,
    deleted_at      INTEGER,
    deleted_reason  TEXT
);
INSERT INTO posts_new (id, user_id, persona_id, ap_id, content, content_html, created_at)
    SELECT id, 1000000000000, persona_id,
           COALESCE(ap_id, 'https://localhost/posts/' || id),
           COALESCE(content_text, ''), content_html,
           CAST(strftime('%s', published_at) AS INTEGER)
    FROM posts;
DROP TABLE posts;
ALTER TABLE posts_new RENAME TO posts;
CREATE INDEX IF NOT EXISTS idx_posts_user ON posts(user_id);
CREATE INDEX IF NOT EXISTS idx_posts_persona_created ON posts(persona_id, created_at DESC);

-- Step 6: Restructure delivery_queue
CREATE TABLE delivery_queue_new (
    id                 INTEGER PRIMARY KEY,
    target_inbox       TEXT NOT NULL,
    sender_persona_id  INTEGER NOT NULL REFERENCES personas(id),
    activity_json      TEXT NOT NULL,
    attempts           INTEGER NOT NULL DEFAULT 0,
    next_attempt_at    INTEGER NOT NULL,
    last_error         TEXT,
    created_at         INTEGER NOT NULL,
    delivered_at       INTEGER,
    dead_at            INTEGER
);
INSERT INTO delivery_queue_new (id, target_inbox, sender_persona_id, activity_json,
    attempts, next_attempt_at, created_at)
    SELECT id, inbox_uri, persona_id, COALESCE(activity_json, '{}'),
           attempts, CAST(strftime('%s', next_retry) AS INTEGER),
           CAST(strftime('%s', created_at) AS INTEGER)
    FROM delivery_queue;
DROP TABLE delivery_queue;
ALTER TABLE delivery_queue_new RENAME TO delivery_queue;
CREATE INDEX IF NOT EXISTS idx_delivery_pending
    ON delivery_queue(next_attempt_at) WHERE delivered_at IS NULL AND dead_at IS NULL;

-- Step 7: Restructure relays
CREATE TABLE relays_new (
    id         INTEGER PRIMARY KEY,
    actor_uri  TEXT NOT NULL UNIQUE,
    inbox_url  TEXT NOT NULL,
    persona_id INTEGER REFERENCES personas(id),
    follow_id  TEXT NOT NULL DEFAULT '',
    state      TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL
);
INSERT INTO relays_new (id, actor_uri, inbox_url, persona_id, follow_id, state, created_at)
    SELECT id, actor_uri, inbox_uri, persona_id, follow_id,
           COALESCE(status, 'pending'),
           CAST(strftime('%s', created_at) AS INTEGER)
    FROM relays;
DROP TABLE relays;
ALTER TABLE relays_new RENAME TO relays;

-- Step 8: Migrate post_media to media table format
CREATE TABLE IF NOT EXISTS media (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    persona_id  INTEGER NOT NULL REFERENCES personas(id),
    post_id     INTEGER REFERENCES posts(id),
    file_path   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    file_size   INTEGER NOT NULL DEFAULT 0,
    width       INTEGER,
    height      INTEGER,
    blurhash    TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
INSERT OR IGNORE INTO media (id, user_id, persona_id, post_id, file_path, mime_type,
    description, created_at)
    SELECT pm.id, 1000000000000, p.persona_id, pm.post_id,
           pm.file_path, pm.mime_type,
           COALESCE(pm.description, ''),
           CAST(strftime('%s', 'now') AS INTEGER)
    FROM post_media pm
    JOIN posts p ON p.id = pm.post_id;
DROP TABLE IF EXISTS post_media;

-- Step 9: Migrate cards to link_cards
CREATE TABLE IF NOT EXISTS link_cards (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    url           TEXT NOT NULL UNIQUE,
    card_type     TEXT NOT NULL DEFAULT 'link',
    title         TEXT NOT NULL DEFAULT '',
    description   TEXT NOT NULL DEFAULT '',
    image_url     TEXT,
    author_name   TEXT NOT NULL DEFAULT '',
    author_url    TEXT NOT NULL DEFAULT '',
    provider_name TEXT NOT NULL DEFAULT '',
    provider_url  TEXT NOT NULL DEFAULT '',
    html          TEXT NOT NULL DEFAULT '',
    width         INTEGER NOT NULL DEFAULT 0,
    height        INTEGER NOT NULL DEFAULT 0,
    fetched_at    INTEGER NOT NULL,
    failed        INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO link_cards (url, title, description, image_url, fetched_at)
    SELECT url, COALESCE(title, ''), COALESCE(description, ''),
           image_url, CAST(strftime('%s', 'now') AS INTEGER)
    FROM cards;
DROP TABLE IF EXISTS cards;

-- Step 10: Clean up legacy version tracking
DROP TABLE IF EXISTS schema_meta;
"#;

/// Legacy upgrade for smallhold databases.
///
/// Smallhold has a monolithic `accounts` table that needs splitting into
/// `users` + `personas`. Every FK in 20+ tables changes.
pub const LEGACY_SMALLHOLD_SQLITE: &str = r#"
-- Step 1: Create users table with a single owner
CREATE TABLE IF NOT EXISTS users (
    id                TEXT PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,
    oidc_subject      TEXT,
    oidc_issuer       TEXT,
    display_name      TEXT NOT NULL DEFAULT '',
    role              TEXT NOT NULL DEFAULT 'admin',
    created_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    suspended_reason  TEXT,
    did_key           TEXT,
    did_plc           TEXT,
    recovery_pubkey   TEXT,
    legal_hold        INTEGER NOT NULL DEFAULT 0,
    quota_media_bytes INTEGER,
    quota_posts_per_day INTEGER,
    quota_followers   INTEGER,
    quota_following   INTEGER,
    UNIQUE (oidc_issuer, oidc_subject)
);
INSERT OR IGNORE INTO users (id, email, display_name, role, created_at, did_key, recovery_pubkey)
    SELECT 1000000000001, COALESCE(
        (SELECT email FROM admin LIMIT 1), 'admin@localhost'),
        COALESCE(display_name, username), 'admin', created_at,
        did_key, recovery_pubkey
    FROM accounts LIMIT 1;

-- Step 2: Create personas from accounts
CREATE TABLE IF NOT EXISTS personas (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    bio_html        TEXT NOT NULL DEFAULT '',
    private_key_pem TEXT NOT NULL,
    public_key_pem  TEXT NOT NULL,
    avatar_media_id INTEGER,
    header_media_id INTEGER,
    is_locked       INTEGER NOT NULL DEFAULT 0,
    discoverable    INTEGER NOT NULL DEFAULT 1,
    bot             INTEGER NOT NULL DEFAULT 0,
    did_web         TEXT,
    fields_json     TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    last_status_at  INTEGER
);
INSERT INTO personas (id, user_id, username, display_name, bio, bio_html,
    private_key_pem, public_key_pem, avatar_media_id, header_media_id,
    is_locked, discoverable, bot, fields_json, created_at, last_status_at)
    SELECT id, 1000000000001, username, display_name, bio,
           COALESCE(bio_html, ''), private_key_pem, public_key_pem,
           avatar_media_id, header_media_id,
           is_locked, discoverable, bot,
           COALESCE(fields_json, '[]'), created_at, last_status_at
    FROM accounts;

-- Step 3: Add user_id + persona_id columns to posts, update from accounts
ALTER TABLE posts ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE posts ADD COLUMN persona_id TEXT REFERENCES personas(id);
UPDATE posts SET user_id = 1000000000001,
    persona_id = account_id;

-- Step 4: Add user_id + persona_id to media
ALTER TABLE media ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE media ADD COLUMN persona_id TEXT REFERENCES personas(id);
UPDATE media SET user_id = 1000000000001,
    persona_id = account_id;

-- Step 5: Rebuild followers with user_id + persona_id instead of local_account_id
CREATE TABLE followers_new (
    persona_id        TEXT NOT NULL,
    user_id           TEXT NOT NULL REFERENCES users(id),
    remote_account_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    accepted_at       INTEGER NOT NULL,
    UNIQUE (persona_id, remote_account_id)
);
INSERT INTO followers_new (persona_id, user_id, remote_account_id, accepted_at)
    SELECT local_account_id, 1000000000001, remote_account_id, accepted_at
    FROM followers;
DROP TABLE followers;
ALTER TABLE followers_new RENAME TO followers;
CREATE INDEX IF NOT EXISTS idx_followers_persona ON followers(persona_id);
CREATE INDEX IF NOT EXISTS idx_followers_user ON followers(user_id);

-- Step 6: Update follow_requests
ALTER TABLE follow_requests RENAME COLUMN target_account_id TO target_persona_id;

-- Step 7: Rebuild follows with persona_id/user_id
CREATE TABLE follows_new (
    persona_id          TEXT NOT NULL,
    user_id             TEXT NOT NULL REFERENCES users(id),
    followee_remote_id  INTEGER REFERENCES remote_accounts(id),
    followee_persona_id TEXT,
    created_at          INTEGER NOT NULL,
    show_reblogs        INTEGER NOT NULL DEFAULT 1,
    notify              INTEGER NOT NULL DEFAULT 0,
    CHECK ((followee_remote_id IS NOT NULL) != (followee_persona_id IS NOT NULL))
);
INSERT INTO follows_new (persona_id, user_id, followee_remote_id, followee_persona_id,
    created_at, show_reblogs, notify)
    SELECT follower_id, 1000000000001, followee_remote_id, followee_persona_id,
           created_at, show_reblogs, notify
    FROM follows;
DROP TABLE follows;
ALTER TABLE follows_new RENAME TO follows;
CREATE INDEX IF NOT EXISTS idx_follows_persona ON follows(persona_id);
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows(user_id);

-- Step 8: Update favourites
ALTER TABLE favourites ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE favourites RENAME COLUMN account_id TO persona_id;
UPDATE favourites SET user_id = 1000000000001;

-- Step 9: Update bookmarks
ALTER TABLE bookmarks ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE bookmarks RENAME COLUMN account_id TO persona_id;
UPDATE bookmarks SET user_id = 1000000000001;

-- Step 10: Update pinned_posts
ALTER TABLE pinned_posts RENAME COLUMN account_id TO persona_id;

-- Step 11: Update notifications
ALTER TABLE notifications ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE notifications RENAME COLUMN account_id TO persona_id;
ALTER TABLE notifications RENAME COLUMN from_account_id TO from_persona_id;
UPDATE notifications SET user_id = 1000000000001;

-- Step 12: Update delivery_queue
ALTER TABLE delivery_queue RENAME COLUMN sender_account_id TO sender_persona_id;
ALTER TABLE delivery_queue ADD COLUMN last_error TEXT;
ALTER TABLE delivery_queue ADD COLUMN delivered_at INTEGER;
ALTER TABLE delivery_queue ADD COLUMN dead_at INTEGER;

-- Step 13: Update oauth_tokens
ALTER TABLE oauth_tokens ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE oauth_tokens RENAME COLUMN account_id TO persona_id;
UPDATE oauth_tokens SET user_id = 1000000000001;

-- Step 14: Update oauth_authz_codes
ALTER TABLE oauth_authz_codes ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE oauth_authz_codes RENAME COLUMN account_id TO persona_id;
UPDATE oauth_authz_codes SET user_id = 1000000000001;

-- Step 15: Update idempotency_keys
ALTER TABLE idempotency_keys RENAME COLUMN account_id TO user_id;

-- Step 16: Update push_subscriptions
ALTER TABLE push_subscriptions RENAME COLUMN account_id TO user_id;

-- Step 17: Update also_known_as
ALTER TABLE also_known_as RENAME COLUMN account_id TO persona_id;

-- Step 18: Update conversation tables
ALTER TABLE conversation_read_markers RENAME COLUMN account_id TO user_id;
ALTER TABLE conversation_hidden RENAME COLUMN account_id TO user_id;

-- Step 19: Update markers
ALTER TABLE markers RENAME COLUMN account_id TO user_id;

-- Step 20: Update lists
ALTER TABLE lists RENAME COLUMN account_id TO user_id;

-- Step 21: Update followed_tags
ALTER TABLE followed_tags RENAME COLUMN account_id TO user_id;

-- Step 22: Update filters
ALTER TABLE filters RENAME COLUMN account_id TO user_id;

-- Step 23: Update scheduled_statuses
ALTER TABLE scheduled_statuses ADD COLUMN user_id TEXT REFERENCES users(id);
ALTER TABLE scheduled_statuses RENAME COLUMN account_id TO persona_id;
UPDATE scheduled_statuses SET user_id = 1000000000001;

-- Step 24: Update webauthn_credentials
ALTER TABLE webauthn_credentials ADD COLUMN user_id TEXT REFERENCES users(id);
UPDATE webauthn_credentials SET user_id = 1000000000001;

-- Step 25: Drop the old accounts table
DROP TABLE IF EXISTS accounts;

-- Step 26: Drop old admin table (replaced by admin_tokens in canonical)
DROP TABLE IF EXISTS admin;
"#;

/// Pre-built legacy upgrade structs for consumer projects.
pub const LEGACY_BROADSIDE: LegacyUpgrade = LegacyUpgrade {
    from: "broadside",
    sql_sqlite: LEGACY_BROADSIDE_SQLITE,
    sql_postgres: "", // ponytail: broadside is SQLite-only for now
};

pub const LEGACY_SMALLHOLD: LegacyUpgrade = LegacyUpgrade {
    from: "smallhold",
    sql_sqlite: LEGACY_SMALLHOLD_SQLITE,
    sql_postgres: "", // ponytail: smallhold is SQLite-only for now
};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sqlite")]
    async fn fresh_sqlite() -> (Pool, sqlx::SqlitePool) {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        (Pool::Sqlite(pool.clone()), pool)
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn sqlite_schema_creates_all_tables() {
        let (db, pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();

        for expected in [
            "users", "personas", "remote_accounts", "posts", "followers",
            "delivery_queue", "media", "relays",
        ] {
            assert!(names.contains(&expected), "missing table: {expected}");
        }

        // Should have 40+ tables total (base tables + schema_version)
        assert!(
            names.len() >= 40,
            "expected 40+ tables, got {}: {names:?}",
            names.len()
        );
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn fresh_db_reaches_current_version() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn migrate_is_idempotent() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();
        // Running again should be a no-op
        migrate(&db).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn migration_v1_adds_did_columns() {
        let (db, pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        // Verify did:scid columns exist by inserting a row that uses them
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at, did_scid) \
             VALUES (999, 'test@example.com', 'Test', 'user', 1000, 'did:scid:sha256:abc')"
        ).execute(&pool).await.unwrap();

        let row: (String,) = sqlx::query_as(
            "SELECT did_scid FROM users WHERE id = 999"
        ).fetch_one(&pool).await.unwrap();

        assert_eq!(row.0, "did:scid:sha256:abc");
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn extra_migrations_run_in_order() {
        let extras = [Migration {
            version: 100,
            description: "test: add test_extra table",
            sql_sqlite: "CREATE TABLE IF NOT EXISTS test_extra (id INTEGER PRIMARY KEY, val TEXT);",
            sql_postgres: "CREATE TABLE IF NOT EXISTS test_extra (id SERIAL PRIMARY KEY, val TEXT);",
        }];

        let (db, pool) = fresh_sqlite().await;
        migrate_with(&db, &extras).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, 100);

        // Extra table should exist
        let exists = table_exists(&db, "test_extra").await.unwrap();
        assert!(exists);

        // DID columns from v1 should also exist
        let row: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'did_scid'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn detect_empty_db() {
        let (db, _pool) = fresh_sqlite().await;
        ensure_version_table(&db).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, None);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn detect_canonical_schema() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("canonical"));
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn detect_smallhold_schema() {
        let (db, pool) = fresh_sqlite().await;
        // Simulate smallhold: has accounts table, no users or personas
        sqlx::raw_sql("CREATE TABLE accounts (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("smallhold"));
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn detect_broadside_schema() {
        let (db, pool) = fresh_sqlite().await;
        // Simulate broadside: has personas but no users
        sqlx::raw_sql("CREATE TABLE personas (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("broadside"));
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn legacy_broadside_upgrade() {
        let (db, pool) = fresh_sqlite().await;

        // Simulate broadside's legacy schema (7 tables, TEXT timestamps)
        sqlx::raw_sql(r#"
            CREATE TABLE personas (
                id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '', bio TEXT NOT NULL DEFAULT '',
                private_key TEXT NOT NULL, public_key TEXT NOT NULL,
                avatar_path TEXT, header_path TEXT,
                metadata TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            INSERT INTO personas (id, username, private_key, public_key)
                VALUES ('p1', 'alice', 'privkey', 'pubkey');

            CREATE TABLE followers (
                id TEXT PRIMARY KEY, persona_id TEXT NOT NULL,
                actor_uri TEXT NOT NULL, inbox_uri TEXT NOT NULL,
                shared_inbox_uri TEXT,
                followed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            INSERT INTO followers (id, persona_id, actor_uri, inbox_uri, followed_at)
                VALUES ('f1', 'p1', 'https://remote/users/bob', 'https://remote/inbox', '2025-06-15T12:00:00Z');

            CREATE TABLE posts (
                id INTEGER PRIMARY KEY, persona_id TEXT NOT NULL,
                ap_id TEXT, source_ref TEXT, content_text TEXT NOT NULL,
                content_html TEXT NOT NULL,
                published_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            INSERT INTO posts (id, persona_id, content_text, content_html, published_at)
                VALUES (1, 'p1', 'hello', '<p>hello</p>', '2025-06-15T12:00:00Z');

            CREATE TABLE delivery_queue (
                id INTEGER PRIMARY KEY, persona_id TEXT NOT NULL,
                inbox_uri TEXT NOT NULL, activity_json TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_retry TEXT NOT NULL, created_at TEXT NOT NULL
            );

            CREATE TABLE relays (
                id INTEGER PRIMARY KEY, actor_uri TEXT NOT NULL UNIQUE,
                inbox_uri TEXT NOT NULL, persona_id TEXT,
                follow_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE post_media (
                id INTEGER PRIMARY KEY, post_id INTEGER NOT NULL,
                file_path TEXT NOT NULL, mime_type TEXT NOT NULL,
                description TEXT
            );

            CREATE TABLE cards (
                id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE,
                title TEXT, description TEXT, image_url TEXT
            );

            CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO schema_meta VALUES ('version', '4');
        "#).execute(&pool).await.unwrap();

        // Run migration with broadside legacy upgrade
        migrate_full(&db, Some(&LEGACY_BROADSIDE), &[]).await.unwrap();

        // Verify: users table exists with legacy operator
        let user: (i64,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(user.0, 1000000000000);

        // Verify: personas has user_id and renamed columns
        let persona: (i64, String) = sqlx::query_as(
            "SELECT user_id, private_key_pem FROM personas WHERE username = 'alice'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(persona.0, 1000000000000);
        assert_eq!(persona.1, "privkey");

        // Verify: followers normalized via remote_accounts
        let ra: (String,) = sqlx::query_as(
            "SELECT actor_uri FROM remote_accounts WHERE actor_uri = 'https://remote/users/bob'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(ra.0, "https://remote/users/bob");

        let follower: (i64,) = sqlx::query_as(
            "SELECT user_id FROM followers LIMIT 1"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(follower.0, 1000000000000);

        // Verify: posts has user_id and INTEGER timestamp
        let post: (i64, i64) = sqlx::query_as(
            "SELECT user_id, created_at FROM posts WHERE id = 1"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(post.0, 1000000000000);
        assert!(post.1 > 1_700_000_000, "timestamp should be epoch seconds: {}", post.1);

        // Verify: schema_meta gone, schema_version exists
        let exists = table_exists(&db, "schema_meta").await.unwrap();
        assert!(!exists);

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn legacy_smallhold_upgrade() {
        let (db, pool) = fresh_sqlite().await;

        // Simulate smallhold's legacy schema (accounts table, no users/personas)
        sqlx::raw_sql(r#"
            CREATE TABLE accounts (
                id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '',
                bio TEXT NOT NULL DEFAULT '', bio_html TEXT NOT NULL DEFAULT '',
                private_key_pem TEXT NOT NULL, public_key_pem TEXT NOT NULL,
                avatar_media_id INTEGER, header_media_id INTEGER,
                is_locked INTEGER NOT NULL DEFAULT 0,
                discoverable INTEGER NOT NULL DEFAULT 1,
                bot INTEGER NOT NULL DEFAULT 0,
                fields_json TEXT NOT NULL DEFAULT '[]',
                did_key TEXT, recovery_pubkey TEXT,
                created_at INTEGER NOT NULL, last_status_at INTEGER
            );
            INSERT INTO accounts (id, username, private_key_pem, public_key_pem, created_at)
                VALUES ('acct1', 'alice', 'privkey', 'pubkey', 1718400000);

            CREATE TABLE admin (
                id INTEGER PRIMARY KEY, email TEXT, password_hash TEXT, totp_secret TEXT
            );
            INSERT INTO admin (id, email) VALUES (1, 'alice@example.com');

            CREATE TABLE remote_accounts (
                id INTEGER PRIMARY KEY, actor_uri TEXT NOT NULL UNIQUE,
                username TEXT NOT NULL, domain TEXT NOT NULL,
                display_name TEXT NOT NULL, bio_html TEXT NOT NULL DEFAULT '',
                avatar_url TEXT, header_url TEXT,
                public_key_pem TEXT NOT NULL, public_key_id TEXT NOT NULL UNIQUE,
                inbox_url TEXT NOT NULL, shared_inbox_url TEXT, followers_url TEXT,
                is_locked INTEGER NOT NULL DEFAULT 0, bot INTEGER NOT NULL DEFAULT 0,
                last_fetched_at INTEGER NOT NULL, fetched_failed_at INTEGER,
                fetch_fail_count INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO remote_accounts (id, actor_uri, username, domain, display_name,
                public_key_pem, public_key_id, inbox_url, last_fetched_at)
                VALUES (1, 'https://remote/users/bob', 'bob', 'remote', 'Bob',
                        'pk', 'https://remote/users/bob#key', 'https://remote/inbox', 1718400000);

            CREATE TABLE followers (
                local_account_id TEXT NOT NULL, remote_account_id INTEGER NOT NULL,
                accepted_at INTEGER NOT NULL,
                UNIQUE (local_account_id, remote_account_id)
            );
            INSERT INTO followers VALUES ('acct1', 1, 1718400000);

            CREATE TABLE posts (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                ap_id TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL, content_html TEXT NOT NULL,
                visibility TEXT NOT NULL, created_at INTEGER NOT NULL
            );
            INSERT INTO posts (id, account_id, ap_id, content, content_html, visibility, created_at)
                VALUES (1, 'acct1', 'https://example.com/posts/1', 'hello', '<p>hello</p>', 'public', 1718400000);

            CREATE TABLE media (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                post_id INTEGER, file_path TEXT NOT NULL, mime_type TEXT NOT NULL,
                file_size INTEGER NOT NULL, description TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );

            CREATE TABLE favourites (
                account_id TEXT NOT NULL, post_id INTEGER, remote_post_id INTEGER,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE bookmarks (
                account_id TEXT NOT NULL, post_id INTEGER, remote_post_id INTEGER,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE pinned_posts (
                account_id TEXT NOT NULL, post_id INTEGER NOT NULL, pinned_at INTEGER NOT NULL
            );

            CREATE TABLE notifications (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                kind TEXT NOT NULL, from_account_id TEXT,
                post_id INTEGER, created_at INTEGER NOT NULL, read_at INTEGER
            );

            CREATE TABLE delivery_queue (
                id INTEGER PRIMARY KEY, sender_account_id TEXT NOT NULL,
                target_inbox TEXT NOT NULL, activity_json TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_attempt_at INTEGER NOT NULL, created_at INTEGER NOT NULL
            );

            CREATE TABLE oauth_tokens (
                id INTEGER PRIMARY KEY, token_hash TEXT NOT NULL UNIQUE,
                app_id INTEGER NOT NULL, account_id TEXT,
                scopes TEXT NOT NULL, created_at INTEGER NOT NULL
            );

            CREATE TABLE oauth_authz_codes (
                code_hash TEXT PRIMARY KEY, app_id INTEGER NOT NULL,
                account_id TEXT NOT NULL, scopes TEXT NOT NULL,
                redirect_uri TEXT NOT NULL, expires_at INTEGER NOT NULL
            );

            CREATE TABLE idempotency_keys (
                key_hash TEXT PRIMARY KEY, account_id TEXT NOT NULL,
                post_id INTEGER NOT NULL, created_at INTEGER NOT NULL
            );

            CREATE TABLE push_subscriptions (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL UNIQUE,
                endpoint TEXT NOT NULL, key_p256dh TEXT NOT NULL, key_auth TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE also_known_as (
                account_id TEXT NOT NULL, uri TEXT NOT NULL, UNIQUE(account_id, uri)
            );

            CREATE TABLE conversation_read_markers (
                account_id TEXT NOT NULL, post_id INTEGER NOT NULL
            );

            CREATE TABLE conversation_hidden (
                account_id TEXT NOT NULL, post_id INTEGER NOT NULL
            );

            CREATE TABLE markers (
                account_id TEXT NOT NULL, timeline TEXT NOT NULL,
                last_read_id TEXT NOT NULL, version INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL, UNIQUE(account_id, timeline)
            );

            CREATE TABLE lists (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                title TEXT NOT NULL, replies_policy TEXT NOT NULL DEFAULT 'list',
                created_at INTEGER NOT NULL
            );

            CREATE TABLE followed_tags (
                account_id TEXT NOT NULL, tag TEXT NOT NULL,
                created_at INTEGER NOT NULL, UNIQUE(account_id, tag)
            );

            CREATE TABLE filters (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                title TEXT NOT NULL, context TEXT NOT NULL DEFAULT '[]',
                filter_action TEXT NOT NULL DEFAULT 'warn',
                expires_at INTEGER, created_at INTEGER NOT NULL
            );

            CREATE TABLE scheduled_statuses (
                id INTEGER PRIMARY KEY, account_id TEXT NOT NULL,
                scheduled_at INTEGER NOT NULL, params_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE webauthn_credentials (
                id INTEGER PRIMARY KEY, credential_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE follows (
                follower_id TEXT NOT NULL, followee_remote_id INTEGER,
                followee_persona_id TEXT,
                created_at INTEGER NOT NULL, show_reblogs INTEGER NOT NULL DEFAULT 1,
                notify INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE follow_requests (
                id INTEGER PRIMARY KEY, requester_remote_id INTEGER NOT NULL,
                target_account_id TEXT NOT NULL, ap_id TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            );
        "#).execute(&pool).await.unwrap();

        // Run migration with smallhold legacy upgrade
        migrate_full(&db, Some(&LEGACY_SMALLHOLD), &[]).await.unwrap();

        // Verify: users table has the legacy owner with email from admin table
        let user: (i64, String) = sqlx::query_as(
            "SELECT id, email FROM users LIMIT 1"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(user.0, 1000000000001);
        assert_eq!(user.1, "alice@example.com");

        // Verify: personas created from accounts
        let persona: (i64, String) = sqlx::query_as(
            "SELECT user_id, username FROM personas WHERE username = 'alice'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(persona.0, 1000000000001);
        assert_eq!(persona.1, "alice");

        // Verify: accounts table is gone
        let exists = table_exists(&db, "accounts").await.unwrap();
        assert!(!exists);

        // Verify: admin table is gone
        let exists = table_exists(&db, "admin").await.unwrap();
        assert!(!exists);

        // Verify: posts has user_id and persona_id
        // After legacy upgrade + v4 migration, user_id values are integers but
        // the column may retain TEXT affinity in SQLite. Use CAST for safe decode.
        let post: (i64,) = sqlx::query_as(
            "SELECT CAST(user_id AS INTEGER) FROM posts WHERE id = 1"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(post.0, 1000000000001);

        // Verify: followers rebuilt with persona_id
        let follower: (i64,) = sqlx::query_as(
            "SELECT CAST(user_id AS INTEGER) FROM followers LIMIT 1"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(follower.0, 1000000000001);

        // Verify: all canonical tables exist after base schema fill
        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);

        // Verify: did:scid columns exist (from v1 migration)
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'did_scid'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(count.0, 1);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn version_too_new_is_error() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        // Manually set version higher than we support
        set_version(&db, 999).await.unwrap();

        let result = migrate(&db).await;
        assert!(matches!(result, Err(MigrateError::VersionTooNew(999, _))));
    }
}
