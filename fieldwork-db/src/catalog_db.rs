//! Database operations for the generalized media catalog.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Media items
// ---------------------------------------------------------------------------

/// A row from the `media_items` table.
#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: i64,
    pub media_type: String,
    pub format: Option<String>,
    pub title: String,
    pub creator: String,
    pub description: String,
    pub cover_url: Option<String>,
    pub language: Option<String>,
    pub pages: Option<i32>,
    pub runtime_min: Option<i32>,
    pub issue: Option<String>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub tracks: Option<i32>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub published_year: Option<i32>,
    pub release_date: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
}

#[cfg(feature = "sqlite")]
fn row_to_media_item(r: &sqlx::sqlite::SqliteRow) -> MediaItem {
    use sqlx::Row;
    MediaItem {
        id: r.get(0),
        media_type: r.get(1),
        format: r.get(2),
        title: r.get(3),
        creator: r.get(4),
        description: r.get(5),
        cover_url: r.get(6),
        language: r.get(7),
        pages: r.get(8),
        runtime_min: r.get(9),
        issue: r.get(10),
        season: r.get(11),
        episode: r.get(12),
        tracks: r.get(13),
        publisher: r.get(14),
        label: r.get(15),
        published_year: r.get(16),
        release_date: r.get(17),
        source: r.get(18),
        created_at: r.get(19),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_media_item(r: &sqlx::postgres::PgRow) -> MediaItem {
    use sqlx::Row;
    MediaItem {
        id: r.get(0),
        media_type: r.get(1),
        format: r.get(2),
        title: r.get(3),
        creator: r.get(4),
        description: r.get(5),
        cover_url: r.get(6),
        language: r.get(7),
        pages: r.get(8),
        runtime_min: r.get(9),
        issue: r.get(10),
        season: r.get(11),
        episode: r.get(12),
        tracks: r.get(13),
        publisher: r.get(14),
        label: r.get(15),
        published_year: r.get(16),
        release_date: r.get(17),
        source: r.get(18),
        created_at: r.get(19),
    }
}

const MEDIA_ITEM_COLS: &str = "id, media_type, format, title, creator, description, \
    cover_url, language, pages, runtime_min, issue, season, episode, tracks, \
    publisher, label, published_year, release_date, source, created_at";

const MEDIA_ITEM_COLS_QUALIFIED: &str = "m.id, m.media_type, m.format, m.title, m.creator, \
    m.description, m.cover_url, m.language, m.pages, m.runtime_min, m.issue, m.season, \
    m.episode, m.tracks, m.publisher, m.label, m.published_year, m.release_date, \
    m.source, m.created_at";

/// Insert a new media item.
pub async fn create_media(pool: &Pool, item: &MediaItem) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO media_items (id, media_type, format, title, creator, description, \
                 cover_url, language, pages, runtime_min, issue, season, episode, tracks, \
                 publisher, label, published_year, release_date, source, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                 $15, $16, $17, $18, $19, $20)",
            )
            .bind(item.id)
            .bind(&item.media_type)
            .bind(&item.format)
            .bind(&item.title)
            .bind(&item.creator)
            .bind(&item.description)
            .bind(&item.cover_url)
            .bind(&item.language)
            .bind(item.pages)
            .bind(item.runtime_min)
            .bind(&item.issue)
            .bind(item.season)
            .bind(item.episode)
            .bind(item.tracks)
            .bind(&item.publisher)
            .bind(&item.label)
            .bind(item.published_year)
            .bind(&item.release_date)
            .bind(&item.source)
            .bind(item.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO media_items (id, media_type, format, title, creator, description, \
                 cover_url, language, pages, runtime_min, issue, season, episode, tracks, \
                 publisher, label, published_year, release_date, source, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
                 ?15, ?16, ?17, ?18, ?19, ?20)",
            )
            .bind(item.id)
            .bind(&item.media_type)
            .bind(&item.format)
            .bind(&item.title)
            .bind(&item.creator)
            .bind(&item.description)
            .bind(&item.cover_url)
            .bind(&item.language)
            .bind(item.pages)
            .bind(item.runtime_min)
            .bind(&item.issue)
            .bind(item.season)
            .bind(item.episode)
            .bind(item.tracks)
            .bind(&item.publisher)
            .bind(&item.label)
            .bind(item.published_year)
            .bind(&item.release_date)
            .bind(&item.source)
            .bind(item.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a media item by ID.
pub async fn get_media(pool: &Pool, id: i64) -> Result<Option<MediaItem>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(&format!(
                "SELECT {MEDIA_ITEM_COLS} FROM media_items WHERE id = $1"
            ))
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.as_ref().map(pg_row_to_media_item))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(&format!(
                "SELECT {MEDIA_ITEM_COLS} FROM media_items WHERE id = ?1"
            ))
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.as_ref().map(row_to_media_item))
        }
    }
}

/// Search media by title or creator (LIKE match), with optional media_type filter.
pub async fn search_media(
    pool: &Pool,
    query: &str,
    media_type: Option<&str>,
    limit: i64,
) -> Result<Vec<MediaItem>, sqlx::Error> {
    let pattern = format!("%{query}%");
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = if let Some(mt) = media_type {
                sqlx::query(&format!(
                    "SELECT {MEDIA_ITEM_COLS} FROM media_items \
                     WHERE (title LIKE $1 OR creator LIKE $1) AND media_type = $2 \
                     ORDER BY title LIMIT $3"
                ))
                .bind(&pattern)
                .bind(mt)
                .bind(limit)
                .fetch_all(pg)
                .await?
            } else {
                sqlx::query(&format!(
                    "SELECT {MEDIA_ITEM_COLS} FROM media_items \
                     WHERE title LIKE $1 OR creator LIKE $1 \
                     ORDER BY title LIMIT $2"
                ))
                .bind(&pattern)
                .bind(limit)
                .fetch_all(pg)
                .await?
            };
            Ok(rows.iter().map(pg_row_to_media_item).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = if let Some(mt) = media_type {
                sqlx::query(&format!(
                    "SELECT {MEDIA_ITEM_COLS} FROM media_items \
                     WHERE (title LIKE ?1 OR creator LIKE ?1) AND media_type = ?2 \
                     ORDER BY title LIMIT ?3"
                ))
                .bind(&pattern)
                .bind(mt)
                .bind(limit)
                .fetch_all(sq)
                .await?
            } else {
                sqlx::query(&format!(
                    "SELECT {MEDIA_ITEM_COLS} FROM media_items \
                     WHERE title LIKE ?1 OR creator LIKE ?1 \
                     ORDER BY title LIMIT ?2"
                ))
                .bind(&pattern)
                .bind(limit)
                .fetch_all(sq)
                .await?
            };
            Ok(rows.iter().map(row_to_media_item).collect())
        }
    }
}

/// Look up a media item by identifier (joins media_identifiers).
pub async fn get_media_by_identifier(
    pool: &Pool,
    id_type: &str,
    id_value: &str,
) -> Result<Option<MediaItem>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(&format!(
                "SELECT {MEDIA_ITEM_COLS_QUALIFIED} FROM media_items m \
                 JOIN media_identifiers i ON m.id = i.media_id \
                 WHERE i.id_type = $1 AND i.id_value = $2 LIMIT 1"
            ))
            .bind(id_type)
            .bind(id_value)
            .fetch_optional(pg)
            .await?;
            Ok(row.as_ref().map(pg_row_to_media_item))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(&format!(
                "SELECT {MEDIA_ITEM_COLS_QUALIFIED} FROM media_items m \
                 JOIN media_identifiers i ON m.id = i.media_id \
                 WHERE i.id_type = ?1 AND i.id_value = ?2 LIMIT 1"
            ))
            .bind(id_type)
            .bind(id_value)
            .fetch_optional(sq)
            .await?;
            Ok(row.as_ref().map(row_to_media_item))
        }
    }
}

/// Update media metadata (for hydration from external sources).
/// If `openlibrary_id` is `Some`, it is upserted into `media_identifiers`.
#[allow(clippy::too_many_arguments)]
pub async fn update_media_metadata(
    pool: &Pool,
    id: i64,
    title: &str,
    creator: &str,
    pages: Option<i32>,
    published_year: Option<i32>,
    cover_url: Option<&str>,
    openlibrary_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE media_items SET title = $2, creator = $3, pages = $4, \
                 published_year = $5, cover_url = $6 WHERE id = $1",
            )
            .bind(id)
            .bind(title)
            .bind(creator)
            .bind(pages)
            .bind(published_year)
            .bind(cover_url)
            .execute(pg)
            .await?;
            if let Some(ol_id) = openlibrary_id {
                sqlx::query(
                    "INSERT INTO media_identifiers (media_id, id_type, id_value) \
                     VALUES ($1, 'openlibrary', $2) \
                     ON CONFLICT (media_id, id_type, id_value) DO NOTHING",
                )
                .bind(id)
                .bind(ol_id)
                .execute(pg)
                .await?;
            }
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE media_items SET title = ?2, creator = ?3, pages = ?4, \
                 published_year = ?5, cover_url = ?6 WHERE id = ?1",
            )
            .bind(id)
            .bind(title)
            .bind(creator)
            .bind(pages)
            .bind(published_year)
            .bind(cover_url)
            .execute(sq)
            .await?;
            if let Some(ol_id) = openlibrary_id {
                sqlx::query(
                    "INSERT OR IGNORE INTO media_identifiers (media_id, id_type, id_value) \
                     VALUES (?1, 'openlibrary', ?2)",
                )
                .bind(id)
                .bind(ol_id)
                .execute(sq)
                .await?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Add an identifier to a media item (INSERT OR IGNORE).
pub async fn add_identifier(
    pool: &Pool,
    media_id: i64,
    id_type: &str,
    id_value: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO media_identifiers (media_id, id_type, id_value) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (media_id, id_type, id_value) DO NOTHING",
            )
            .bind(media_id)
            .bind(id_type)
            .bind(id_value)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR IGNORE INTO media_identifiers (media_id, id_type, id_value) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(media_id)
            .bind(id_type)
            .bind(id_value)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all identifiers for a media item. Returns (id_type, id_value) pairs.
pub async fn get_identifiers(
    pool: &Pool,
    media_id: i64,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT id_type, id_value FROM media_identifiers \
                 WHERE media_id = $1 ORDER BY id_type",
            )
            .bind(media_id)
            .fetch_all(pg)
            .await?;
            Ok(rows)
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT id_type, id_value FROM media_identifiers \
                 WHERE media_id = ?1 ORDER BY id_type",
            )
            .bind(media_id)
            .fetch_all(sq)
            .await?;
            Ok(rows)
        }
    }
}

// ---------------------------------------------------------------------------
// Shelves
// ---------------------------------------------------------------------------

/// Set or update the shelf status for a user+media pair (UPSERT).
pub async fn set_shelf_status(
    pool: &Pool,
    user_id: i64,
    media_id: i64,
    status: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO media_shelves (user_id, media_id, status, updated_at) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (user_id, media_id) DO UPDATE SET status = $3, updated_at = $4",
            )
            .bind(user_id)
            .bind(media_id)
            .bind(status)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO media_shelves (user_id, media_id, status, updated_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (user_id, media_id) DO UPDATE SET status = ?3, updated_at = ?4",
            )
            .bind(user_id)
            .bind(media_id)
            .bind(status)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get the shelf status for a user+media pair.
/// Returns (status, started_at, finished_at, rating, updated_at).
pub async fn get_shelf_status(
    pool: &Pool,
    user_id: i64,
    media_id: i64,
) -> Result<Option<(String, Option<i64>, Option<i64>, Option<i32>, i64)>, sqlx::Error> {
    let row: Option<(String, Option<i64>, Option<i64>, Option<i32>, i64)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT status, started_at, finished_at, rating, updated_at \
                 FROM media_shelves WHERE user_id = $1 AND media_id = $2",
            )
            .bind(user_id)
            .bind(media_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT status, started_at, finished_at, rating, updated_at \
                 FROM media_shelves WHERE user_id = ?1 AND media_id = ?2",
            )
            .bind(user_id)
            .bind(media_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row)
}

/// Rate a media item (updates existing shelf row).
pub async fn rate_media(
    pool: &Pool,
    user_id: i64,
    media_id: i64,
    rating: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE media_shelves SET rating = $1, updated_at = $2 \
                 WHERE user_id = $3 AND media_id = $4",
            )
            .bind(rating)
            .bind(now)
            .bind(user_id)
            .bind(media_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE media_shelves SET rating = ?1, updated_at = ?2 \
                 WHERE user_id = ?3 AND media_id = ?4",
            )
            .bind(rating)
            .bind(now)
            .bind(user_id)
            .bind(media_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// List media on a user's shelf by status (joins media_items).
/// Returns (media_id, title, creator, cover_url, rating).
pub async fn user_shelf(
    pool: &Pool,
    user_id: i64,
    status: &str,
    limit: i64,
) -> Result<Vec<(i64, String, String, Option<String>, Option<i32>)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, String, String, Option<String>, Option<i32>)> = sqlx::query_as(
                "SELECT m.id, m.title, m.creator, m.cover_url, s.rating \
                 FROM media_shelves s \
                 JOIN media_items m ON m.id = s.media_id \
                 WHERE s.user_id = $1 AND s.status = $2 \
                 ORDER BY s.updated_at DESC \
                 LIMIT $3",
            )
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows)
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, String, String, Option<String>, Option<i32>)> = sqlx::query_as(
                "SELECT m.id, m.title, m.creator, m.cover_url, s.rating \
                 FROM media_shelves s \
                 JOIN media_items m ON m.id = s.media_id \
                 WHERE s.user_id = ?1 AND s.status = ?2 \
                 ORDER BY s.updated_at DESC \
                 LIMIT ?3",
            )
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows)
        }
    }
}

/// Get shelf stats for a user: (backlog, in_progress, done) counts.
/// Counts across all media types using status group matching.
pub async fn shelf_stats(
    pool: &Pool,
    user_id: i64,
) -> Result<(i64, i64, i64), sqlx::Error> {
    let row: (i64, i64, i64) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT \
                 COUNT(CASE WHEN status IN ('to-read', 'to-watch', 'to-listen', 'backlog') THEN 1 END), \
                 COUNT(CASE WHEN status IN ('reading', 'watching', 'listening', 'in-progress') THEN 1 END), \
                 COUNT(CASE WHEN status IN ('read', 'watched', 'listened', 'done') THEN 1 END) \
                 FROM media_shelves WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT \
                 COUNT(CASE WHEN status IN ('to-read', 'to-watch', 'to-listen', 'backlog') THEN 1 END), \
                 COUNT(CASE WHEN status IN ('reading', 'watching', 'listening', 'in-progress') THEN 1 END), \
                 COUNT(CASE WHEN status IN ('read', 'watched', 'listened', 'done') THEN 1 END) \
                 FROM media_shelves WHERE user_id = ?1",
            )
            .bind(user_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row)
}

// ---------------------------------------------------------------------------
// Reviews
// ---------------------------------------------------------------------------

/// A row from the `media_reviews` table.
#[derive(Debug, Clone)]
pub struct MediaReview {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub media_id: i64,
    pub content: String,
    pub content_html: String,
    pub rating: Option<i32>,
    pub spoiler: bool,
    pub ap_id: String,
    pub created_at: i64,
}

type ReviewTuple = (
    i64,
    i64,
    i64,
    i64,
    String,
    String,
    Option<i32>,
    i32,
    String,
    i64,
);

fn tuple_to_review(r: ReviewTuple) -> MediaReview {
    MediaReview {
        id: r.0,
        user_id: r.1,
        persona_id: r.2,
        media_id: r.3,
        content: r.4,
        content_html: r.5,
        rating: r.6,
        spoiler: r.7 != 0,
        ap_id: r.8,
        created_at: r.9,
    }
}

const REVIEW_COLS: &str = "id, user_id, persona_id, media_id, content, content_html, \
    rating, spoiler, ap_id, created_at";

/// Insert a new media review.
pub async fn create_review(pool: &Pool, review: &MediaReview) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO media_reviews (id, user_id, persona_id, media_id, content, \
                 content_html, rating, spoiler, ap_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(review.id)
            .bind(review.user_id)
            .bind(review.persona_id)
            .bind(review.media_id)
            .bind(&review.content)
            .bind(&review.content_html)
            .bind(review.rating)
            .bind(review.spoiler)
            .bind(&review.ap_id)
            .bind(review.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let spoiler_int: i32 = if review.spoiler { 1 } else { 0 };
            sqlx::query(
                "INSERT INTO media_reviews (id, user_id, persona_id, media_id, content, \
                 content_html, rating, spoiler, ap_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(review.id)
            .bind(review.user_id)
            .bind(review.persona_id)
            .bind(review.media_id)
            .bind(&review.content)
            .bind(&review.content_html)
            .bind(review.rating)
            .bind(spoiler_int)
            .bind(&review.ap_id)
            .bind(review.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a review by ID.
pub async fn get_review(pool: &Pool, id: i64) -> Result<Option<MediaReview>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews WHERE id = $1"
            ))
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_review))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews WHERE id = ?1"
            ))
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_review))
        }
    }
}

/// Get reviews for a media item, ordered by newest first.
pub async fn reviews_for_media(
    pool: &Pool,
    media_id: i64,
    limit: i64,
) -> Result<Vec<MediaReview>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews \
                 WHERE media_id = $1 ORDER BY created_at DESC LIMIT $2"
            ))
            .bind(media_id)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews \
                 WHERE media_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ))
            .bind(media_id)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
    }
}

/// Get reviews by a user, ordered by newest first.
pub async fn reviews_by_user(
    pool: &Pool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<MediaReview>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews \
                 WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"
            ))
            .bind(user_id)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM media_reviews \
                 WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ))
            .bind(user_id)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// API type conversions (feature: api-convert)
// ---------------------------------------------------------------------------

#[cfg(feature = "api-convert")]
impl From<&MediaItem> for fieldwork::bookwyrm_api::BookResponse {
    fn from(m: &MediaItem) -> Self {
        Self {
            id: m.id,
            title: m.title.clone(),
            author: m.creator.clone(),
            isbn: None,
            isbn13: None,
            openlibrary_id: None,
            cover_url: m.cover_url.clone(),
            description: m.description.clone(),
            pages: m.pages,
            published_year: m.published_year,
            language: m.language.clone(),
            created_at: m.created_at,
        }
    }
}

#[cfg(feature = "api-convert")]
impl From<&MediaReview> for fieldwork::bookwyrm_api::ReviewResponse {
    fn from(r: &MediaReview) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            persona_id: r.persona_id,
            book_id: r.media_id,
            content: r.content.clone(),
            content_html: r.content_html.clone(),
            rating: r.rating,
            spoiler: r.spoiler,
            ap_id: r.ap_id.clone(),
            created_at: r.created_at,
        }
    }
}
