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
//! - **`db-postgres`** — PostgreSQL support via sqlx.
//! - **`db-sqlite`** — SQLite support via sqlx.

#[cfg(not(any(feature = "db-postgres", feature = "db-sqlite")))]
compile_error!("fieldwork::db requires at least one of `db-postgres` or `db-sqlite` features");

/// Database pool abstraction. Projects using a single backend can match
/// on the one variant they need; projects supporting both dispatch here.
#[derive(Clone)]
pub enum Pool {
    #[cfg(feature = "db-postgres")]
    Postgres(sqlx::PgPool),
    #[cfg(feature = "db-sqlite")]
    Sqlite(sqlx::SqlitePool),
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
const CURRENT_VERSION: u32 = 1;

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
];

/// Run schema migrations. Call at startup.
///
/// For fresh databases, creates the full schema (version 0) then runs all
/// incremental migrations. For existing databases, runs only pending migrations.
pub async fn migrate(pool: &Pool) -> Result<(), MigrateError> {
    migrate_with(pool, &[]).await
}

/// Run schema migrations including project-specific extras.
///
/// Extra migrations are interleaved by version number with the built-in chain.
/// Use this for project-specific tables (e.g., broadside's `feed_state`).
pub async fn migrate_with(
    pool: &Pool,
    extras: &[Migration],
) -> Result<(), MigrateError> {
    ensure_version_table(pool).await?;

    let current = get_version(pool).await?;

    if current > max_version(extras) {
        return Err(MigrateError::VersionTooNew(current, max_version(extras)));
    }

    // Fresh database: run full CREATE TABLE schema as version 0
    if current == 0 && !has_any_tables(pool).await? {
        run_base_schema(pool).await?;
        set_version(pool, 0).await?;
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
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => { sqlx::raw_sql(sql).execute(pg).await?; }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => { sqlx::raw_sql(sql).execute(sq).await?; }
    }
    Ok(())
}

async fn get_version(pool: &Pool) -> Result<u32, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT version FROM schema_version LIMIT 1")
                .fetch_optional(pg)
                .await?
        }
        #[cfg(feature = "db-sqlite")]
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
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(delete).execute(pg).await?;
            sqlx::raw_sql(&insert).execute(pg).await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::raw_sql(delete).execute(sq).await?;
            sqlx::raw_sql(&insert).execute(sq).await?;
        }
    }
    Ok(())
}

async fn has_any_tables(pool: &Pool) -> Result<bool, sqlx::Error> {
    let count: (i32,) = match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::int FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name != 'schema_version'"
            ).fetch_one(pg).await?
        }
        #[cfg(feature = "db-sqlite")]
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
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*)::int FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name = $1"
            ).bind(table).fetch_one(pg).await?
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1"
            ).bind(table).fetch_one(sq).await?
        }
    };
    Ok(count.0 > 0)
}

async fn run_base_schema(pool: &Pool) -> Result<(), MigrateError> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(SCHEMA_POSTGRES).execute(pg).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: 0, description: "base schema", source: e
                })?;
        }
        #[cfg(feature = "db-sqlite")]
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
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::raw_sql(migration.sql_postgres).execute(pg).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: migration.version,
                    description: migration.description,
                    source: e,
                })?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::raw_sql(migration.sql_sqlite).execute(sq).await
                .map_err(|e| MigrateError::MigrationFailed {
                    version: migration.version,
                    description: migration.description,
                    source: e,
                })?;
        }
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

#[cfg(feature = "db-postgres")]
const SCHEMA_POSTGRES: &str = r#"
-- Users: identity + auth. One user can own multiple personas.
CREATE TABLE IF NOT EXISTS users (
    id                UUID PRIMARY KEY,
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
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
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
    user_id         UUID NOT NULL REFERENCES users(id),
    persona_id      UUID NOT NULL REFERENCES personas(id),
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
    deleted_reason  TEXT
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
    user_id     UUID NOT NULL REFERENCES users(id),
    persona_id  UUID NOT NULL REFERENCES personas(id),
    post_id     BIGINT REFERENCES posts(id),
    file_path   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    file_size   BIGINT NOT NULL,
    width       INTEGER,
    height      INTEGER,
    blurhash    TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_user ON media(user_id);

-- Inbound followers (remote actors following local personas).
CREATE TABLE IF NOT EXISTS followers (
    persona_id        UUID NOT NULL REFERENCES personas(id),
    user_id           UUID NOT NULL REFERENCES users(id),
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
    target_persona_id   UUID NOT NULL REFERENCES personas(id),
    ap_id               TEXT NOT NULL UNIQUE,
    created_at          INTEGER NOT NULL
);

-- Outbound follows (local personas following others).
CREATE TABLE IF NOT EXISTS follows (
    persona_id          UUID NOT NULL REFERENCES personas(id),
    user_id             UUID NOT NULL REFERENCES users(id),
    followee_remote_id  BIGINT REFERENCES remote_accounts(id),
    followee_persona_id UUID REFERENCES personas(id),
    created_at          INTEGER NOT NULL,
    show_reblogs        BOOLEAN NOT NULL DEFAULT true,
    notify              BOOLEAN NOT NULL DEFAULT false,
    CHECK ((followee_remote_id IS NOT NULL) != (followee_persona_id IS NOT NULL)),
    UNIQUE (persona_id, followee_remote_id, followee_persona_id)
);
CREATE INDEX IF NOT EXISTS idx_follows_persona ON follows(persona_id);
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows(user_id);

-- Favourites / likes.
CREATE TABLE IF NOT EXISTS favourites (
    user_id        UUID NOT NULL REFERENCES users(id),
    persona_id     UUID NOT NULL REFERENCES personas(id),
    post_id        BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL)),
    UNIQUE (persona_id, post_id, remote_post_id)
);

-- Bookmarks.
CREATE TABLE IF NOT EXISTS bookmarks (
    user_id        UUID NOT NULL REFERENCES users(id),
    persona_id     UUID NOT NULL REFERENCES personas(id),
    post_id        BIGINT REFERENCES posts(id),
    remote_post_id BIGINT REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    UNIQUE (persona_id, post_id, remote_post_id)
);

-- Pinned posts.
CREATE TABLE IF NOT EXISTS pinned_posts (
    persona_id UUID NOT NULL REFERENCES personas(id),
    post_id    BIGINT NOT NULL REFERENCES posts(id),
    pinned_at  INTEGER NOT NULL,
    UNIQUE (persona_id, post_id)
);

-- Notifications.
CREATE TABLE IF NOT EXISTS notifications (
    id                     BIGINT PRIMARY KEY,
    user_id                UUID NOT NULL REFERENCES users(id),
    persona_id             UUID NOT NULL REFERENCES personas(id),
    kind                   TEXT NOT NULL,
    from_persona_id        UUID REFERENCES personas(id),
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
    mentioned_persona_id UUID REFERENCES personas(id)
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
    user_id      UUID REFERENCES users(id),
    persona_id   UUID REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

-- OAuth authorization codes.
CREATE TABLE IF NOT EXISTS oauth_authz_codes (
    code_hash    TEXT PRIMARY KEY,
    app_id       BIGINT NOT NULL REFERENCES oauth_apps(id),
    user_id      UUID NOT NULL REFERENCES users(id),
    persona_id   UUID NOT NULL REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

-- Delivery queue for outbound ActivityPub activities.
CREATE TABLE IF NOT EXISTS delivery_queue (
    id                 BIGINT PRIMARY KEY,
    target_inbox       TEXT NOT NULL,
    sender_persona_id  UUID NOT NULL REFERENCES personas(id),
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
    persona_id UUID REFERENCES personas(id),
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
    user_id           UUID NOT NULL REFERENCES users(id),
    persona_id        UUID NOT NULL REFERENCES personas(id),
    target_persona_id UUID REFERENCES personas(id),
    target_remote_id  BIGINT REFERENCES remote_accounts(id),
    created_at        INTEGER NOT NULL
);

-- User mutes.
CREATE TABLE IF NOT EXISTS mutes (
    user_id           UUID NOT NULL REFERENCES users(id),
    persona_id        UUID NOT NULL REFERENCES personas(id),
    target_persona_id UUID REFERENCES personas(id),
    target_remote_id  BIGINT REFERENCES remote_accounts(id),
    expires_at        INTEGER,
    created_at        INTEGER NOT NULL
);

-- Idempotency keys for Mastodon Client API.
CREATE TABLE IF NOT EXISTS idempotency_keys (
    key_hash   TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id),
    post_id    BIGINT NOT NULL REFERENCES posts(id),
    created_at INTEGER NOT NULL
);

-- Push notification subscriptions (Web Push).
CREATE TABLE IF NOT EXISTS push_subscriptions (
    id               BIGINT PRIMARY KEY,
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
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
    created_by UUID REFERENCES users(id),
    used_by    UUID REFERENCES users(id),
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
    user_id        UUID NOT NULL REFERENCES users(id),
    title          TEXT NOT NULL,
    replies_policy TEXT NOT NULL DEFAULT 'list',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_accounts (
    list_id      BIGINT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    persona_id   UUID,
    remote_id    BIGINT,
    UNIQUE (list_id, persona_id, remote_id)
);

-- Followed tags.
CREATE TABLE IF NOT EXISTS followed_tags (
    user_id    UUID NOT NULL REFERENCES users(id),
    tag        TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (user_id, tag)
);

-- Content filters.
CREATE TABLE IF NOT EXISTS filters (
    id            BIGINT PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id),
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
    user_id      UUID NOT NULL REFERENCES users(id),
    persona_id   UUID NOT NULL REFERENCES personas(id),
    scheduled_at INTEGER NOT NULL,
    params_json  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scheduled_due ON scheduled_statuses(scheduled_at);

-- Timeline markers (Mastodon Client API).
CREATE TABLE IF NOT EXISTS markers (
    user_id      UUID NOT NULL REFERENCES users(id),
    timeline     TEXT NOT NULL,
    last_read_id TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL,
    UNIQUE (user_id, timeline)
);

-- WebAuthn credentials.
CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id              BIGINT PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id),
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
    persona_id UUID NOT NULL REFERENCES personas(id),
    uri        TEXT NOT NULL,
    UNIQUE (persona_id, uri)
);

-- Conversation tracking.
CREATE TABLE IF NOT EXISTS conversation_read_markers (
    user_id UUID NOT NULL REFERENCES users(id),
    post_id BIGINT NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS conversation_hidden (
    user_id UUID NOT NULL REFERENCES users(id),
    post_id BIGINT NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

-- Feed state (RSS/Atom feed polling, used by broadside).
CREATE TABLE IF NOT EXISTS feed_state (
    feed_url     TEXT PRIMARY KEY,
    persona_id   UUID NOT NULL REFERENCES personas(id),
    last_seen_id TEXT,
    last_poll    INTEGER
);
"#;

// ---------------------------------------------------------------------------
// SQLite schema
// ---------------------------------------------------------------------------

#[cfg(feature = "db-sqlite")]
const SCHEMA_SQLITE: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id                TEXT PRIMARY KEY,
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
    user_id         TEXT NOT NULL REFERENCES users(id),
    persona_id      TEXT NOT NULL REFERENCES personas(id),
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
    deleted_reason  TEXT
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
    user_id     TEXT NOT NULL REFERENCES users(id),
    persona_id  TEXT NOT NULL REFERENCES personas(id),
    post_id     INTEGER REFERENCES posts(id),
    file_path   TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    file_size   INTEGER NOT NULL,
    width       INTEGER,
    height      INTEGER,
    blurhash    TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_user ON media(user_id);

CREATE TABLE IF NOT EXISTS followers (
    persona_id        TEXT NOT NULL REFERENCES personas(id),
    user_id           TEXT NOT NULL REFERENCES users(id),
    remote_account_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    accepted_at       INTEGER NOT NULL,
    UNIQUE (persona_id, remote_account_id)
);
CREATE INDEX IF NOT EXISTS idx_followers_persona ON followers(persona_id);
CREATE INDEX IF NOT EXISTS idx_followers_user ON followers(user_id);

CREATE TABLE IF NOT EXISTS follow_requests (
    id                  INTEGER PRIMARY KEY,
    requester_remote_id INTEGER NOT NULL REFERENCES remote_accounts(id),
    target_persona_id   TEXT NOT NULL REFERENCES personas(id),
    ap_id               TEXT NOT NULL UNIQUE,
    created_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS follows (
    persona_id          TEXT NOT NULL REFERENCES personas(id),
    user_id             TEXT NOT NULL REFERENCES users(id),
    followee_remote_id  INTEGER REFERENCES remote_accounts(id),
    followee_persona_id TEXT REFERENCES personas(id),
    created_at          INTEGER NOT NULL,
    show_reblogs        INTEGER NOT NULL DEFAULT 1,
    notify              INTEGER NOT NULL DEFAULT 0,
    CHECK ((followee_remote_id IS NOT NULL) != (followee_persona_id IS NOT NULL)),
    UNIQUE (persona_id, followee_remote_id, followee_persona_id)
);
CREATE INDEX IF NOT EXISTS idx_follows_persona ON follows(persona_id);
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows(user_id);

CREATE TABLE IF NOT EXISTS favourites (
    user_id        TEXT NOT NULL REFERENCES users(id),
    persona_id     TEXT NOT NULL REFERENCES personas(id),
    post_id        INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    CHECK ((post_id IS NOT NULL) != (remote_post_id IS NOT NULL)),
    UNIQUE (persona_id, post_id, remote_post_id)
);

CREATE TABLE IF NOT EXISTS bookmarks (
    user_id        TEXT NOT NULL REFERENCES users(id),
    persona_id     TEXT NOT NULL REFERENCES personas(id),
    post_id        INTEGER REFERENCES posts(id),
    remote_post_id INTEGER REFERENCES remote_posts(id),
    created_at     INTEGER NOT NULL,
    UNIQUE (persona_id, post_id, remote_post_id)
);

CREATE TABLE IF NOT EXISTS pinned_posts (
    persona_id TEXT NOT NULL REFERENCES personas(id),
    post_id    INTEGER NOT NULL REFERENCES posts(id),
    pinned_at  INTEGER NOT NULL,
    UNIQUE (persona_id, post_id)
);

CREATE TABLE IF NOT EXISTS notifications (
    id                     INTEGER PRIMARY KEY,
    user_id                TEXT NOT NULL REFERENCES users(id),
    persona_id             TEXT NOT NULL REFERENCES personas(id),
    kind                   TEXT NOT NULL,
    from_persona_id        TEXT REFERENCES personas(id),
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
    mentioned_persona_id TEXT REFERENCES personas(id)
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
    user_id      TEXT REFERENCES users(id),
    persona_id   TEXT REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

CREATE TABLE IF NOT EXISTS oauth_authz_codes (
    code_hash    TEXT PRIMARY KEY,
    app_id       INTEGER NOT NULL REFERENCES oauth_apps(id),
    user_id      TEXT NOT NULL REFERENCES users(id),
    persona_id   TEXT NOT NULL REFERENCES personas(id),
    scopes       TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS delivery_queue (
    id                 INTEGER PRIMARY KEY,
    target_inbox       TEXT NOT NULL,
    sender_persona_id  TEXT NOT NULL REFERENCES personas(id),
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
    persona_id TEXT REFERENCES personas(id),
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
    user_id           TEXT NOT NULL REFERENCES users(id),
    persona_id        TEXT NOT NULL REFERENCES personas(id),
    target_persona_id TEXT REFERENCES personas(id),
    target_remote_id  INTEGER REFERENCES remote_accounts(id),
    created_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mutes (
    user_id           TEXT NOT NULL REFERENCES users(id),
    persona_id        TEXT NOT NULL REFERENCES personas(id),
    target_persona_id TEXT REFERENCES personas(id),
    target_remote_id  INTEGER REFERENCES remote_accounts(id),
    expires_at        INTEGER,
    created_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key_hash   TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    post_id    INTEGER NOT NULL REFERENCES posts(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id               INTEGER PRIMARY KEY,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
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
    created_by TEXT REFERENCES users(id),
    used_by    TEXT REFERENCES users(id),
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
    user_id        TEXT NOT NULL REFERENCES users(id),
    title          TEXT NOT NULL,
    replies_policy TEXT NOT NULL DEFAULT 'list',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_accounts (
    list_id    INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    persona_id TEXT,
    remote_id  INTEGER,
    UNIQUE (list_id, persona_id, remote_id)
);

CREATE TABLE IF NOT EXISTS followed_tags (
    user_id    TEXT NOT NULL REFERENCES users(id),
    tag        TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (user_id, tag)
);

CREATE TABLE IF NOT EXISTS filters (
    id            INTEGER PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id),
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
    user_id      TEXT NOT NULL REFERENCES users(id),
    persona_id   TEXT NOT NULL REFERENCES personas(id),
    scheduled_at INTEGER NOT NULL,
    params_json  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scheduled_due ON scheduled_statuses(scheduled_at);

CREATE TABLE IF NOT EXISTS markers (
    user_id      TEXT NOT NULL REFERENCES users(id),
    timeline     TEXT NOT NULL,
    last_read_id TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 0,
    updated_at   INTEGER NOT NULL,
    UNIQUE (user_id, timeline)
);

CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id              INTEGER PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id),
    credential_json TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS webauthn_challenges (
    challenge_id TEXT PRIMARY KEY,
    state_json   TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS also_known_as (
    persona_id TEXT NOT NULL REFERENCES personas(id),
    uri        TEXT NOT NULL,
    UNIQUE (persona_id, uri)
);

CREATE TABLE IF NOT EXISTS conversation_read_markers (
    user_id TEXT NOT NULL REFERENCES users(id),
    post_id INTEGER NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS conversation_hidden (
    user_id TEXT NOT NULL REFERENCES users(id),
    post_id INTEGER NOT NULL REFERENCES posts(id),
    UNIQUE (user_id, post_id)
);

CREATE TABLE IF NOT EXISTS feed_state (
    feed_url     TEXT PRIMARY KEY,
    persona_id   TEXT NOT NULL REFERENCES personas(id),
    last_seen_id TEXT,
    last_poll    INTEGER
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "db-sqlite")]
    async fn fresh_sqlite() -> (Pool, sqlx::SqlitePool) {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        (Pool::Sqlite(pool.clone()), pool)
    }

    #[cfg(feature = "db-sqlite")]
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

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn fresh_db_reaches_current_version() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn migrate_is_idempotent() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();
        // Running again should be a no-op
        migrate(&db).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn migration_v1_adds_did_columns() {
        let (db, pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();

        // Verify did:scid columns exist by inserting a row that uses them
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at, did_scid) \
             VALUES ('test-uuid', 'test@example.com', 'Test', 'user', 1000, 'did:scid:sha256:abc')"
        ).execute(&pool).await.unwrap();

        let row: (String,) = sqlx::query_as(
            "SELECT did_scid FROM users WHERE id = 'test-uuid'"
        ).fetch_one(&pool).await.unwrap();

        assert_eq!(row.0, "did:scid:sha256:abc");
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn extra_migrations_run_in_order() {
        let extras = [Migration {
            version: 2,
            description: "test: add test_extra table",
            sql_sqlite: "CREATE TABLE IF NOT EXISTS test_extra (id INTEGER PRIMARY KEY, val TEXT);",
            sql_postgres: "CREATE TABLE IF NOT EXISTS test_extra (id SERIAL PRIMARY KEY, val TEXT);",
        }];

        let (db, pool) = fresh_sqlite().await;
        migrate_with(&db, &extras).await.unwrap();

        let version = schema_version(&db).await.unwrap();
        assert_eq!(version, 2);

        // Extra table should exist
        let exists = table_exists(&db, "test_extra").await.unwrap();
        assert!(exists);

        // DID columns from v1 should also exist
        let row: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'did_scid'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn detect_empty_db() {
        let (db, _pool) = fresh_sqlite().await;
        ensure_version_table(&db).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, None);
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn detect_canonical_schema() {
        let (db, _pool) = fresh_sqlite().await;
        migrate(&db).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("canonical"));
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn detect_smallhold_schema() {
        let (db, pool) = fresh_sqlite().await;
        // Simulate smallhold: has accounts table, no users or personas
        sqlx::raw_sql("CREATE TABLE accounts (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("smallhold"));
    }

    #[cfg(feature = "db-sqlite")]
    #[tokio::test]
    async fn detect_broadside_schema() {
        let (db, pool) = fresh_sqlite().await;
        // Simulate broadside: has personas but no users
        sqlx::raw_sql("CREATE TABLE personas (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        let legacy = detect_legacy_schema(&db).await.unwrap();
        assert_eq!(legacy, Some("broadside"));
    }

    #[cfg(feature = "db-sqlite")]
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
