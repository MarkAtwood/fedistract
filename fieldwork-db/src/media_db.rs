//! Database persistence for media attachments.
//!
//! Pure CRUD operations against the canonical `media` table defined in [`crate::db`].
//! Each function dispatches on [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;

/// A row in the `media` table.
pub struct MediaRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub post_id: Option<i64>,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub blurhash: Option<String>,
    pub integrity: Option<String>,
    pub description: String,
    pub created_at: i64,
}

/// Insert a media row.
pub async fn insert_media(pool: &Pool, media: &MediaRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, \
                 file_size, width, height, blurhash, integrity, description, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            )
            .bind(media.id)
            .bind(&media.user_id)
            .bind(&media.persona_id)
            .bind(media.post_id)
            .bind(&media.file_path)
            .bind(&media.mime_type)
            .bind(media.file_size)
            .bind(media.width)
            .bind(media.height)
            .bind(&media.blurhash)
            .bind(&media.integrity)
            .bind(&media.description)
            .bind(media.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, \
                 file_size, width, height, blurhash, integrity, description, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )
            .bind(media.id)
            .bind(&media.user_id)
            .bind(&media.persona_id)
            .bind(media.post_id)
            .bind(&media.file_path)
            .bind(&media.mime_type)
            .bind(media.file_size)
            .bind(media.width)
            .bind(media.height)
            .bind(&media.blurhash)
            .bind(&media.integrity)
            .bind(&media.description)
            .bind(media.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

const SELECT_COLS_PG: &str = "id, user_id, persona_id, post_id, file_path, mime_type, \
    file_size, width, height, blurhash, integrity, description, created_at";
const SELECT_COLS_SQ: &str = "id, user_id, persona_id, post_id, file_path, mime_type, \
    file_size, width, height, blurhash, integrity, description, created_at";

type MediaTuple = (
    i64, i64, i64, Option<i64>, String, String,
    i64, Option<i32>, Option<i32>, Option<String>, Option<String>, String, i64,
);

/// Look up a single media row by ID.
pub async fn get_media(pool: &Pool, id: i64) -> Result<Option<MediaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query_as::<_, MediaTuple>(
                &format!("SELECT {SELECT_COLS_PG} FROM media WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(row_to_media))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query_as::<_, MediaTuple>(
                &format!("SELECT {SELECT_COLS_SQ} FROM media WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(row_to_media))
        }
    }
}

/// Fetch all media attachments for a given post, ordered by ID.
pub async fn attachments_for_post(
    pool: &Pool,
    post_id: i64,
) -> Result<Vec<MediaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query_as::<_, MediaTuple>(
                &format!("SELECT {SELECT_COLS_PG} FROM media WHERE post_id = $1 ORDER BY id"),
            )
            .bind(post_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(row_to_media).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query_as::<_, MediaTuple>(
                &format!("SELECT {SELECT_COLS_SQ} FROM media WHERE post_id = ?1 ORDER BY id"),
            )
            .bind(post_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(row_to_media).collect())
        }
    }
}

/// List image media for a persona, ordered by newest first.
/// Only returns image/* MIME types for photo grid display.
pub async fn list_images_for_persona(
    pool: &Pool,
    persona_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<MediaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<MediaTuple> = sqlx::query_as(&format!(
                "SELECT {SELECT_COLS_PG} FROM media \
                 WHERE persona_id = $1 AND mime_type LIKE 'image/%' \
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            ))
            .bind(persona_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(row_to_media).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<MediaTuple> = sqlx::query_as(&format!(
                "SELECT {SELECT_COLS_SQ} FROM media \
                 WHERE persona_id = ?1 AND mime_type LIKE 'image/%' \
                 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
            ))
            .bind(persona_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(row_to_media).collect())
        }
    }
}

/// Delete a media row by ID.
pub async fn delete_media(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM media WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM media WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

fn row_to_media(r: MediaTuple) -> MediaRow {
    MediaRow {
        id: r.0,
        user_id: r.1,
        persona_id: r.2,
        post_id: r.3,
        file_path: r.4,
        mime_type: r.5,
        file_size: r.6,
        width: r.7,
        height: r.8,
        blurhash: r.9,
        integrity: r.10,
        description: r.11,
        created_at: r.12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sqlite")]
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

    #[cfg(feature = "sqlite")]
    async fn seed_parents(pool: &Pool) {
        let sql = "\
            INSERT INTO users (id, email, display_name, role, created_at) \
            VALUES (1, 'test@example.com', 'Test', 'user', 1000);\
            INSERT INTO personas (id, user_id, username, display_name, private_key_pem, public_key_pem, created_at) \
            VALUES (101, 1, 'alice', 'Alice', 'pk', 'pub', 1000);\
            INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
            VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000);";
        match pool {
            #[cfg(feature = "postgres")]
            Pool::Postgres(_) => unreachable!(),
            #[cfg(feature = "sqlite")]
            Pool::Sqlite(sq) => {
                sqlx::raw_sql(sql).execute(sq).await.unwrap();
            }
        }
    }

    fn sample_media(id: i64, post_id: Option<i64>) -> MediaRow {
        MediaRow {
            id,
            user_id: 1,
            persona_id: 101,
            post_id,
            file_path: format!("media/u1/{id}.png"),
            mime_type: "image/png".into(),
            file_size: 12345,
            width: Some(800),
            height: Some(600),
            blurhash: Some("LEHV6nWB2yk8pyo0adR*.7kCMdnj".into()),
            integrity: None,
            description: "a test image".into(),
            created_at: 1000,
        }
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn insert_and_get() {
        let pool = setup_db().await;
        seed_parents(&pool).await;

        let media = sample_media(1, None);
        insert_media(&pool, &media).await.unwrap();

        let fetched = get_media(&pool, 1).await.unwrap().expect("row should exist");
        assert_eq!(fetched.id, 1);
        assert_eq!(fetched.file_path, "media/u1/1.png");
        assert!(fetched.integrity.is_none());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn insert_with_integrity() {
        let pool = setup_db().await;
        seed_parents(&pool).await;

        let mut media = sample_media(2, None);
        media.integrity = Some("sha256-abc123def456".into());
        insert_media(&pool, &media).await.unwrap();

        let fetched = get_media(&pool, 2).await.unwrap().expect("row should exist");
        assert_eq!(fetched.integrity.as_deref(), Some("sha256-abc123def456"));
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup_db().await;
        let fetched = get_media(&pool, 999).await.unwrap();
        assert!(fetched.is_none());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn attachments_for_post_returns_ordered() {
        let pool = setup_db().await;
        seed_parents(&pool).await;

        let m1 = sample_media(10, Some(100));
        let m2 = sample_media(20, Some(100));
        let m3 = sample_media(5, Some(100));
        insert_media(&pool, &m1).await.unwrap();
        insert_media(&pool, &m2).await.unwrap();
        insert_media(&pool, &m3).await.unwrap();

        let rows = attachments_for_post(&pool, 100).await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, 5);
        assert_eq!(rows[1].id, 10);
        assert_eq!(rows[2].id, 20);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn attachments_for_post_empty() {
        let pool = setup_db().await;
        let rows = attachments_for_post(&pool, 999).await.unwrap();
        assert!(rows.is_empty());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn delete_removes_row() {
        let pool = setup_db().await;
        seed_parents(&pool).await;

        let media = sample_media(1, None);
        insert_media(&pool, &media).await.unwrap();
        assert!(get_media(&pool, 1).await.unwrap().is_some());

        delete_media(&pool, 1).await.unwrap();
        assert!(get_media(&pool, 1).await.unwrap().is_none());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn delete_nonexistent_is_ok() {
        let pool = setup_db().await;
        delete_media(&pool, 999).await.unwrap();
    }
}
