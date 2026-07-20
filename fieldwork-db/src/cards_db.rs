//! Database persistence for link preview cards.
//!
//! Provides CRUD operations for the `link_cards` and `post_cards` tables.
//! Each function dispatches on [`db::Pool`] variant to support both SQLite
//! and PostgreSQL.

use crate::db;

/// A row from the `link_cards` table.
#[derive(Debug, Clone)]
pub struct CardRow {
    pub id: i64,
    pub url: String,
    pub card_type: String,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub author_name: String,
    pub author_url: String,
    pub provider_name: String,
    pub provider_url: String,
    pub html: String,
    pub width: i32,
    pub height: i32,
    pub fetched_at: i64,
    pub failed: bool,
}

/// Insert or update a card. Keyed on `url` (UNIQUE).
///
/// For SQLite: `INSERT OR REPLACE`. For Postgres: `ON CONFLICT (url) DO UPDATE`.
/// The `id` field on the input is ignored — the database assigns it.
pub async fn upsert_card(pool: &db::Pool, card: &CardRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        db::Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO link_cards \
                 (url, card_type, title, description, image_url, author_name, author_url, \
                  provider_name, provider_url, html, width, height, fetched_at, failed) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
                 ON CONFLICT (url) DO UPDATE SET \
                 card_type = EXCLUDED.card_type, title = EXCLUDED.title, \
                 description = EXCLUDED.description, image_url = EXCLUDED.image_url, \
                 author_name = EXCLUDED.author_name, author_url = EXCLUDED.author_url, \
                 provider_name = EXCLUDED.provider_name, provider_url = EXCLUDED.provider_url, \
                 html = EXCLUDED.html, width = EXCLUDED.width, height = EXCLUDED.height, \
                 fetched_at = EXCLUDED.fetched_at, failed = EXCLUDED.failed",
            )
            .bind(&card.url)
            .bind(&card.card_type)
            .bind(&card.title)
            .bind(&card.description)
            .bind(&card.image_url)
            .bind(&card.author_name)
            .bind(&card.author_url)
            .bind(&card.provider_name)
            .bind(&card.provider_url)
            .bind(&card.html)
            .bind(card.width)
            .bind(card.height)
            .bind(card.fetched_at)
            .bind(card.failed)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        db::Pool::Sqlite(sq) => {
            // SQLite stores booleans as integers.
            let failed_int: i32 = if card.failed { 1 } else { 0 };
            sqlx::query(
                "INSERT OR REPLACE INTO link_cards \
                 (url, card_type, title, description, image_url, author_name, author_url, \
                  provider_name, provider_url, html, width, height, fetched_at, failed) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )
            .bind(&card.url)
            .bind(&card.card_type)
            .bind(&card.title)
            .bind(&card.description)
            .bind(&card.image_url)
            .bind(&card.author_name)
            .bind(&card.author_url)
            .bind(&card.provider_name)
            .bind(&card.provider_url)
            .bind(&card.html)
            .bind(card.width)
            .bind(card.height)
            .bind(card.fetched_at)
            .bind(failed_int)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Look up a card by its URL. Returns `None` if not found.
pub async fn get_card_by_url(
    pool: &db::Pool,
    url: &str,
) -> Result<Option<CardRow>, sqlx::Error> {
    type Row = (
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        i32,
        i32,
        i64,
        i32,
    );

    let row: Option<Row> = match pool {
        #[cfg(feature = "postgres")]
        db::Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, url, card_type, title, description, image_url, \
                 author_name, author_url, provider_name, provider_url, \
                 html, width, height, fetched_at, failed::int \
                 FROM link_cards WHERE url = $1",
            )
            .bind(url)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        db::Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, url, card_type, title, description, image_url, \
                 author_name, author_url, provider_name, provider_url, \
                 html, width, height, fetched_at, failed \
                 FROM link_cards WHERE url = ?1",
            )
            .bind(url)
            .fetch_optional(sq)
            .await?
        }
    };

    Ok(row.map(row_to_card))
}

/// Insert a row into the `post_cards` junction table, linking a post to a card URL.
/// Silently ignores duplicates.
pub async fn link_card_to_post(
    pool: &db::Pool,
    post_id: i64,
    card_url: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        db::Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO post_cards (post_id, card_url) \
                 VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(post_id)
            .bind(card_url)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        db::Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO post_cards (post_id, card_url) VALUES (?1, ?2)",
            )
            .bind(post_id)
            .bind(card_url)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Load all non-failed cards linked to a post via the `post_cards` junction.
pub async fn cards_for_post(
    pool: &db::Pool,
    post_id: i64,
) -> Result<Vec<CardRow>, sqlx::Error> {
    type Row = (
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        i32,
        i32,
        i64,
        i32,
    );

    let rows: Vec<Row> = match pool {
        #[cfg(feature = "postgres")]
        db::Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT lc.id, lc.url, lc.card_type, lc.title, lc.description, lc.image_url, \
                 lc.author_name, lc.author_url, lc.provider_name, lc.provider_url, \
                 lc.html, lc.width, lc.height, lc.fetched_at, lc.failed::int \
                 FROM post_cards pc \
                 JOIN link_cards lc ON pc.card_url = lc.url \
                 WHERE pc.post_id = $1 AND lc.failed = false",
            )
            .bind(post_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        db::Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT lc.id, lc.url, lc.card_type, lc.title, lc.description, lc.image_url, \
                 lc.author_name, lc.author_url, lc.provider_name, lc.provider_url, \
                 lc.html, lc.width, lc.height, lc.fetched_at, lc.failed \
                 FROM post_cards pc \
                 JOIN link_cards lc ON pc.card_url = lc.url \
                 WHERE pc.post_id = ?1 AND lc.failed = 0",
            )
            .bind(post_id)
            .fetch_all(sq)
            .await?
        }
    };

    Ok(rows.into_iter().map(row_to_card).collect())
}

/// Check whether a URL was marked as failed within the given time window.
///
/// `max_age_secs` is compared against `fetched_at` (epoch seconds).
/// Returns `true` if the URL has `failed = true` and was fetched less than
/// `max_age_secs` seconds ago.
pub async fn is_recent_failure(
    pool: &db::Pool,
    url: &str,
    max_age_secs: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = match pool {
        #[cfg(feature = "postgres")]
        db::Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT fetched_at FROM link_cards \
                 WHERE url = $1 AND failed = true",
            )
            .bind(url)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        db::Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT fetched_at FROM link_cards \
                 WHERE url = ?1 AND failed = 1",
            )
            .bind(url)
            .fetch_optional(sq)
            .await?
        }
    };

    match row {
        Some((fetched_at,)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            Ok((now - fetched_at) < max_age_secs)
        }
        None => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn row_to_card(
    row: (
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        i32,
        i32,
        i64,
        i32,
    ),
) -> CardRow {
    CardRow {
        id: row.0,
        url: row.1,
        card_type: row.2,
        title: row.3,
        description: row.4,
        image_url: row.5,
        author_name: row.6,
        author_url: row.7,
        provider_name: row.8,
        provider_url: row.9,
        html: row.10,
        width: row.11,
        height: row.12,
        fetched_at: row.13,
        failed: row.14 != 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
    use super::*;

    async fn setup() -> db::Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let db_pool = db::Pool::Sqlite(pool);
        db::migrate(&db_pool).await.unwrap();
        db_pool
    }

    fn sample_card(url: &str) -> CardRow {
        CardRow {
            id: 0,
            url: url.to_string(),
            card_type: "link".to_string(),
            title: "Example".to_string(),
            description: "An example page".to_string(),
            image_url: Some("https://example.com/img.png".to_string()),
            author_name: "Alice".to_string(),
            author_url: "https://example.com/@alice".to_string(),
            provider_name: "Example Corp".to_string(),
            provider_url: "https://example.com".to_string(),
            html: String::new(),
            width: 800,
            height: 600,
            fetched_at: 1_700_000_000,
            failed: false,
        }
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let pool = setup().await;
        let card = sample_card("https://example.com/page");

        upsert_card(&pool, &card).await.unwrap();

        let loaded = get_card_by_url(&pool, "https://example.com/page")
            .await
            .unwrap()
            .expect("card should exist");

        assert_eq!(loaded.url, "https://example.com/page");
        assert_eq!(loaded.title, "Example");
        assert_eq!(loaded.description, "An example page");
        assert_eq!(loaded.image_url.as_deref(), Some("https://example.com/img.png"));
        assert_eq!(loaded.author_name, "Alice");
        assert_eq!(loaded.width, 800);
        assert_eq!(loaded.height, 600);
        assert!(!loaded.failed);
        assert!(loaded.id > 0);
    }

    #[tokio::test]
    async fn upsert_replaces_on_conflict() {
        let pool = setup().await;
        let mut card = sample_card("https://example.com/page");
        upsert_card(&pool, &card).await.unwrap();

        card.title = "Updated Title".to_string();
        card.fetched_at = 1_700_001_000;
        upsert_card(&pool, &card).await.unwrap();

        let loaded = get_card_by_url(&pool, "https://example.com/page")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.title, "Updated Title");
        assert_eq!(loaded.fetched_at, 1_700_001_000);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        let result = get_card_by_url(&pool, "https://nope.example.com")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn link_and_load_cards_for_post() {
        let pool = setup().await;

        // Create a minimal post so the FK is satisfied.
        // Requires users and personas tables to exist (they do from migrate).
        let sq = match &pool {
            db::Pool::Sqlite(sq) => sq,
            #[cfg(feature = "postgres")]
            _ => unreachable!(),
        };
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'a@b.com', 'A', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (42, 1, 101, 'https://x.com/posts/42', 'hi', '<p>hi</p>', 'public', 1000);",
        )
        .execute(sq)
        .await
        .unwrap();

        let card = sample_card("https://example.com/page");
        upsert_card(&pool, &card).await.unwrap();
        link_card_to_post(&pool, 42, "https://example.com/page")
            .await
            .unwrap();

        let cards = cards_for_post(&pool, 42).await.unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].url, "https://example.com/page");
        assert_eq!(cards[0].title, "Example");
    }

    #[tokio::test]
    async fn link_card_to_post_is_idempotent() {
        let pool = setup().await;

        let sq = match &pool {
            db::Pool::Sqlite(sq) => sq,
            #[cfg(feature = "postgres")]
            _ => unreachable!(),
        };
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'a@b.com', 'A', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (42, 1, 101, 'https://x.com/posts/42', 'hi', '<p>hi</p>', 'public', 1000);",
        )
        .execute(sq)
        .await
        .unwrap();

        let card = sample_card("https://example.com/page");
        upsert_card(&pool, &card).await.unwrap();

        link_card_to_post(&pool, 42, "https://example.com/page").await.unwrap();
        link_card_to_post(&pool, 42, "https://example.com/page").await.unwrap();

        let cards = cards_for_post(&pool, 42).await.unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[tokio::test]
    async fn failed_cards_excluded_from_post_results() {
        let pool = setup().await;

        let sq = match &pool {
            db::Pool::Sqlite(sq) => sq,
            #[cfg(feature = "postgres")]
            _ => unreachable!(),
        };
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'a@b.com', 'A', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (42, 1, 101, 'https://x.com/posts/42', 'hi', '<p>hi</p>', 'public', 1000);",
        )
        .execute(sq)
        .await
        .unwrap();

        let mut card = sample_card("https://bad.example.com");
        card.failed = true;
        upsert_card(&pool, &card).await.unwrap();
        link_card_to_post(&pool, 42, "https://bad.example.com").await.unwrap();

        let cards = cards_for_post(&pool, 42).await.unwrap();
        assert!(cards.is_empty(), "failed cards should be excluded");
    }

    #[tokio::test]
    async fn is_recent_failure_within_window() {
        let pool = setup().await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut card = sample_card("https://fail.example.com");
        card.failed = true;
        card.fetched_at = now - 30; // 30 seconds ago

        upsert_card(&pool, &card).await.unwrap();

        let recent = is_recent_failure(&pool, "https://fail.example.com", 3600)
            .await
            .unwrap();
        assert!(recent, "failure 30s ago should be recent within 1h window");
    }

    #[tokio::test]
    async fn is_recent_failure_outside_window() {
        let pool = setup().await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut card = sample_card("https://fail.example.com");
        card.failed = true;
        card.fetched_at = now - 7200; // 2 hours ago

        upsert_card(&pool, &card).await.unwrap();

        let recent = is_recent_failure(&pool, "https://fail.example.com", 3600)
            .await
            .unwrap();
        assert!(!recent, "failure 2h ago should NOT be recent within 1h window");
    }

    #[tokio::test]
    async fn is_recent_failure_nonexistent_url() {
        let pool = setup().await;
        let recent = is_recent_failure(&pool, "https://nope.example.com", 3600)
            .await
            .unwrap();
        assert!(!recent);
    }

    #[tokio::test]
    async fn is_recent_failure_non_failed_card() {
        let pool = setup().await;

        let card = sample_card("https://ok.example.com");
        // card.failed is false by default
        upsert_card(&pool, &card).await.unwrap();

        let recent = is_recent_failure(&pool, "https://ok.example.com", 3600)
            .await
            .unwrap();
        assert!(!recent, "non-failed card should not register as failure");
    }
}
