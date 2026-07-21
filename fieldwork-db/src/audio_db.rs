//! Database operations for Funkwhale-style audio hosting and podcasts.
//!
//! Audio channels are AP actors backed by a persona (same pattern as
//! video channels and communities). This module provides CRUD for channels,
//! tracks, playlists, podcast series, and podcast episodes.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A row from the `audio_channels` table.
#[derive(Debug, Clone)]
pub struct AudioChannelRow {
    pub persona_id: i64,
    pub description: String,
    pub category: String,
    pub language: Option<String>,
    pub rss_enabled: bool,
    pub itunes_category: Option<String>,
    pub created_at: i64,
}

type ChannelTuple = (i64, String, String, Option<String>, bool, Option<String>, i64);

fn tuple_to_channel(t: ChannelTuple) -> AudioChannelRow {
    AudioChannelRow {
        persona_id: t.0,
        description: t.1,
        category: t.2,
        language: t.3,
        rss_enabled: t.4,
        itunes_category: t.5,
        created_at: t.6,
    }
}

const CHANNEL_COLS: &str = "persona_id, description, category, language, rss_enabled, itunes_category, created_at";

/// A row from the `tracks` table.
#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: i64,
    pub channel_id: i64,
    pub user_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<i32>,
    pub duration: Option<i64>,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub cover_media_id: Option<i64>,
    pub description: String,
    pub ap_id: String,
    pub visibility: String,
    pub created_at: i64,
}

const TRACK_COLS: &str = "\
    id, channel_id, user_id, title, artist, album, \
    track_number, duration, file_path, file_size, mime_type, \
    cover_media_id, description, ap_id, visibility, created_at";

#[cfg(feature = "sqlite")]
fn sqlite_row_to_track(r: sqlx::sqlite::SqliteRow) -> TrackRow {
    use sqlx::Row;
    TrackRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        artist: r.get(4),
        album: r.get(5),
        track_number: r.get(6),
        duration: r.get(7),
        file_path: r.get(8),
        file_size: r.get(9),
        mime_type: r.get(10),
        cover_media_id: r.get(11),
        description: r.get(12),
        ap_id: r.get(13),
        visibility: r.get(14),
        created_at: r.get(15),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_track(r: sqlx::postgres::PgRow) -> TrackRow {
    use sqlx::Row;
    TrackRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        artist: r.get(4),
        album: r.get(5),
        track_number: r.get(6),
        duration: r.get(7),
        file_path: r.get(8),
        file_size: r.get(9),
        mime_type: r.get(10),
        cover_media_id: r.get(11),
        description: r.get(12),
        ap_id: r.get(13),
        visibility: r.get(14),
        created_at: r.get(15),
    }
}

/// A row from the `podcast_series` table.
#[derive(Debug, Clone)]
pub struct PodcastSeriesRow {
    pub id: i64,
    pub channel_id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: String,
    pub description_html: String,
    pub author: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub itunes_category: Option<String>,
    pub cover_media_id: Option<i64>,
    pub explicit: bool,
    pub rss_link: Option<String>,
    pub created_at: i64,
}

const SERIES_COLS: &str = "\
    id, channel_id, user_id, title, description, description_html, \
    author, language, category, itunes_category, \
    cover_media_id, explicit, rss_link, created_at";

#[cfg(feature = "sqlite")]
fn sqlite_row_to_series(r: sqlx::sqlite::SqliteRow) -> PodcastSeriesRow {
    use sqlx::Row;
    PodcastSeriesRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        description: r.get(4),
        description_html: r.get(5),
        author: r.get(6),
        language: r.get(7),
        category: r.get(8),
        itunes_category: r.get(9),
        cover_media_id: r.get(10),
        explicit: r.get(11),
        rss_link: r.get(12),
        created_at: r.get(13),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_series(r: sqlx::postgres::PgRow) -> PodcastSeriesRow {
    use sqlx::Row;
    PodcastSeriesRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        description: r.get(4),
        description_html: r.get(5),
        author: r.get(6),
        language: r.get(7),
        category: r.get(8),
        itunes_category: r.get(9),
        cover_media_id: r.get(10),
        explicit: r.get(11),
        rss_link: r.get(12),
        created_at: r.get(13),
    }
}

/// A row from the `podcast_episodes` table.
#[derive(Debug, Clone)]
pub struct PodcastEpisodeRow {
    pub id: i64,
    pub series_id: i64,
    pub title: String,
    pub description: String,
    pub description_html: String,
    pub duration: Option<i64>,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub episode_number: Option<i32>,
    pub season_number: Option<i32>,
    pub explicit: bool,
    pub ap_id: String,
    pub published_at: i64,
    pub created_at: i64,
}

const EPISODE_COLS: &str = "\
    id, series_id, title, description, description_html, \
    duration, file_path, file_size, mime_type, \
    episode_number, season_number, explicit, ap_id, \
    published_at, created_at";

#[cfg(feature = "sqlite")]
fn sqlite_row_to_episode(r: sqlx::sqlite::SqliteRow) -> PodcastEpisodeRow {
    use sqlx::Row;
    PodcastEpisodeRow {
        id: r.get(0),
        series_id: r.get(1),
        title: r.get(2),
        description: r.get(3),
        description_html: r.get(4),
        duration: r.get(5),
        file_path: r.get(6),
        file_size: r.get(7),
        mime_type: r.get(8),
        episode_number: r.get(9),
        season_number: r.get(10),
        explicit: r.get(11),
        ap_id: r.get(12),
        published_at: r.get(13),
        created_at: r.get(14),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_episode(r: sqlx::postgres::PgRow) -> PodcastEpisodeRow {
    use sqlx::Row;
    PodcastEpisodeRow {
        id: r.get(0),
        series_id: r.get(1),
        title: r.get(2),
        description: r.get(3),
        description_html: r.get(4),
        duration: r.get(5),
        file_path: r.get(6),
        file_size: r.get(7),
        mime_type: r.get(8),
        episode_number: r.get(9),
        season_number: r.get(10),
        explicit: r.get(11),
        ap_id: r.get(12),
        published_at: r.get(13),
        created_at: r.get(14),
    }
}

// ---------------------------------------------------------------------------
// Audio Channel CRUD
// ---------------------------------------------------------------------------

/// Create an audio channel (persona must already exist).
pub async fn create_audio_channel(pool: &Pool, row: &AudioChannelRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO audio_channels (persona_id, description, category, language, rss_enabled, itunes_category, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(row.persona_id)
            .bind(&row.description)
            .bind(&row.category)
            .bind(&row.language)
            .bind(row.rss_enabled)
            .bind(&row.itunes_category)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO audio_channels (persona_id, description, category, language, rss_enabled, itunes_category, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(row.persona_id)
            .bind(&row.description)
            .bind(&row.category)
            .bind(&row.language)
            .bind(row.rss_enabled)
            .bind(&row.itunes_category)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get an audio channel by persona_id.
pub async fn get_audio_channel(pool: &Pool, persona_id: i64) -> Result<Option<AudioChannelRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM audio_channels WHERE persona_id = $1"),
            )
            .bind(persona_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_channel))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM audio_channels WHERE persona_id = ?1"),
            )
            .bind(persona_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_channel))
        }
    }
}

/// List audio channels owned by a user (via personas.user_id join).
pub async fn list_audio_channels(pool: &Pool, user_id: i64) -> Result<Vec<AudioChannelRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ChannelTuple> = sqlx::query_as(
                &format!(
                    "SELECT ac.{} FROM audio_channels ac \
                     JOIN personas p ON p.id = ac.persona_id \
                     WHERE p.user_id = $1 ORDER BY ac.created_at DESC",
                    CHANNEL_COLS.replace(", ", ", ac.")
                ),
            )
            .bind(user_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_channel).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ChannelTuple> = sqlx::query_as(
                &format!(
                    "SELECT ac.{} FROM audio_channels ac \
                     JOIN personas p ON p.id = ac.persona_id \
                     WHERE p.user_id = ?1 ORDER BY ac.created_at DESC",
                    CHANNEL_COLS.replace(", ", ", ac.")
                ),
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_channel).collect())
        }
    }
}

/// List all audio channels (public listing).
pub async fn list_all_audio_channels(pool: &Pool, limit: i64, offset: i64) -> Result<Vec<AudioChannelRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM audio_channels ORDER BY created_at DESC LIMIT $1 OFFSET $2"),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_channel).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM audio_channels ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_channel).collect())
        }
    }
}

/// Delete an audio channel.
pub async fn delete_audio_channel(pool: &Pool, persona_id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM audio_channels WHERE persona_id = $1")
                .bind(persona_id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM audio_channels WHERE persona_id = ?1")
                .bind(persona_id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Track CRUD
// ---------------------------------------------------------------------------

/// Create a track.
pub async fn create_track(pool: &Pool, track: &TrackRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO tracks ({TRACK_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
                ),
            )
            .bind(track.id)
            .bind(track.channel_id)
            .bind(track.user_id)
            .bind(&track.title)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(track.track_number)
            .bind(track.duration)
            .bind(&track.file_path)
            .bind(track.file_size)
            .bind(&track.mime_type)
            .bind(track.cover_media_id)
            .bind(&track.description)
            .bind(&track.ap_id)
            .bind(&track.visibility)
            .bind(track.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO tracks ({TRACK_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"
                ),
            )
            .bind(track.id)
            .bind(track.channel_id)
            .bind(track.user_id)
            .bind(&track.title)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(track.track_number)
            .bind(track.duration)
            .bind(&track.file_path)
            .bind(track.file_size)
            .bind(&track.mime_type)
            .bind(track.cover_media_id)
            .bind(&track.description)
            .bind(&track.ap_id)
            .bind(&track.visibility)
            .bind(track.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a track by ID.
pub async fn get_track(pool: &Pool, id: i64) -> Result<Option<TrackRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {TRACK_COLS} FROM tracks WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_track))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {TRACK_COLS} FROM tracks WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_track))
        }
    }
}

/// List tracks in a channel, ordered by creation time descending.
pub async fn list_tracks(
    pool: &Pool,
    channel_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE channel_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                ),
            )
            .bind(channel_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_track).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE channel_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
                ),
            )
            .bind(channel_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_track).collect())
        }
    }
}

/// List all public tracks, ordered by creation time descending.
pub async fn list_public_tracks(
    pool: &Pool,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE visibility = 'public' \
                     ORDER BY created_at DESC LIMIT $1 OFFSET $2"
                ),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_track).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE visibility = 'public' \
                     ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                ),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_track).collect())
        }
    }
}

/// List distinct album names for public tracks, with track count.
/// Returns Vec<(album, artist, track_count)>.
pub async fn list_albums(
    pool: &Pool,
    limit: i64,
    offset: i64,
) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT album, MIN(artist) as artist, COUNT(*) as cnt FROM tracks \
                 WHERE visibility = 'public' AND album != '' \
                 GROUP BY album ORDER BY MAX(created_at) DESC LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT album, MIN(artist) as artist, COUNT(*) as cnt FROM tracks \
                 WHERE visibility = 'public' AND album != '' \
                 GROUP BY album ORDER BY MAX(created_at) DESC LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await
        }
    }
}

/// List tracks in a specific album.
pub async fn list_tracks_by_album(
    pool: &Pool,
    album: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE album = $1 AND visibility = 'public' \
                     ORDER BY track_number, created_at LIMIT $2 OFFSET $3"
                ),
            )
            .bind(album)
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_track).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {TRACK_COLS} FROM tracks \
                     WHERE album = ?1 AND visibility = 'public' \
                     ORDER BY track_number, created_at LIMIT ?2 OFFSET ?3"
                ),
            )
            .bind(album)
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_track).collect())
        }
    }
}

/// Delete a track.
pub async fn delete_track(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM tracks WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM tracks WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Audio Playlist CRUD
// ---------------------------------------------------------------------------

/// Create an audio playlist.
pub async fn create_audio_playlist(
    pool: &Pool,
    id: i64,
    user_id: i64,
    title: &str,
    desc: &str,
    visibility: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO audio_playlists (id, user_id, title, description, visibility, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(user_id)
            .bind(title)
            .bind(desc)
            .bind(visibility)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO audio_playlists (id, user_id, title, description, visibility, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(id)
            .bind(user_id)
            .bind(title)
            .bind(desc)
            .bind(visibility)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get an audio playlist by ID.
/// Returns (id, user_id, title, description, visibility, created_at).
pub async fn get_audio_playlist(
    pool: &Pool,
    id: i64,
) -> Result<Option<(i64, i64, String, String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM audio_playlists WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM audio_playlists WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(sq)
            .await
        }
    }
}

/// List audio playlists (public).
/// Returns Vec<(id, user_id, title, description, visibility, created_at)>.
pub async fn list_audio_playlists(
    pool: &Pool,
    limit: i64,
    offset: i64,
) -> Result<Vec<(i64, i64, String, String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM audio_playlists WHERE visibility = 'public' \
                 ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM audio_playlists WHERE visibility = 'public' \
                 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await
        }
    }
}

/// List audio playlists for a user.
/// Returns Vec<(id, title, visibility, created_at)>.
pub async fn list_user_audio_playlists(
    pool: &Pool,
    user_id: i64,
) -> Result<Vec<(i64, String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, title, visibility, created_at FROM audio_playlists \
                 WHERE user_id = $1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, title, visibility, created_at FROM audio_playlists \
                 WHERE user_id = ?1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await
        }
    }
}

/// Delete an audio playlist.
pub async fn delete_audio_playlist(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM audio_playlists WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM audio_playlists WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Add a track to an audio playlist. Silently replaces on duplicate.
pub async fn add_to_audio_playlist(
    pool: &Pool,
    playlist_id: i64,
    track_id: i64,
    position: i32,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO audio_playlist_items (playlist_id, track_id, position) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (playlist_id, track_id) DO UPDATE SET position = $3",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO audio_playlist_items (playlist_id, track_id, position) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT (playlist_id, track_id) DO UPDATE SET position = ?3",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get track IDs in an audio playlist, ordered by position.
pub async fn audio_playlist_items(pool: &Pool, playlist_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT track_id FROM audio_playlist_items \
                 WHERE playlist_id = $1 ORDER BY position, track_id",
            )
            .bind(playlist_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT track_id FROM audio_playlist_items \
                 WHERE playlist_id = ?1 ORDER BY position, track_id",
            )
            .bind(playlist_id)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|r| r.0).collect())
}

// ---------------------------------------------------------------------------
// Podcast Series CRUD
// ---------------------------------------------------------------------------

/// Create a podcast series.
pub async fn create_podcast_series(pool: &Pool, series: &PodcastSeriesRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO podcast_series ({SERIES_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
                ),
            )
            .bind(series.id)
            .bind(series.channel_id)
            .bind(series.user_id)
            .bind(&series.title)
            .bind(&series.description)
            .bind(&series.description_html)
            .bind(&series.author)
            .bind(&series.language)
            .bind(&series.category)
            .bind(&series.itunes_category)
            .bind(series.cover_media_id)
            .bind(series.explicit)
            .bind(&series.rss_link)
            .bind(series.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO podcast_series ({SERIES_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
                ),
            )
            .bind(series.id)
            .bind(series.channel_id)
            .bind(series.user_id)
            .bind(&series.title)
            .bind(&series.description)
            .bind(&series.description_html)
            .bind(&series.author)
            .bind(&series.language)
            .bind(&series.category)
            .bind(&series.itunes_category)
            .bind(series.cover_media_id)
            .bind(series.explicit)
            .bind(&series.rss_link)
            .bind(series.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a podcast series by ID.
pub async fn get_podcast_series(pool: &Pool, id: i64) -> Result<Option<PodcastSeriesRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {SERIES_COLS} FROM podcast_series WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_series))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {SERIES_COLS} FROM podcast_series WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_series))
        }
    }
}

/// List podcast series in a channel.
pub async fn list_podcast_series(
    pool: &Pool,
    channel_id: i64,
) -> Result<Vec<PodcastSeriesRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {SERIES_COLS} FROM podcast_series \
                     WHERE channel_id = $1 ORDER BY created_at DESC"
                ),
            )
            .bind(channel_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_series).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {SERIES_COLS} FROM podcast_series \
                     WHERE channel_id = ?1 ORDER BY created_at DESC"
                ),
            )
            .bind(channel_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_series).collect())
        }
    }
}

/// Delete a podcast series (episodes cascade-deleted by FK).
pub async fn delete_podcast_series(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM podcast_series WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM podcast_series WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Podcast Episode CRUD
// ---------------------------------------------------------------------------

/// Create a podcast episode.
pub async fn create_podcast_episode(pool: &Pool, ep: &PodcastEpisodeRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO podcast_episodes ({EPISODE_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
                ),
            )
            .bind(ep.id)
            .bind(ep.series_id)
            .bind(&ep.title)
            .bind(&ep.description)
            .bind(&ep.description_html)
            .bind(ep.duration)
            .bind(&ep.file_path)
            .bind(ep.file_size)
            .bind(&ep.mime_type)
            .bind(ep.episode_number)
            .bind(ep.season_number)
            .bind(ep.explicit)
            .bind(&ep.ap_id)
            .bind(ep.published_at)
            .bind(ep.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO podcast_episodes ({EPISODE_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
                ),
            )
            .bind(ep.id)
            .bind(ep.series_id)
            .bind(&ep.title)
            .bind(&ep.description)
            .bind(&ep.description_html)
            .bind(ep.duration)
            .bind(&ep.file_path)
            .bind(ep.file_size)
            .bind(&ep.mime_type)
            .bind(ep.episode_number)
            .bind(ep.season_number)
            .bind(ep.explicit)
            .bind(&ep.ap_id)
            .bind(ep.published_at)
            .bind(ep.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a podcast episode by ID.
pub async fn get_podcast_episode(pool: &Pool, id: i64) -> Result<Option<PodcastEpisodeRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {EPISODE_COLS} FROM podcast_episodes WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_episode))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {EPISODE_COLS} FROM podcast_episodes WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_episode))
        }
    }
}

/// List episodes in a podcast series, ordered by published_at descending.
pub async fn list_podcast_episodes(
    pool: &Pool,
    series_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<PodcastEpisodeRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {EPISODE_COLS} FROM podcast_episodes \
                     WHERE series_id = $1 ORDER BY published_at DESC LIMIT $2 OFFSET $3"
                ),
            )
            .bind(series_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_episode).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {EPISODE_COLS} FROM podcast_episodes \
                     WHERE series_id = ?1 ORDER BY published_at DESC LIMIT ?2 OFFSET ?3"
                ),
            )
            .bind(series_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_episode).collect())
        }
    }
}

/// Delete a podcast episode.
pub async fn delete_podcast_episode(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM podcast_episodes WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM podcast_episodes WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        // Seed a user and personas for FK constraints
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (100, 1, 'audio_persona', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO posts (id, user_id, persona_id, ap_id, content, content_html, visibility, created_at) \
             VALUES (100, 1, 101, 'https://example.com/posts/100', 'hi', '<p>hi</p>', 'public', 1000); \
             INSERT INTO media (id, user_id, persona_id, post_id, file_path, mime_type, file_size, description, created_at) \
             VALUES (10, 1, 101, 100, 'media/cover.jpg', 'image/jpeg', 5000, 'cover', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    fn sample_channel() -> AudioChannelRow {
        AudioChannelRow {
            persona_id: 100,
            description: "An audio channel".into(),
            category: "music".into(),
            language: Some("en".into()),
            rss_enabled: false,
            itunes_category: None,
            created_at: 2000,
        }
    }

    fn sample_track(id: i64, ap_id: &str) -> TrackRow {
        TrackRow {
            id,
            channel_id: 100,
            user_id: 1,
            title: "My Song".into(),
            artist: "Test Artist".into(),
            album: "Test Album".into(),
            track_number: Some(1),
            duration: Some(180),
            file_path: "uploads/song.mp3".into(),
            file_size: 5_000_000,
            mime_type: "audio/mpeg".into(),
            cover_media_id: None,
            description: "A great song".into(),
            ap_id: ap_id.into(),
            visibility: "public".into(),
            created_at: 3000,
        }
    }

    fn sample_series() -> PodcastSeriesRow {
        PodcastSeriesRow {
            id: 1,
            channel_id: 100,
            user_id: 1,
            title: "My Podcast".into(),
            description: "A podcast".into(),
            description_html: "<p>A podcast</p>".into(),
            author: "Test Author".into(),
            language: Some("en".into()),
            category: Some("Technology".into()),
            itunes_category: Some("Technology".into()),
            cover_media_id: Some(10),
            explicit: false,
            rss_link: None,
            created_at: 4000,
        }
    }

    fn sample_episode(id: i64, series_id: i64, ap_id: &str) -> PodcastEpisodeRow {
        PodcastEpisodeRow {
            id,
            series_id,
            title: "Episode 1".into(),
            description: "First episode".into(),
            description_html: "<p>First episode</p>".into(),
            duration: Some(3600),
            file_path: "uploads/ep1.mp3".into(),
            file_size: 50_000_000,
            mime_type: "audio/mpeg".into(),
            episode_number: Some(1),
            season_number: Some(1),
            explicit: false,
            ap_id: ap_id.into(),
            published_at: 5000,
            created_at: 5000,
        }
    }

    // -- Audio Channel CRUD --

    #[tokio::test]
    async fn create_and_get_audio_channel() {
        let (pool, _) = setup().await;
        let ch = sample_channel();
        create_audio_channel(&pool, &ch).await.unwrap();

        let fetched = get_audio_channel(&pool, 100).await.unwrap().expect("should exist");
        assert_eq!(fetched.description, "An audio channel");
        assert_eq!(fetched.category, "music");
        assert_eq!(fetched.language.as_deref(), Some("en"));
        assert!(!fetched.rss_enabled);
        assert!(fetched.itunes_category.is_none());
        assert_eq!(fetched.created_at, 2000);
    }

    #[tokio::test]
    async fn get_nonexistent_audio_channel() {
        let (pool, _) = setup().await;
        assert!(get_audio_channel(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_audio_channels_by_user() {
        let (pool, sq) = setup().await;
        sqlx::raw_sql(
            "INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (200, 1, 'channel2', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();

        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let ch2 = AudioChannelRow {
            persona_id: 200,
            description: "Second channel".into(),
            category: "podcast".into(),
            language: None,
            rss_enabled: true,
            itunes_category: Some("Arts".into()),
            created_at: 3000,
        };
        create_audio_channel(&pool, &ch2).await.unwrap();

        let channels = list_audio_channels(&pool, 1).await.unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].persona_id, 200); // newer first
        assert_eq!(channels[1].persona_id, 100);

        let empty = list_audio_channels(&pool, 999).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn list_all_audio_channels_paginated() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let all = list_all_audio_channels(&pool, 10, 0).await.unwrap();
        assert_eq!(all.len(), 1);

        let empty = list_all_audio_channels(&pool, 10, 1).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn delete_audio_channel_works() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        delete_audio_channel(&pool, 100).await.unwrap();
        assert!(get_audio_channel(&pool, 100).await.unwrap().is_none());
    }

    // -- Track CRUD --

    #[tokio::test]
    async fn create_and_get_track() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let t = sample_track(1, "https://example.com/track/1");
        create_track(&pool, &t).await.unwrap();

        let fetched = get_track(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "My Song");
        assert_eq!(fetched.artist, "Test Artist");
        assert_eq!(fetched.album, "Test Album");
        assert_eq!(fetched.channel_id, 100);
        assert_eq!(fetched.duration, Some(180));
        assert_eq!(fetched.file_size, 5_000_000);
        assert_eq!(fetched.mime_type, "audio/mpeg");
    }

    #[tokio::test]
    async fn get_nonexistent_track() {
        let (pool, _) = setup().await;
        assert!(get_track(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_tracks_in_channel() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let mut t1 = sample_track(1, "https://example.com/track/1");
        t1.created_at = 3000;
        let mut t2 = sample_track(2, "https://example.com/track/2");
        t2.title = "Second Song".into();
        t2.created_at = 4000;
        create_track(&pool, &t1).await.unwrap();
        create_track(&pool, &t2).await.unwrap();

        let tracks = list_tracks(&pool, 100, 10, 0).await.unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Second Song"); // newer first
        assert_eq!(tracks[1].title, "My Song");

        // Pagination
        let page = list_tracks(&pool, 100, 1, 0).await.unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, 2);
    }

    #[tokio::test]
    async fn list_public_tracks_filters() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let t1 = sample_track(1, "https://example.com/track/1");
        create_track(&pool, &t1).await.unwrap();

        let mut t2 = sample_track(2, "https://example.com/track/2");
        t2.visibility = "private".into();
        create_track(&pool, &t2).await.unwrap();

        let public = list_public_tracks(&pool, 10, 0).await.unwrap();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].id, 1);
    }

    #[tokio::test]
    async fn list_albums_groups_tracks() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let mut t1 = sample_track(1, "https://example.com/track/1");
        t1.album = "Album A".into();
        t1.created_at = 3000;
        let mut t2 = sample_track(2, "https://example.com/track/2");
        t2.album = "Album A".into();
        t2.created_at = 3001;
        let mut t3 = sample_track(3, "https://example.com/track/3");
        t3.album = "Album B".into();
        t3.created_at = 4000;
        create_track(&pool, &t1).await.unwrap();
        create_track(&pool, &t2).await.unwrap();
        create_track(&pool, &t3).await.unwrap();

        let albums = list_albums(&pool, 10, 0).await.unwrap();
        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].0, "Album B"); // newer first
        assert_eq!(albums[1].0, "Album A");
        assert_eq!(albums[1].2, 2); // 2 tracks in Album A
    }

    #[tokio::test]
    async fn list_tracks_by_album_works() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let mut t1 = sample_track(1, "https://example.com/track/1");
        t1.album = "Test Album".into();
        t1.track_number = Some(2);
        let mut t2 = sample_track(2, "https://example.com/track/2");
        t2.album = "Test Album".into();
        t2.track_number = Some(1);
        let mut t3 = sample_track(3, "https://example.com/track/3");
        t3.album = "Other Album".into();
        create_track(&pool, &t1).await.unwrap();
        create_track(&pool, &t2).await.unwrap();
        create_track(&pool, &t3).await.unwrap();

        let tracks = list_tracks_by_album(&pool, "Test Album", 10, 0).await.unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].track_number, Some(1)); // ordered by track_number
        assert_eq!(tracks[1].track_number, Some(2));
    }

    #[tokio::test]
    async fn delete_track_works() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_track(&pool, &sample_track(1, "https://example.com/track/1"))
            .await
            .unwrap();

        delete_track(&pool, 1).await.unwrap();
        assert!(get_track(&pool, 1).await.unwrap().is_none());
    }

    // -- Audio Playlist CRUD --

    #[tokio::test]
    async fn create_and_get_audio_playlist() {
        let (pool, _) = setup().await;
        create_audio_playlist(&pool, 1, 1, "Favorites", "My favorites", "public", 5000)
            .await
            .unwrap();

        let pl = get_audio_playlist(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(pl.0, 1);
        assert_eq!(pl.1, 1);
        assert_eq!(pl.2, "Favorites");
        assert_eq!(pl.3, "My favorites");
        assert_eq!(pl.4, "public");
        assert_eq!(pl.5, 5000);
    }

    #[tokio::test]
    async fn get_nonexistent_audio_playlist() {
        let (pool, _) = setup().await;
        assert!(get_audio_playlist(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_user_audio_playlists_works() {
        let (pool, _) = setup().await;
        create_audio_playlist(&pool, 1, 1, "First", "", "public", 5000)
            .await
            .unwrap();
        create_audio_playlist(&pool, 2, 1, "Second", "", "private", 6000)
            .await
            .unwrap();

        let playlists = list_user_audio_playlists(&pool, 1).await.unwrap();
        assert_eq!(playlists.len(), 2);
        assert_eq!(playlists[0].1, "Second"); // newer first
        assert_eq!(playlists[1].1, "First");

        let empty = list_user_audio_playlists(&pool, 999).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn list_public_audio_playlists() {
        let (pool, _) = setup().await;
        create_audio_playlist(&pool, 1, 1, "Public", "", "public", 5000)
            .await
            .unwrap();
        create_audio_playlist(&pool, 2, 1, "Private", "", "private", 6000)
            .await
            .unwrap();

        let public = list_audio_playlists(&pool, 10, 0).await.unwrap();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].2, "Public");
    }

    #[tokio::test]
    async fn audio_playlist_items_ordered() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_track(&pool, &sample_track(1, "https://example.com/track/1"))
            .await
            .unwrap();
        create_track(&pool, &sample_track(2, "https://example.com/track/2"))
            .await
            .unwrap();
        create_track(&pool, &sample_track(3, "https://example.com/track/3"))
            .await
            .unwrap();

        create_audio_playlist(&pool, 1, 1, "Queue", "", "public", 5000)
            .await
            .unwrap();
        add_to_audio_playlist(&pool, 1, 3, 0).await.unwrap();
        add_to_audio_playlist(&pool, 1, 1, 1).await.unwrap();
        add_to_audio_playlist(&pool, 1, 2, 2).await.unwrap();

        let items = audio_playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![3, 1, 2]);
    }

    #[tokio::test]
    async fn add_to_audio_playlist_upsert() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_track(&pool, &sample_track(1, "https://example.com/track/1"))
            .await
            .unwrap();

        create_audio_playlist(&pool, 1, 1, "My List", "", "public", 5000)
            .await
            .unwrap();
        add_to_audio_playlist(&pool, 1, 1, 0).await.unwrap();
        add_to_audio_playlist(&pool, 1, 1, 5).await.unwrap();

        let items = audio_playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![1]);
    }

    #[tokio::test]
    async fn delete_audio_playlist_cascades() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_track(&pool, &sample_track(1, "https://example.com/track/1"))
            .await
            .unwrap();

        create_audio_playlist(&pool, 1, 1, "Temp", "", "public", 5000)
            .await
            .unwrap();
        add_to_audio_playlist(&pool, 1, 1, 0).await.unwrap();

        delete_audio_playlist(&pool, 1).await.unwrap();
        assert!(get_audio_playlist(&pool, 1).await.unwrap().is_none());
        let items = audio_playlist_items(&pool, 1).await.unwrap();
        assert!(items.is_empty());
    }

    // -- Podcast Series CRUD --

    #[tokio::test]
    async fn create_and_get_podcast_series() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let s = sample_series();
        create_podcast_series(&pool, &s).await.unwrap();

        let fetched = get_podcast_series(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "My Podcast");
        assert_eq!(fetched.author, "Test Author");
        assert_eq!(fetched.language.as_deref(), Some("en"));
        assert_eq!(fetched.itunes_category.as_deref(), Some("Technology"));
        assert_eq!(fetched.cover_media_id, Some(10));
        assert!(!fetched.explicit);
    }

    #[tokio::test]
    async fn get_nonexistent_podcast_series() {
        let (pool, _) = setup().await;
        assert!(get_podcast_series(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_podcast_series_in_channel() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        let mut s1 = sample_series();
        s1.created_at = 4000;
        create_podcast_series(&pool, &s1).await.unwrap();

        let mut s2 = sample_series();
        s2.id = 2;
        s2.title = "Second Podcast".into();
        s2.created_at = 5000;
        create_podcast_series(&pool, &s2).await.unwrap();

        let series = list_podcast_series(&pool, 100).await.unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].title, "Second Podcast"); // newer first
        assert_eq!(series[1].title, "My Podcast");
    }

    #[tokio::test]
    async fn delete_podcast_series_cascades_episodes() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_podcast_series(&pool, &sample_series()).await.unwrap();
        create_podcast_episode(&pool, &sample_episode(1, 1, "https://example.com/ep/1"))
            .await
            .unwrap();

        delete_podcast_series(&pool, 1).await.unwrap();
        assert!(get_podcast_series(&pool, 1).await.unwrap().is_none());
        let eps = list_podcast_episodes(&pool, 1, 10, 0).await.unwrap();
        assert!(eps.is_empty());
    }

    // -- Podcast Episode CRUD --

    #[tokio::test]
    async fn create_and_get_podcast_episode() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_podcast_series(&pool, &sample_series()).await.unwrap();

        let ep = sample_episode(1, 1, "https://example.com/ep/1");
        create_podcast_episode(&pool, &ep).await.unwrap();

        let fetched = get_podcast_episode(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "Episode 1");
        assert_eq!(fetched.duration, Some(3600));
        assert_eq!(fetched.file_size, 50_000_000);
        assert_eq!(fetched.episode_number, Some(1));
        assert_eq!(fetched.season_number, Some(1));
        assert!(!fetched.explicit);
    }

    #[tokio::test]
    async fn get_nonexistent_podcast_episode() {
        let (pool, _) = setup().await;
        assert!(get_podcast_episode(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_podcast_episodes_ordered() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_podcast_series(&pool, &sample_series()).await.unwrap();

        let mut ep1 = sample_episode(1, 1, "https://example.com/ep/1");
        ep1.published_at = 5000;
        let mut ep2 = sample_episode(2, 1, "https://example.com/ep/2");
        ep2.title = "Episode 2".into();
        ep2.published_at = 6000;
        create_podcast_episode(&pool, &ep1).await.unwrap();
        create_podcast_episode(&pool, &ep2).await.unwrap();

        let eps = list_podcast_episodes(&pool, 1, 10, 0).await.unwrap();
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].title, "Episode 2"); // newer first
        assert_eq!(eps[1].title, "Episode 1");

        // Pagination
        let page = list_podcast_episodes(&pool, 1, 1, 0).await.unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, 2);
    }

    #[tokio::test]
    async fn delete_podcast_episode_works() {
        let (pool, _) = setup().await;
        create_audio_channel(&pool, &sample_channel()).await.unwrap();
        create_podcast_series(&pool, &sample_series()).await.unwrap();
        create_podcast_episode(&pool, &sample_episode(1, 1, "https://example.com/ep/1"))
            .await
            .unwrap();

        delete_podcast_episode(&pool, 1).await.unwrap();
        assert!(get_podcast_episode(&pool, 1).await.unwrap().is_none());
    }

    // -- Full lifecycle --

    #[tokio::test]
    async fn full_audio_lifecycle() {
        let (pool, _) = setup().await;

        // Create channel
        create_audio_channel(&pool, &sample_channel()).await.unwrap();

        // Upload tracks
        let t1 = sample_track(1, "https://example.com/track/1");
        create_track(&pool, &t1).await.unwrap();
        let mut t2 = sample_track(2, "https://example.com/track/2");
        t2.title = "Second Song".into();
        t2.created_at = 4000;
        create_track(&pool, &t2).await.unwrap();

        // Create playlist
        create_audio_playlist(&pool, 1, 1, "My Mix", "Best tracks", "public", 5000)
            .await
            .unwrap();
        add_to_audio_playlist(&pool, 1, 2, 0).await.unwrap();
        add_to_audio_playlist(&pool, 1, 1, 1).await.unwrap();

        let items = audio_playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![2, 1]);

        // Create podcast series
        create_podcast_series(&pool, &sample_series()).await.unwrap();

        // Add episodes
        create_podcast_episode(&pool, &sample_episode(1, 1, "https://example.com/ep/1"))
            .await
            .unwrap();
        let mut ep2 = sample_episode(2, 1, "https://example.com/ep/2");
        ep2.title = "Episode 2".into();
        ep2.published_at = 6000;
        ep2.created_at = 6000;
        create_podcast_episode(&pool, &ep2).await.unwrap();

        // Verify listings
        let tracks = list_public_tracks(&pool, 10, 0).await.unwrap();
        assert_eq!(tracks.len(), 2);

        let albums = list_albums(&pool, 10, 0).await.unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].2, 2); // 2 tracks

        let series = list_podcast_series(&pool, 100).await.unwrap();
        assert_eq!(series.len(), 1);

        let eps = list_podcast_episodes(&pool, 1, 10, 0).await.unwrap();
        assert_eq!(eps.len(), 2);
    }
}
