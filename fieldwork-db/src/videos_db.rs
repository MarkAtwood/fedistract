//! Database operations for PeerTube-style video hosting.
//!
//! Video channels are AP Group actors backed by a persona (same pattern as
//! communities). This module provides CRUD for channels, videos, transcoding
//! jobs, HLS video files, threaded comments, and playlists.
//!
//! `VideoRow` has 22 columns, exceeding sqlx's 16-tuple `FromRow` limit,
//! so we use `query().fetch_*()` + `Row::get` by column index.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A row from the `video_channels` table.
#[derive(Debug, Clone)]
pub struct ChannelRow {
    pub persona_id: i64,
    pub description: String,
    pub banner_media_id: Option<i64>,
    pub created_at: i64,
}

type ChannelTuple = (i64, String, Option<i64>, i64);

fn tuple_to_channel(t: ChannelTuple) -> ChannelRow {
    ChannelRow {
        persona_id: t.0,
        description: t.1,
        banner_media_id: t.2,
        created_at: t.3,
    }
}

const CHANNEL_COLS: &str = "persona_id, description, banner_media_id, created_at";

/// A row from the `videos` table.
#[derive(Debug, Clone)]
pub struct VideoRow {
    pub id: i64,
    pub channel_id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: String,
    pub description_html: String,
    pub ap_id: String,
    pub duration: Option<i64>,
    pub original_file_path: Option<String>,
    pub original_file_size: Option<i64>,
    pub original_mime_type: Option<String>,
    pub thumbnail_path: Option<String>,
    pub preview_path: Option<String>,
    pub visibility: String,
    pub nsfw: bool,
    pub language: Option<String>,
    pub state: String,
    pub views: i64,
    pub likes: i64,
    pub dislikes: i64,
    pub created_at: i64,
    pub published_at: Option<i64>,
}

const VIDEO_COLS: &str = "\
    id, channel_id, user_id, \
    title, description, description_html, ap_id, \
    duration, original_file_path, original_file_size, original_mime_type, \
    thumbnail_path, preview_path, \
    visibility, nsfw, language, \
    state, views, likes, dislikes, \
    created_at, published_at";

#[cfg(feature = "sqlite")]
fn sqlite_row_to_video(r: sqlx::sqlite::SqliteRow) -> VideoRow {
    use sqlx::Row;
    VideoRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        description: r.get(4),
        description_html: r.get(5),
        ap_id: r.get(6),
        duration: r.get(7),
        original_file_path: r.get(8),
        original_file_size: r.get(9),
        original_mime_type: r.get(10),
        thumbnail_path: r.get(11),
        preview_path: r.get(12),
        visibility: r.get(13),
        nsfw: r.get(14),
        language: r.get(15),
        state: r.get(16),
        views: r.get(17),
        likes: r.get(18),
        dislikes: r.get(19),
        created_at: r.get(20),
        published_at: r.get(21),
    }
}

#[cfg(feature = "postgres")]
fn pg_row_to_video(r: sqlx::postgres::PgRow) -> VideoRow {
    use sqlx::Row;
    VideoRow {
        id: r.get(0),
        channel_id: r.get(1),
        user_id: r.get(2),
        title: r.get(3),
        description: r.get(4),
        description_html: r.get(5),
        ap_id: r.get(6),
        duration: r.get(7),
        original_file_path: r.get(8),
        original_file_size: r.get(9),
        original_mime_type: r.get(10),
        thumbnail_path: r.get(11),
        preview_path: r.get(12),
        visibility: r.get(13),
        nsfw: r.get(14),
        language: r.get(15),
        state: r.get(16),
        views: r.get(17),
        likes: r.get(18),
        dislikes: r.get(19),
        created_at: r.get(20),
        published_at: r.get(21),
    }
}

/// A row from the `transcoding_jobs` table.
#[derive(Debug, Clone)]
pub struct TranscodingJobRow {
    pub id: i64,
    pub video_id: i64,
    pub resolution: i32,
    pub state: String,
    pub progress: i32,
    pub output_path: Option<String>,
    pub file_size: Option<i64>,
    pub error: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

type JobTuple = (
    i64, i64, i32, String, i32,
    Option<String>, Option<i64>, Option<String>,
    i64, Option<i64>, Option<i64>,
);

fn tuple_to_job(t: JobTuple) -> TranscodingJobRow {
    TranscodingJobRow {
        id: t.0,
        video_id: t.1,
        resolution: t.2,
        state: t.3,
        progress: t.4,
        output_path: t.5,
        file_size: t.6,
        error: t.7,
        created_at: t.8,
        started_at: t.9,
        completed_at: t.10,
    }
}

const JOB_COLS: &str = "\
    id, video_id, resolution, state, progress, \
    output_path, file_size, error, \
    created_at, started_at, completed_at";

/// A row from the `video_comments` table.
#[derive(Debug, Clone)]
pub struct VideoCommentRow {
    pub id: i64,
    pub video_id: i64,
    pub parent_comment_id: Option<i64>,
    pub author_persona_id: i64,
    pub content: String,
    pub content_html: String,
    pub ap_id: String,
    pub removed: bool,
    pub created_at: i64,
}

type CommentTuple = (
    i64, i64, Option<i64>, i64,
    String, String, String, bool, i64,
);

fn tuple_to_comment(t: CommentTuple) -> VideoCommentRow {
    VideoCommentRow {
        id: t.0,
        video_id: t.1,
        parent_comment_id: t.2,
        author_persona_id: t.3,
        content: t.4,
        content_html: t.5,
        ap_id: t.6,
        removed: t.7,
        created_at: t.8,
    }
}

const COMMENT_COLS: &str = "\
    id, video_id, parent_comment_id, author_persona_id, \
    content, content_html, ap_id, removed, created_at";

// ---------------------------------------------------------------------------
// Channel CRUD
// ---------------------------------------------------------------------------

/// Create a video channel (persona must already exist).
pub async fn create_channel(pool: &Pool, row: &ChannelRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO video_channels (persona_id, description, banner_media_id, created_at) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(row.persona_id)
            .bind(&row.description)
            .bind(row.banner_media_id)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO video_channels (persona_id, description, banner_media_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(row.persona_id)
            .bind(&row.description)
            .bind(row.banner_media_id)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a video channel by persona_id.
pub async fn get_channel(pool: &Pool, persona_id: i64) -> Result<Option<ChannelRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM video_channels WHERE persona_id = $1"),
            )
            .bind(persona_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_channel))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<ChannelTuple> = sqlx::query_as(
                &format!("SELECT {CHANNEL_COLS} FROM video_channels WHERE persona_id = ?1"),
            )
            .bind(persona_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_channel))
        }
    }
}

/// List channels owned by a user (via personas.user_id join).
pub async fn list_channels(pool: &Pool, user_id: i64) -> Result<Vec<ChannelRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ChannelTuple> = sqlx::query_as(
                &format!(
                    "SELECT vc.{} FROM video_channels vc \
                     JOIN personas p ON p.id = vc.persona_id \
                     WHERE p.user_id = $1 ORDER BY vc.created_at DESC",
                    CHANNEL_COLS.replace(", ", ", vc.")
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
                    "SELECT vc.{} FROM video_channels vc \
                     JOIN personas p ON p.id = vc.persona_id \
                     WHERE p.user_id = ?1 ORDER BY vc.created_at DESC",
                    CHANNEL_COLS.replace(", ", ", vc.")
                ),
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_channel).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Video CRUD
// ---------------------------------------------------------------------------

/// Create a video.
pub async fn create_video(pool: &Pool, video: &VideoRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO videos ({VIDEO_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                             $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)"
                ),
            )
            .bind(video.id)
            .bind(video.channel_id)
            .bind(video.user_id)
            .bind(&video.title)
            .bind(&video.description)
            .bind(&video.description_html)
            .bind(&video.ap_id)
            .bind(video.duration)
            .bind(&video.original_file_path)
            .bind(video.original_file_size)
            .bind(&video.original_mime_type)
            .bind(&video.thumbnail_path)
            .bind(&video.preview_path)
            .bind(&video.visibility)
            .bind(video.nsfw)
            .bind(&video.language)
            .bind(&video.state)
            .bind(video.views)
            .bind(video.likes)
            .bind(video.dislikes)
            .bind(video.created_at)
            .bind(video.published_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO videos ({VIDEO_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
                             ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"
                ),
            )
            .bind(video.id)
            .bind(video.channel_id)
            .bind(video.user_id)
            .bind(&video.title)
            .bind(&video.description)
            .bind(&video.description_html)
            .bind(&video.ap_id)
            .bind(video.duration)
            .bind(&video.original_file_path)
            .bind(video.original_file_size)
            .bind(&video.original_mime_type)
            .bind(&video.thumbnail_path)
            .bind(&video.preview_path)
            .bind(&video.visibility)
            .bind(video.nsfw)
            .bind(&video.language)
            .bind(&video.state)
            .bind(video.views)
            .bind(video.likes)
            .bind(video.dislikes)
            .bind(video.created_at)
            .bind(video.published_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a video by ID.
pub async fn get_video(pool: &Pool, id: i64) -> Result<Option<VideoRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {VIDEO_COLS} FROM videos WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_video))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {VIDEO_COLS} FROM videos WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sqlite_row_to_video))
        }
    }
}

/// List videos in a channel, ordered by creation time descending.
pub async fn list_videos(
    pool: &Pool,
    channel_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<VideoRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {VIDEO_COLS} FROM videos \
                     WHERE channel_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                ),
            )
            .bind(channel_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_video).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {VIDEO_COLS} FROM videos \
                     WHERE channel_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
                ),
            )
            .bind(channel_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_video).collect())
        }
    }
}

/// List published public videos, ordered by creation time descending.
pub async fn list_published_videos(
    pool: &Pool,
    limit: i64,
    offset: i64,
) -> Result<Vec<VideoRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {VIDEO_COLS} FROM videos \
                     WHERE state = 'published' AND visibility = 'public' \
                     ORDER BY created_at DESC LIMIT $1 OFFSET $2"
                ),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_video).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!(
                    "SELECT {VIDEO_COLS} FROM videos \
                     WHERE state = 'published' AND visibility = 'public' \
                     ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                ),
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sqlite_row_to_video).collect())
        }
    }
}

/// Update a video's title, description, and/or description_html.
pub async fn update_video_metadata(
    pool: &Pool,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    description_html: Option<&str>,
) -> Result<(), sqlx::Error> {
    let current = match get_video(pool, id).await? {
        Some(v) => v,
        None => return Ok(()),
    };
    let new_title = title.unwrap_or(&current.title);
    let new_desc = description.unwrap_or(&current.description);
    let new_html = description_html.unwrap_or(&current.description_html);

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE videos SET title = $1, description = $2, description_html = $3 \
                 WHERE id = $4",
            )
            .bind(new_title)
            .bind(new_desc)
            .bind(new_html)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE videos SET title = ?1, description = ?2, description_html = ?3 \
                 WHERE id = ?4",
            )
            .bind(new_title)
            .bind(new_desc)
            .bind(new_html)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Set a video's state (pending/processing/published/error).
pub async fn set_video_state(pool: &Pool, id: i64, state: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE videos SET state = $1 WHERE id = $2")
                .bind(state)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE videos SET state = ?1 WHERE id = ?2")
                .bind(state)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Set a video's thumbnail path.
pub async fn set_video_thumbnail(pool: &Pool, id: i64, path: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE videos SET thumbnail_path = $1 WHERE id = $2")
                .bind(path)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE videos SET thumbnail_path = ?1 WHERE id = ?2")
                .bind(path)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Increment the view count for a video.
pub async fn increment_views(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE videos SET views = views + 1 WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE videos SET views = views + 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Delete a video (cascades to transcoding_jobs, video_files, video_comments).
pub async fn delete_video(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM videos WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM videos WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transcoding jobs
// ---------------------------------------------------------------------------

/// Create a transcoding job.
pub async fn create_job(pool: &Pool, job: &TranscodingJobRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO transcoding_jobs ({JOB_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                ),
            )
            .bind(job.id)
            .bind(job.video_id)
            .bind(job.resolution)
            .bind(&job.state)
            .bind(job.progress)
            .bind(&job.output_path)
            .bind(job.file_size)
            .bind(&job.error)
            .bind(job.created_at)
            .bind(job.started_at)
            .bind(job.completed_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO transcoding_jobs ({JOB_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
                ),
            )
            .bind(job.id)
            .bind(job.video_id)
            .bind(job.resolution)
            .bind(&job.state)
            .bind(job.progress)
            .bind(&job.output_path)
            .bind(job.file_size)
            .bind(&job.error)
            .bind(job.created_at)
            .bind(job.started_at)
            .bind(job.completed_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get pending transcoding jobs, ordered by creation time.
pub async fn get_pending_jobs(
    pool: &Pool,
    limit: i64,
) -> Result<Vec<TranscodingJobRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<JobTuple> = sqlx::query_as(
                &format!(
                    "SELECT {JOB_COLS} FROM transcoding_jobs \
                     WHERE state = 'pending' ORDER BY created_at ASC LIMIT $1"
                ),
            )
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_job).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<JobTuple> = sqlx::query_as(
                &format!(
                    "SELECT {JOB_COLS} FROM transcoding_jobs \
                     WHERE state = 'pending' ORDER BY created_at ASC LIMIT ?1"
                ),
            )
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_job).collect())
        }
    }
}

/// Mark a transcoding job as started.
pub async fn start_job(pool: &Pool, id: i64, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE transcoding_jobs SET state = 'processing', started_at = $1 WHERE id = $2",
            )
            .bind(now)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE transcoding_jobs SET state = 'processing', started_at = ?1 WHERE id = ?2",
            )
            .bind(now)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Mark a transcoding job as completed.
pub async fn complete_job(
    pool: &Pool,
    id: i64,
    output_path: &str,
    file_size: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE transcoding_jobs \
                 SET state = 'completed', output_path = $1, file_size = $2, completed_at = $3 \
                 WHERE id = $4",
            )
            .bind(output_path)
            .bind(file_size)
            .bind(now)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE transcoding_jobs \
                 SET state = 'completed', output_path = ?1, file_size = ?2, completed_at = ?3 \
                 WHERE id = ?4",
            )
            .bind(output_path)
            .bind(file_size)
            .bind(now)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Mark a transcoding job as failed.
pub async fn fail_job(pool: &Pool, id: i64, error: &str, now: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE transcoding_jobs \
                 SET state = 'failed', error = $1, completed_at = $2 WHERE id = $3",
            )
            .bind(error)
            .bind(now)
            .bind(id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE transcoding_jobs \
                 SET state = 'failed', error = ?1, completed_at = ?2 WHERE id = ?3",
            )
            .bind(error)
            .bind(now)
            .bind(id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Update a transcoding job's progress (0-100).
pub async fn update_progress(pool: &Pool, id: i64, progress: i32) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE transcoding_jobs SET progress = $1 WHERE id = $2")
                .bind(progress)
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE transcoding_jobs SET progress = ?1 WHERE id = ?2")
                .bind(progress)
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Get all transcoding jobs for a video.
pub async fn jobs_for_video(
    pool: &Pool,
    video_id: i64,
) -> Result<Vec<TranscodingJobRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<JobTuple> = sqlx::query_as(
                &format!(
                    "SELECT {JOB_COLS} FROM transcoding_jobs \
                     WHERE video_id = $1 ORDER BY resolution"
                ),
            )
            .bind(video_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_job).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<JobTuple> = sqlx::query_as(
                &format!(
                    "SELECT {JOB_COLS} FROM transcoding_jobs \
                     WHERE video_id = ?1 ORDER BY resolution"
                ),
            )
            .bind(video_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_job).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Video files (HLS)
// ---------------------------------------------------------------------------

/// Add an HLS video file for a given resolution (upserts on video_id+resolution).
pub async fn add_video_file(
    pool: &Pool,
    video_id: i64,
    resolution: i32,
    file_path: &str,
    file_size: i64,
    fps: Option<i32>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO video_files (video_id, resolution, file_path, file_size, fps, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (video_id, resolution) \
                 DO UPDATE SET file_path = $3, file_size = $4, fps = $5",
            )
            .bind(video_id)
            .bind(resolution)
            .bind(file_path)
            .bind(file_size)
            .bind(fps)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO video_files (video_id, resolution, file_path, file_size, fps, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (video_id, resolution) \
                 DO UPDATE SET file_path = ?3, file_size = ?4, fps = ?5",
            )
            .bind(video_id)
            .bind(resolution)
            .bind(file_path)
            .bind(file_size)
            .bind(fps)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all HLS files for a video, ordered by resolution.
/// Returns (resolution, file_path, file_size, fps).
pub async fn video_files(
    pool: &Pool,
    video_id: i64,
) -> Result<Vec<(i32, String, i64, Option<i32>)>, sqlx::Error> {
    let rows: Vec<(i32, String, i64, Option<i32>)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT resolution, file_path, file_size, fps FROM video_files \
                 WHERE video_id = $1 ORDER BY resolution",
            )
            .bind(video_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT resolution, file_path, file_size, fps FROM video_files \
                 WHERE video_id = ?1 ORDER BY resolution",
            )
            .bind(video_id)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Video comments
// ---------------------------------------------------------------------------

/// Create a comment on a video.
pub async fn create_video_comment(
    pool: &Pool,
    comment: &VideoCommentRow,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO video_comments ({COMMENT_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
                ),
            )
            .bind(comment.id)
            .bind(comment.video_id)
            .bind(comment.parent_comment_id)
            .bind(comment.author_persona_id)
            .bind(&comment.content)
            .bind(&comment.content_html)
            .bind(&comment.ap_id)
            .bind(comment.removed)
            .bind(comment.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO video_comments ({COMMENT_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                ),
            )
            .bind(comment.id)
            .bind(comment.video_id)
            .bind(comment.parent_comment_id)
            .bind(comment.author_persona_id)
            .bind(&comment.content)
            .bind(&comment.content_html)
            .bind(&comment.ap_id)
            .bind(comment.removed)
            .bind(comment.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get all comments on a video, ordered by creation time.
pub async fn get_video_comments(
    pool: &Pool,
    video_id: i64,
) -> Result<Vec<VideoCommentRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<CommentTuple> = sqlx::query_as(
                &format!(
                    "SELECT {COMMENT_COLS} FROM video_comments \
                     WHERE video_id = $1 ORDER BY created_at ASC"
                ),
            )
            .bind(video_id)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_comment).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<CommentTuple> = sqlx::query_as(
                &format!(
                    "SELECT {COMMENT_COLS} FROM video_comments \
                     WHERE video_id = ?1 ORDER BY created_at ASC"
                ),
            )
            .bind(video_id)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_comment).collect())
        }
    }
}

/// Soft-remove a video comment (set removed = true).
pub async fn remove_video_comment(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE video_comments SET removed = true WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE video_comments SET removed = 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------------

/// Create a video playlist.
pub async fn create_playlist(
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
                "INSERT INTO video_playlists (id, user_id, title, description, visibility, created_at) \
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
                "INSERT INTO video_playlists (id, user_id, title, description, visibility, created_at) \
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

/// Get a playlist by ID.
/// Returns (id, user_id, title, description, visibility, created_at).
pub async fn get_playlist(
    pool: &Pool,
    id: i64,
) -> Result<Option<(i64, i64, String, String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM video_playlists WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, user_id, title, description, visibility, created_at \
                 FROM video_playlists WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(sq)
            .await
        }
    }
}

/// List playlists for a user.
/// Returns Vec<(id, title, visibility, created_at)>.
pub async fn list_playlists(
    pool: &Pool,
    user_id: i64,
) -> Result<Vec<(i64, String, String, i64)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT id, title, visibility, created_at FROM video_playlists \
                 WHERE user_id = $1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT id, title, visibility, created_at FROM video_playlists \
                 WHERE user_id = ?1 ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await
        }
    }
}

/// Add a video to a playlist. Silently replaces on duplicate.
pub async fn add_to_playlist(
    pool: &Pool,
    playlist_id: i64,
    video_id: i64,
    position: i32,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO video_playlist_items (playlist_id, video_id, position) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (playlist_id, video_id) DO UPDATE SET position = $3",
            )
            .bind(playlist_id)
            .bind(video_id)
            .bind(position)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO video_playlist_items (playlist_id, video_id, position) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT (playlist_id, video_id) DO UPDATE SET position = ?3",
            )
            .bind(playlist_id)
            .bind(video_id)
            .bind(position)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get video IDs in a playlist, ordered by position.
pub async fn playlist_items(pool: &Pool, playlist_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT video_id FROM video_playlist_items \
                 WHERE playlist_id = $1 ORDER BY position, video_id",
            )
            .bind(playlist_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT video_id FROM video_playlist_items \
                 WHERE playlist_id = ?1 ORDER BY position, video_id",
            )
            .bind(playlist_id)
            .fetch_all(sq)
            .await?
        }
    };
    Ok(rows.into_iter().map(|r| r.0).collect())
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
             VALUES (100, 1, 'channel_persona', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    fn sample_channel() -> ChannelRow {
        ChannelRow {
            persona_id: 100,
            description: "A video channel".into(),
            banner_media_id: None,
            created_at: 2000,
        }
    }

    fn sample_video(id: i64, ap_id: &str) -> VideoRow {
        VideoRow {
            id,
            channel_id: 100,
            user_id: 1,
            title: "My Video".into(),
            description: "A great video".into(),
            description_html: "<p>A great video</p>".into(),
            ap_id: ap_id.into(),
            duration: Some(120),
            original_file_path: Some("uploads/video.mp4".into()),
            original_file_size: Some(10_000_000),
            original_mime_type: Some("video/mp4".into()),
            thumbnail_path: None,
            preview_path: None,
            visibility: "public".into(),
            nsfw: false,
            language: Some("en".into()),
            state: "pending".into(),
            views: 0,
            likes: 0,
            dislikes: 0,
            created_at: 3000,
            published_at: None,
        }
    }

    fn sample_comment(id: i64, video_id: i64, ap_id: &str) -> VideoCommentRow {
        VideoCommentRow {
            id,
            video_id,
            parent_comment_id: None,
            author_persona_id: 101,
            content: "Great video!".into(),
            content_html: "<p>Great video!</p>".into(),
            ap_id: ap_id.into(),
            removed: false,
            created_at: 4000,
        }
    }

    fn sample_job(id: i64, video_id: i64, resolution: i32) -> TranscodingJobRow {
        TranscodingJobRow {
            id,
            video_id,
            resolution,
            state: "pending".into(),
            progress: 0,
            output_path: None,
            file_size: None,
            error: None,
            created_at: 5000,
            started_at: None,
            completed_at: None,
        }
    }

    // -- Channel CRUD --

    #[tokio::test]
    async fn create_and_get_channel() {
        let (pool, _) = setup().await;
        let ch = sample_channel();
        create_channel(&pool, &ch).await.unwrap();

        let fetched = get_channel(&pool, 100).await.unwrap().expect("should exist");
        assert_eq!(fetched.description, "A video channel");
        assert!(fetched.banner_media_id.is_none());
        assert_eq!(fetched.created_at, 2000);
    }

    #[tokio::test]
    async fn get_nonexistent_channel() {
        let (pool, _) = setup().await;
        assert!(get_channel(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_channels_by_user() {
        let (pool, sq) = setup().await;
        // Create a second persona for a second channel
        sqlx::raw_sql(
            "INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (200, 1, 'channel2', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();

        let mut ch1 = sample_channel();
        ch1.created_at = 2000;
        create_channel(&pool, &ch1).await.unwrap();

        let ch2 = ChannelRow {
            persona_id: 200,
            description: "Second channel".into(),
            banner_media_id: None,
            created_at: 3000,
        };
        create_channel(&pool, &ch2).await.unwrap();

        let channels = list_channels(&pool, 1).await.unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].persona_id, 200); // newer first
        assert_eq!(channels[1].persona_id, 100);

        // No channels for user 999
        let empty = list_channels(&pool, 999).await.unwrap();
        assert!(empty.is_empty());
    }

    // -- Video CRUD --

    #[tokio::test]
    async fn create_and_get_video() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();

        let v = sample_video(1, "https://example.com/video/1");
        create_video(&pool, &v).await.unwrap();

        let fetched = get_video(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "My Video");
        assert_eq!(fetched.channel_id, 100);
        assert_eq!(fetched.state, "pending");
        assert_eq!(fetched.views, 0);
        assert!(!fetched.nsfw);
        assert_eq!(fetched.duration, Some(120));
    }

    #[tokio::test]
    async fn get_nonexistent_video() {
        let (pool, _) = setup().await;
        assert!(get_video(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_videos_in_channel() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();

        let mut v1 = sample_video(1, "https://example.com/video/1");
        v1.created_at = 3000;
        let mut v2 = sample_video(2, "https://example.com/video/2");
        v2.title = "Second Video".into();
        v2.created_at = 4000;
        create_video(&pool, &v1).await.unwrap();
        create_video(&pool, &v2).await.unwrap();

        let videos = list_videos(&pool, 100, 10, 0).await.unwrap();
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].title, "Second Video"); // newer first
        assert_eq!(videos[1].title, "My Video");

        // Pagination
        let page = list_videos(&pool, 100, 1, 0).await.unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, 2);

        let page2 = list_videos(&pool, 100, 1, 1).await.unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].id, 1);
    }

    #[tokio::test]
    async fn list_published_videos_filters() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();

        let mut v1 = sample_video(1, "https://example.com/video/1");
        v1.state = "published".into();
        v1.visibility = "public".into();
        create_video(&pool, &v1).await.unwrap();

        let mut v2 = sample_video(2, "https://example.com/video/2");
        v2.state = "pending".into();
        create_video(&pool, &v2).await.unwrap();

        let mut v3 = sample_video(3, "https://example.com/video/3");
        v3.state = "published".into();
        v3.visibility = "private".into();
        create_video(&pool, &v3).await.unwrap();

        let published = list_published_videos(&pool, 10, 0).await.unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].id, 1);
    }

    #[tokio::test]
    async fn update_video_metadata_partial() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        update_video_metadata(&pool, 1, Some("New Title"), None, None)
            .await
            .unwrap();
        let fetched = get_video(&pool, 1).await.unwrap().unwrap();
        assert_eq!(fetched.title, "New Title");
        assert_eq!(fetched.description, "A great video"); // unchanged
    }

    #[tokio::test]
    async fn update_nonexistent_video_is_ok() {
        let (pool, _) = setup().await;
        update_video_metadata(&pool, 999, Some("X"), None, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn set_video_state_and_thumbnail() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        set_video_state(&pool, 1, "published").await.unwrap();
        let v = get_video(&pool, 1).await.unwrap().unwrap();
        assert_eq!(v.state, "published");

        set_video_thumbnail(&pool, 1, "thumbs/1.jpg").await.unwrap();
        let v = get_video(&pool, 1).await.unwrap().unwrap();
        assert_eq!(v.thumbnail_path.as_deref(), Some("thumbs/1.jpg"));
    }

    #[tokio::test]
    async fn increment_views_works() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        increment_views(&pool, 1).await.unwrap();
        increment_views(&pool, 1).await.unwrap();
        increment_views(&pool, 1).await.unwrap();
        let v = get_video(&pool, 1).await.unwrap().unwrap();
        assert_eq!(v.views, 3);
    }

    #[tokio::test]
    async fn delete_video_cascades() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        // Add related data
        create_job(&pool, &sample_job(1, 1, 720)).await.unwrap();
        add_video_file(&pool, 1, 720, "hls/720.m3u8", 5_000_000, Some(30), 6000)
            .await
            .unwrap();
        create_video_comment(
            &pool,
            &sample_comment(1, 1, "https://example.com/comment/1"),
        )
        .await
        .unwrap();

        delete_video(&pool, 1).await.unwrap();
        assert!(get_video(&pool, 1).await.unwrap().is_none());
        assert!(jobs_for_video(&pool, 1).await.unwrap().is_empty());
        assert!(video_files(&pool, 1).await.unwrap().is_empty());
        assert!(get_video_comments(&pool, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_video_is_ok() {
        let (pool, _) = setup().await;
        delete_video(&pool, 999).await.unwrap();
    }

    // -- Transcoding jobs --

    #[tokio::test]
    async fn job_lifecycle() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        let job = sample_job(1, 1, 720);
        create_job(&pool, &job).await.unwrap();

        // Should show up as pending
        let pending = get_pending_jobs(&pool, 10).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].resolution, 720);

        // Start the job
        start_job(&pool, 1, 6000).await.unwrap();
        let pending = get_pending_jobs(&pool, 10).await.unwrap();
        assert!(pending.is_empty());

        // Update progress
        update_progress(&pool, 1, 50).await.unwrap();

        // Complete the job
        complete_job(&pool, 1, "hls/720.m3u8", 5_000_000, 7000)
            .await
            .unwrap();

        let jobs = jobs_for_video(&pool, 1).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].state, "completed");
        assert_eq!(jobs[0].output_path.as_deref(), Some("hls/720.m3u8"));
        assert_eq!(jobs[0].file_size, Some(5_000_000));
        assert_eq!(jobs[0].started_at, Some(6000));
        assert_eq!(jobs[0].completed_at, Some(7000));
    }

    #[tokio::test]
    async fn job_failure() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        create_job(&pool, &sample_job(1, 1, 1080)).await.unwrap();
        start_job(&pool, 1, 6000).await.unwrap();
        fail_job(&pool, 1, "codec not supported", 7000)
            .await
            .unwrap();

        let jobs = jobs_for_video(&pool, 1).await.unwrap();
        assert_eq!(jobs[0].state, "failed");
        assert_eq!(jobs[0].error.as_deref(), Some("codec not supported"));
    }

    #[tokio::test]
    async fn multiple_resolutions() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        create_job(&pool, &sample_job(1, 1, 720)).await.unwrap();
        create_job(&pool, &sample_job(2, 1, 1080)).await.unwrap();
        create_job(&pool, &sample_job(3, 1, 480)).await.unwrap();

        let jobs = jobs_for_video(&pool, 1).await.unwrap();
        assert_eq!(jobs.len(), 3);
        // Ordered by resolution
        assert_eq!(jobs[0].resolution, 480);
        assert_eq!(jobs[1].resolution, 720);
        assert_eq!(jobs[2].resolution, 1080);
    }

    // -- Video files --

    #[tokio::test]
    async fn add_and_list_video_files() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        add_video_file(&pool, 1, 720, "hls/720.m3u8", 5_000_000, Some(30), 6000)
            .await
            .unwrap();
        add_video_file(&pool, 1, 1080, "hls/1080.m3u8", 10_000_000, Some(30), 6000)
            .await
            .unwrap();
        add_video_file(&pool, 1, 480, "hls/480.m3u8", 2_000_000, None, 6000)
            .await
            .unwrap();

        let files = video_files(&pool, 1).await.unwrap();
        assert_eq!(files.len(), 3);
        // Ordered by resolution
        assert_eq!(files[0].0, 480);
        assert_eq!(files[1].0, 720);
        assert_eq!(files[2].0, 1080);
        assert_eq!(files[0].1, "hls/480.m3u8");
        assert_eq!(files[0].3, None); // no fps
        assert_eq!(files[1].3, Some(30));
    }

    #[tokio::test]
    async fn add_video_file_upsert() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        add_video_file(&pool, 1, 720, "hls/720-old.m3u8", 5_000_000, Some(30), 6000)
            .await
            .unwrap();
        add_video_file(&pool, 1, 720, "hls/720-new.m3u8", 4_500_000, Some(24), 7000)
            .await
            .unwrap();

        let files = video_files(&pool, 1).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, "hls/720-new.m3u8");
        assert_eq!(files[0].2, 4_500_000);
    }

    #[tokio::test]
    async fn video_files_empty() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        let files = video_files(&pool, 1).await.unwrap();
        assert!(files.is_empty());
    }

    // -- Comments --

    #[tokio::test]
    async fn create_and_get_comments() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        let c = sample_comment(1, 1, "https://example.com/comment/1");
        create_video_comment(&pool, &c).await.unwrap();

        let comments = get_video_comments(&pool, 1).await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content, "Great video!");
        assert!(comments[0].parent_comment_id.is_none());
    }

    #[tokio::test]
    async fn threaded_comments() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();

        let c1 = sample_comment(1, 1, "https://example.com/comment/1");
        create_video_comment(&pool, &c1).await.unwrap();

        let mut c2 = sample_comment(2, 1, "https://example.com/comment/2");
        c2.parent_comment_id = Some(1);
        c2.content = "Reply!".into();
        c2.content_html = "<p>Reply!</p>".into();
        c2.created_at = 5000;
        create_video_comment(&pool, &c2).await.unwrap();

        let comments = get_video_comments(&pool, 1).await.unwrap();
        assert_eq!(comments.len(), 2);
        assert!(comments[0].parent_comment_id.is_none());
        assert_eq!(comments[1].parent_comment_id, Some(1));
    }

    #[tokio::test]
    async fn remove_comment_sets_flag() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();
        create_video_comment(
            &pool,
            &sample_comment(1, 1, "https://example.com/comment/1"),
        )
        .await
        .unwrap();

        remove_video_comment(&pool, 1).await.unwrap();
        let comments = get_video_comments(&pool, 1).await.unwrap();
        assert!(comments[0].removed);
    }

    // -- Playlists --

    #[tokio::test]
    async fn create_and_get_playlist() {
        let (pool, _) = setup().await;
        create_playlist(&pool, 1, 1, "Favorites", "My favorites", "public", 5000)
            .await
            .unwrap();

        let pl = get_playlist(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(pl.0, 1);
        assert_eq!(pl.1, 1);
        assert_eq!(pl.2, "Favorites");
        assert_eq!(pl.3, "My favorites");
        assert_eq!(pl.4, "public");
        assert_eq!(pl.5, 5000);
    }

    #[tokio::test]
    async fn get_nonexistent_playlist() {
        let (pool, _) = setup().await;
        assert!(get_playlist(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_playlists_for_user() {
        let (pool, _) = setup().await;
        create_playlist(&pool, 1, 1, "First", "", "public", 5000)
            .await
            .unwrap();
        create_playlist(&pool, 2, 1, "Second", "", "private", 6000)
            .await
            .unwrap();

        let playlists = list_playlists(&pool, 1).await.unwrap();
        assert_eq!(playlists.len(), 2);
        assert_eq!(playlists[0].1, "Second"); // newer first
        assert_eq!(playlists[1].1, "First");

        let empty = list_playlists(&pool, 999).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn playlist_items_ordered() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();
        create_video(&pool, &sample_video(2, "https://example.com/video/2"))
            .await
            .unwrap();
        create_video(&pool, &sample_video(3, "https://example.com/video/3"))
            .await
            .unwrap();

        create_playlist(&pool, 1, 1, "Watch Later", "", "public", 5000)
            .await
            .unwrap();
        add_to_playlist(&pool, 1, 3, 0).await.unwrap();
        add_to_playlist(&pool, 1, 1, 1).await.unwrap();
        add_to_playlist(&pool, 1, 2, 2).await.unwrap();

        let items = playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![3, 1, 2]);
    }

    #[tokio::test]
    async fn add_to_playlist_upsert() {
        let (pool, _) = setup().await;
        create_channel(&pool, &sample_channel()).await.unwrap();
        create_video(&pool, &sample_video(1, "https://example.com/video/1"))
            .await
            .unwrap();
        create_video(&pool, &sample_video(2, "https://example.com/video/2"))
            .await
            .unwrap();

        create_playlist(&pool, 1, 1, "My List", "", "public", 5000)
            .await
            .unwrap();
        add_to_playlist(&pool, 1, 1, 0).await.unwrap();
        add_to_playlist(&pool, 1, 2, 1).await.unwrap();
        // Move video 1 to position 5
        add_to_playlist(&pool, 1, 1, 5).await.unwrap();

        let items = playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![2, 1]); // 2 at pos 1, 1 at pos 5
    }

    #[tokio::test]
    async fn playlist_items_empty() {
        let (pool, _) = setup().await;
        create_playlist(&pool, 1, 1, "Empty", "", "public", 5000)
            .await
            .unwrap();
        let items = playlist_items(&pool, 1).await.unwrap();
        assert!(items.is_empty());
    }

    // -- Full lifecycle --

    #[tokio::test]
    async fn full_lifecycle() {
        let (pool, _) = setup().await;

        // Create channel
        create_channel(&pool, &sample_channel()).await.unwrap();

        // Upload video
        let v = sample_video(1, "https://example.com/video/1");
        create_video(&pool, &v).await.unwrap();

        // Create transcoding jobs
        create_job(&pool, &sample_job(1, 1, 720)).await.unwrap();
        create_job(&pool, &sample_job(2, 1, 1080)).await.unwrap();

        // Process jobs
        let pending = get_pending_jobs(&pool, 10).await.unwrap();
        assert_eq!(pending.len(), 2);

        start_job(&pool, 1, 6000).await.unwrap();
        update_progress(&pool, 1, 50).await.unwrap();
        complete_job(&pool, 1, "hls/720.m3u8", 5_000_000, 7000)
            .await
            .unwrap();

        start_job(&pool, 2, 6000).await.unwrap();
        complete_job(&pool, 2, "hls/1080.m3u8", 10_000_000, 8000)
            .await
            .unwrap();

        // Register HLS files
        add_video_file(&pool, 1, 720, "hls/720.m3u8", 5_000_000, Some(30), 7000)
            .await
            .unwrap();
        add_video_file(&pool, 1, 1080, "hls/1080.m3u8", 10_000_000, Some(30), 8000)
            .await
            .unwrap();

        // Publish
        set_video_state(&pool, 1, "published").await.unwrap();
        set_video_thumbnail(&pool, 1, "thumbs/1.jpg").await.unwrap();

        // View it
        increment_views(&pool, 1).await.unwrap();
        increment_views(&pool, 1).await.unwrap();
        let v = get_video(&pool, 1).await.unwrap().unwrap();
        assert_eq!(v.state, "published");
        assert_eq!(v.views, 2);

        // Comment
        create_video_comment(
            &pool,
            &sample_comment(1, 1, "https://example.com/comment/1"),
        )
        .await
        .unwrap();

        // Add to playlist
        create_playlist(&pool, 1, 1, "Best Of", "", "public", 9000)
            .await
            .unwrap();
        add_to_playlist(&pool, 1, 1, 0).await.unwrap();

        let items = playlist_items(&pool, 1).await.unwrap();
        assert_eq!(items, vec![1]);

        // Verify listing
        let published = list_published_videos(&pool, 10, 0).await.unwrap();
        assert_eq!(published.len(), 1);

        let files = video_files(&pool, 1).await.unwrap();
        assert_eq!(files.len(), 2);
    }
}
