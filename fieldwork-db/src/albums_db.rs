//! Database operations for photo albums / collections.
//!
//! Pure CRUD operations against the canonical `albums` and `album_items`
//! tables defined in [`crate::db`]. Each function dispatches on [`crate::db::Pool`]
//! to emit the correct SQL dialect.

use crate::db::Pool;

/// A row from the `albums` table.
#[derive(Debug, Clone)]
pub struct AlbumRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub title: String,
    pub description: String,
    pub visibility: String,
    pub cover_media_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

type AlbumTuple = (
    i64, i64, i64, String, String, String, Option<i64>, i64, Option<i64>,
);

fn tuple_to_album(r: AlbumTuple) -> AlbumRow {
    AlbumRow {
        id: r.0,
        user_id: r.1,
        persona_id: r.2,
        title: r.3,
        description: r.4,
        visibility: r.5,
        cover_media_id: r.6,
        created_at: r.7,
        updated_at: r.8,
    }
}

const COLS: &str = "id, user_id, persona_id, title, description, visibility, \
    cover_media_id, created_at, updated_at";

/// Create a new album.
pub async fn create_album(pool: &Pool, album: &AlbumRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO albums (id, user_id, persona_id, title, description, \
                 visibility, cover_media_id, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(album.id)
            .bind(album.user_id)
            .bind(album.persona_id)
            .bind(&album.title)
            .bind(&album.description)
            .bind(&album.visibility)
            .bind(album.cover_media_id)
            .bind(album.created_at)
            .bind(album.updated_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO albums (id, user_id, persona_id, title, description, \
                 visibility, cover_media_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(album.id)
            .bind(album.user_id)
            .bind(album.persona_id)
            .bind(&album.title)
            .bind(&album.description)
            .bind(&album.visibility)
            .bind(album.cover_media_id)
            .bind(album.created_at)
            .bind(album.updated_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get an album by ID.
pub async fn get_album(pool: &Pool, id: i64) -> Result<Option<AlbumRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<AlbumTuple> = sqlx::query_as(
                &format!("SELECT {COLS} FROM albums WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_album))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<AlbumTuple> = sqlx::query_as(
                &format!("SELECT {COLS} FROM albums WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_album))
        }
    }
}

/// List all albums for a persona, ordered by creation time descending.
pub async fn list_albums(pool: &Pool, persona_id: i64) -> Result<Vec<AlbumRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<AlbumTuple> = sqlx::query_as(
                &format!("SELECT {COLS} FROM albums WHERE persona_id = $1 ORDER BY created_at DESC"),
            )
            .bind(persona_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_album).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<AlbumTuple> = sqlx::query_as(
                &format!("SELECT {COLS} FROM albums WHERE persona_id = ?1 ORDER BY created_at DESC"),
            )
            .bind(persona_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_album).collect())
        }
    }
}

/// Update an album's title and/or description.
pub async fn update_album(
    pool: &Pool,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
) -> Result<(), sqlx::Error> {
    let current = get_album(pool, id).await?;
    let current = match current {
        Some(c) => c,
        None => return Ok(()),
    };
    let new_title = title.unwrap_or(&current.title);
    let new_desc = description.unwrap_or(&current.description);

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE albums SET title = $1, description = $2 WHERE id = $3",
            )
            .bind(new_title)
            .bind(new_desc)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE albums SET title = ?1, description = ?2 WHERE id = ?3",
            )
            .bind(new_title)
            .bind(new_desc)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Delete an album (album_items cascade-deleted by FK).
pub async fn delete_album(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM albums WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM albums WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Add a media item to an album. Silently replaces on duplicate.
pub async fn add_to_album(
    pool: &Pool,
    album_id: i64,
    media_id: i64,
    position: i32,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO album_items (album_id, media_id, position) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (album_id, media_id) DO UPDATE SET position = $3",
            )
            .bind(album_id)
            .bind(media_id)
            .bind(position)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO album_items (album_id, media_id, position) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT (album_id, media_id) DO UPDATE SET position = ?3",
            )
            .bind(album_id)
            .bind(media_id)
            .bind(position)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a media item from an album.
pub async fn remove_from_album(
    pool: &Pool,
    album_id: i64,
    media_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM album_items WHERE album_id = $1 AND media_id = $2",
            )
            .bind(album_id)
            .bind(media_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM album_items WHERE album_id = ?1 AND media_id = ?2",
            )
            .bind(album_id)
            .bind(media_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get media IDs in an album, ordered by position.
pub async fn album_items(pool: &Pool, album_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT media_id FROM album_items \
                 WHERE album_id = $1 ORDER BY position, media_id",
            )
            .bind(album_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT media_id FROM album_items \
                 WHERE album_id = ?1 ORDER BY position, media_id",
            )
            .bind(album_id)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|r| r.0).collect())
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> (Pool, sqlx::SqlitePool) {
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
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000); \
             INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, file_size, description, created_at) \
             VALUES (10, 1, 101, 100, 'media/10.png', 'image/png', 1234, 'img a', 1000); \
             INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, file_size, description, created_at) \
             VALUES (20, 1, 101, 100, 'media/20.png', 'image/png', 5678, 'img b', 1000); \
             INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, file_size, description, created_at) \
             VALUES (30, 1, 101, 100, 'media/30.png', 'image/png', 9012, 'img c', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    fn sample_album(id: i64) -> AlbumRow {
        AlbumRow {
            id,
            user_id: 1,
            persona_id: 101,
            title: "Vacation".into(),
            description: "Summer 2025".into(),
            visibility: "public".into(),
            cover_media_id: None,
            created_at: 2000,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let (pool, _) = setup().await;
        let album = sample_album(1);
        create_album(&pool, &album).await.unwrap();

        let fetched = get_album(&pool, 1).await.unwrap().expect("album should exist");
        assert_eq!(fetched.title, "Vacation");
        assert_eq!(fetched.description, "Summer 2025");
        assert_eq!(fetched.visibility, "public");
        assert!(fetched.cover_media_id.is_none());
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let (pool, _) = setup().await;
        assert!(get_album(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_albums_filters_by_persona() {
        let (pool, _) = setup().await;
        let mut a1 = sample_album(1);
        a1.created_at = 3000;
        let mut a2 = sample_album(2);
        a2.title = "Work".into();
        a2.created_at = 2000;
        create_album(&pool, &a1).await.unwrap();
        create_album(&pool, &a2).await.unwrap();

        let albums = list_albums(&pool, 101).await.unwrap();
        assert_eq!(albums.len(), 2);
        // Ordered by created_at DESC
        assert_eq!(albums[0].title, "Vacation");
        assert_eq!(albums[1].title, "Work");

        let empty = list_albums(&pool, 999).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn update_album_partial() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();

        update_album(&pool, 1, Some("New Title"), None).await.unwrap();
        let fetched = get_album(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.title, "New Title");
        assert_eq!(fetched.description, "Summer 2025");
    }

    #[tokio::test]
    async fn update_nonexistent_is_ok() {
        let (pool, _) = setup().await;
        update_album(&pool, 999, Some("X"), None).await.unwrap();
    }

    #[tokio::test]
    async fn delete_album_cascades_items() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();
        add_to_album(&pool, 1, 10, 0).await.unwrap();
        add_to_album(&pool, 1, 20, 1).await.unwrap();

        delete_album(&pool, 1).await.unwrap();
        assert!(get_album(&pool, 1).await.unwrap().is_none());
        let items = album_items(&pool, 1).await.unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_is_ok() {
        let (pool, _) = setup().await;
        delete_album(&pool, 999).await.unwrap();
    }

    #[tokio::test]
    async fn add_and_list_items_ordered() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();

        add_to_album(&pool, 1, 30, 0).await.unwrap();
        add_to_album(&pool, 1, 10, 1).await.unwrap();
        add_to_album(&pool, 1, 20, 2).await.unwrap();

        let items = album_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![30, 10, 20]);
    }

    #[tokio::test]
    async fn add_duplicate_updates_position() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();

        add_to_album(&pool, 1, 10, 0).await.unwrap();
        add_to_album(&pool, 1, 20, 1).await.unwrap();
        // Move media 10 to position 5
        add_to_album(&pool, 1, 10, 5).await.unwrap();

        let items = album_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![20, 10]); // 20 at pos 1, 10 at pos 5
    }

    #[tokio::test]
    async fn remove_from_album_works() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();
        add_to_album(&pool, 1, 10, 0).await.unwrap();
        add_to_album(&pool, 1, 20, 1).await.unwrap();

        remove_from_album(&pool, 1, 10).await.unwrap();
        let items = album_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![20]);
    }

    #[tokio::test]
    async fn remove_nonexistent_item_is_ok() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();
        remove_from_album(&pool, 1, 999).await.unwrap();
    }

    #[tokio::test]
    async fn album_items_empty() {
        let (pool, _) = setup().await;
        create_album(&pool, &sample_album(1)).await.unwrap();
        let items = album_items(&pool, 1).await.unwrap();
        assert!(items.is_empty());
    }
}
