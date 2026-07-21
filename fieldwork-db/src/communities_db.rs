//! Database operations for Lemmy-style communities.
//!
//! Communities are AP Group actors backed by a persona. This module provides
//! CRUD for community metadata, membership, posts (titled link/text entries),
//! threaded comments, voting, and bans.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A row from the `communities` table.
#[derive(Debug, Clone)]
pub struct CommunityRow {
    pub persona_id: i64,
    pub title: String,
    pub sidebar_html: String,
    pub description: String,
    pub nsfw: bool,
    pub posting_restricted: bool,
    pub created_at: i64,
}

type CommunityTuple = (i64, String, String, String, bool, bool, i64);

fn tuple_to_community(t: CommunityTuple) -> CommunityRow {
    CommunityRow {
        persona_id: t.0,
        title: t.1,
        sidebar_html: t.2,
        description: t.3,
        nsfw: t.4,
        posting_restricted: t.5,
        created_at: t.6,
    }
}

/// A row from the `community_posts` table.
#[derive(Debug, Clone)]
pub struct CommunityPostRow {
    pub id: i64,
    pub community_id: i64,
    pub author_persona_id: i64,
    pub author_user_id: i64,
    pub title: String,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub url: Option<String>,
    pub ap_id: String,
    pub locked: bool,
    pub pinned: bool,
    pub removed: bool,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

type PostTuple = (
    i64, i64, i64, i64,
    String, Option<String>, Option<String>, Option<String>,
    String, bool, bool, bool,
    i64, Option<i64>,
);

fn tuple_to_post(t: PostTuple) -> CommunityPostRow {
    CommunityPostRow {
        id: t.0,
        community_id: t.1,
        author_persona_id: t.2,
        author_user_id: t.3,
        title: t.4,
        body: t.5,
        body_html: t.6,
        url: t.7,
        ap_id: t.8,
        locked: t.9,
        pinned: t.10,
        removed: t.11,
        created_at: t.12,
        updated_at: t.13,
    }
}

const POST_COLS: &str = "\
    id, community_id, author_persona_id, author_user_id, \
    title, body, body_html, url, \
    ap_id, locked, pinned, removed, \
    created_at, updated_at";

/// A row from the `community_comments` table.
#[derive(Debug, Clone)]
pub struct CommentRow {
    pub id: i64,
    pub post_id: i64,
    pub parent_comment_id: Option<i64>,
    pub author_persona_id: i64,
    pub author_user_id: i64,
    pub content: String,
    pub content_html: String,
    pub ap_id: String,
    pub removed: bool,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

type CommentTuple = (
    i64, i64, Option<i64>, i64, i64,
    String, String, String, bool,
    i64, Option<i64>,
);

fn tuple_to_comment(t: CommentTuple) -> CommentRow {
    CommentRow {
        id: t.0,
        post_id: t.1,
        parent_comment_id: t.2,
        author_persona_id: t.3,
        author_user_id: t.4,
        content: t.5,
        content_html: t.6,
        ap_id: t.7,
        removed: t.8,
        created_at: t.9,
        updated_at: t.10,
    }
}

const COMMENT_COLS: &str = "\
    id, post_id, parent_comment_id, author_persona_id, author_user_id, \
    content, content_html, ap_id, removed, \
    created_at, updated_at";

// ---------------------------------------------------------------------------
// Community CRUD
// ---------------------------------------------------------------------------

/// Create a new community (persona must already exist).
pub async fn create_community(pool: &Pool, row: &CommunityRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO communities \
                 (persona_id, title, sidebar_html, description, nsfw, posting_restricted, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(row.persona_id)
            .bind(&row.title)
            .bind(&row.sidebar_html)
            .bind(&row.description)
            .bind(row.nsfw)
            .bind(row.posting_restricted)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO communities \
                 (persona_id, title, sidebar_html, description, nsfw, posting_restricted, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(row.persona_id)
            .bind(&row.title)
            .bind(&row.sidebar_html)
            .bind(&row.description)
            .bind(row.nsfw)
            .bind(row.posting_restricted)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a community by persona_id.
pub async fn get_community(pool: &Pool, persona_id: i64) -> Result<Option<CommunityRow>, sqlx::Error> {
    let cols = "persona_id, title, sidebar_html, description, nsfw, posting_restricted, created_at";
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<CommunityTuple> = sqlx::query_as(
                &format!("SELECT {cols} FROM communities WHERE persona_id = $1"),
            )
            .bind(persona_id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_community))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<CommunityTuple> = sqlx::query_as(
                &format!("SELECT {cols} FROM communities WHERE persona_id = ?1"),
            )
            .bind(persona_id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_community))
        }
    }
}

/// List communities ordered by creation time descending.
pub async fn list_communities(pool: &Pool, limit: i64) -> Result<Vec<CommunityRow>, sqlx::Error> {
    let cols = "persona_id, title, sidebar_html, description, nsfw, posting_restricted, created_at";
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<CommunityTuple> = sqlx::query_as(
                &format!("SELECT {cols} FROM communities ORDER BY created_at DESC LIMIT $1"),
            )
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_community).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<CommunityTuple> = sqlx::query_as(
                &format!("SELECT {cols} FROM communities ORDER BY created_at DESC LIMIT ?1"),
            )
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_community).collect())
        }
    }
}

/// Update a community's title, sidebar, and/or description.
pub async fn update_community(
    pool: &Pool,
    persona_id: i64,
    title: Option<&str>,
    sidebar: Option<&str>,
    description: Option<&str>,
) -> Result<(), sqlx::Error> {
    let current = match get_community(pool, persona_id).await? {
        Some(c) => c,
        None => return Ok(()),
    };
    let new_title = title.unwrap_or(&current.title);
    let new_sidebar = sidebar.unwrap_or(&current.sidebar_html);
    let new_desc = description.unwrap_or(&current.description);

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE communities SET title = $1, sidebar_html = $2, description = $3 \
                 WHERE persona_id = $4",
            )
            .bind(new_title)
            .bind(new_sidebar)
            .bind(new_desc)
            .bind(persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE communities SET title = ?1, sidebar_html = ?2, description = ?3 \
                 WHERE persona_id = ?4",
            )
            .bind(new_title)
            .bind(new_sidebar)
            .bind(new_desc)
            .bind(persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Membership
// ---------------------------------------------------------------------------

/// Join a community as a member.
pub async fn join_community(
    pool: &Pool,
    community_id: i64,
    user_id: i64,
    persona_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO community_members (community_id, user_id, persona_id, role, created_at) \
                 VALUES ($1, $2, $3, 'member', $4) \
                 ON CONFLICT (community_id, persona_id) DO NOTHING",
            )
            .bind(community_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO community_members (community_id, user_id, persona_id, role, created_at) \
                 VALUES (?1, ?2, ?3, 'member', ?4) \
                 ON CONFLICT (community_id, persona_id) DO NOTHING",
            )
            .bind(community_id)
            .bind(user_id)
            .bind(persona_id)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Leave a community.
pub async fn leave_community(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM community_members WHERE community_id = $1 AND persona_id = $2",
            )
            .bind(community_id)
            .bind(persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM community_members WHERE community_id = ?1 AND persona_id = ?2",
            )
            .bind(community_id)
            .bind(persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Set a member's role (member/mod/admin).
pub async fn set_role(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
    role: &str,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE community_members SET role = $1 \
                 WHERE community_id = $2 AND persona_id = $3",
            )
            .bind(role)
            .bind(community_id)
            .bind(persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE community_members SET role = ?1 \
                 WHERE community_id = ?2 AND persona_id = ?3",
            )
            .bind(role)
            .bind(community_id)
            .bind(persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Count members in a community.
pub async fn member_count(pool: &Pool, community_id: i64) -> Result<i64, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT COUNT(*) FROM community_members WHERE community_id = $1")
                .bind(community_id)
                .fetch_one(pg)
                .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT COUNT(*) FROM community_members WHERE community_id = ?1")
                .bind(community_id)
                .fetch_one(sq)
                .await?
        }
    };
    Ok(row.0)
}

/// Check if a persona is a member of a community.
pub async fn is_member(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_members \
                 WHERE community_id = $1 AND persona_id = $2",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_members \
                 WHERE community_id = ?1 AND persona_id = ?2",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0 > 0)
}

/// Check if a persona is a moderator (role = 'mod' or 'admin') in a community.
pub async fn is_moderator(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_members \
                 WHERE community_id = $1 AND persona_id = $2 AND role IN ('mod', 'admin')",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_members \
                 WHERE community_id = ?1 AND persona_id = ?2 AND role IN ('mod', 'admin')",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0 > 0)
}

// ---------------------------------------------------------------------------
// Posts
// ---------------------------------------------------------------------------

/// Create a community post.
pub async fn create_post(pool: &Pool, post: &CommunityPostRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO community_posts ({POST_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
                ),
            )
            .bind(post.id)
            .bind(post.community_id)
            .bind(post.author_persona_id)
            .bind(post.author_user_id)
            .bind(&post.title)
            .bind(&post.body)
            .bind(&post.body_html)
            .bind(&post.url)
            .bind(&post.ap_id)
            .bind(post.locked)
            .bind(post.pinned)
            .bind(post.removed)
            .bind(post.created_at)
            .bind(post.updated_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO community_posts ({POST_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
                ),
            )
            .bind(post.id)
            .bind(post.community_id)
            .bind(post.author_persona_id)
            .bind(post.author_user_id)
            .bind(&post.title)
            .bind(&post.body)
            .bind(&post.body_html)
            .bind(&post.url)
            .bind(&post.ap_id)
            .bind(post.locked)
            .bind(post.pinned)
            .bind(post.removed)
            .bind(post.created_at)
            .bind(post.updated_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a community post by ID.
pub async fn get_post(pool: &Pool, id: i64) -> Result<Option<CommunityPostRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<PostTuple> = sqlx::query_as(
                &format!("SELECT {POST_COLS} FROM community_posts WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_post))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<PostTuple> = sqlx::query_as(
                &format!("SELECT {POST_COLS} FROM community_posts WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_post))
        }
    }
}

/// List posts in a community with sort order and pagination.
///
/// Sort options: "new" (newest first), "top" (highest score), "hot" (score
/// decayed by age). Falls back to "new" for unrecognized sort values.
pub async fn list_posts(
    pool: &Pool,
    community_id: i64,
    sort: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<CommunityPostRow>, sqlx::Error> {
    // ponytail: hot sort uses a simple score/age formula in SQL rather than
    // precomputed ranks. Ceiling: slow past ~10k posts per community; upgrade
    // path is a materialized hot_rank column refreshed by a background job.
    let order = match sort {
        "top" => "ORDER BY (SELECT COALESCE(SUM(score), 0) FROM votes WHERE post_id = community_posts.id) DESC, created_at DESC".to_string(),
        "hot" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            format!(
                "ORDER BY (COALESCE((SELECT SUM(score) FROM votes WHERE post_id = community_posts.id), 0) + 1) * 1000000 \
                 / ({now} - created_at + 3600) DESC, created_at DESC"
            )
        }
        _ => "ORDER BY created_at DESC".to_string(),
    };

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {POST_COLS} FROM community_posts \
                 WHERE community_id = $1 AND removed = false \
                 {order} LIMIT $2 OFFSET $3"
            );
            let rows: Vec<PostTuple> = sqlx::query_as(&sql)
                .bind(community_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(tuple_to_post).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {POST_COLS} FROM community_posts \
                 WHERE community_id = ?1 AND removed = 0 \
                 {order} LIMIT ?2 OFFSET ?3"
            );
            let rows: Vec<PostTuple> = sqlx::query_as(&sql)
                .bind(community_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(tuple_to_post).collect())
        }
    }
}

/// Pin a post (set pinned = true).
pub async fn pin_post(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE community_posts SET pinned = true WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE community_posts SET pinned = 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Lock a post (set locked = true).
pub async fn lock_post(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE community_posts SET locked = true WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE community_posts SET locked = 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Soft-remove a post (set removed = true).
pub async fn remove_post(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE community_posts SET removed = true WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE community_posts SET removed = 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------------

/// Create a comment on a community post.
pub async fn create_comment(pool: &Pool, comment: &CommentRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                &format!(
                    "INSERT INTO community_comments ({COMMENT_COLS}) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                ),
            )
            .bind(comment.id)
            .bind(comment.post_id)
            .bind(comment.parent_comment_id)
            .bind(comment.author_persona_id)
            .bind(comment.author_user_id)
            .bind(&comment.content)
            .bind(&comment.content_html)
            .bind(&comment.ap_id)
            .bind(comment.removed)
            .bind(comment.created_at)
            .bind(comment.updated_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                &format!(
                    "INSERT INTO community_comments ({COMMENT_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
                ),
            )
            .bind(comment.id)
            .bind(comment.post_id)
            .bind(comment.parent_comment_id)
            .bind(comment.author_persona_id)
            .bind(comment.author_user_id)
            .bind(&comment.content)
            .bind(&comment.content_html)
            .bind(&comment.ap_id)
            .bind(comment.removed)
            .bind(comment.created_at)
            .bind(comment.updated_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a comment by ID.
pub async fn get_comment(pool: &Pool, id: i64) -> Result<Option<CommentRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<CommentTuple> = sqlx::query_as(
                &format!("SELECT {COMMENT_COLS} FROM community_comments WHERE id = $1"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_comment))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<CommentTuple> = sqlx::query_as(
                &format!("SELECT {COMMENT_COLS} FROM community_comments WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_comment))
        }
    }
}

/// Get all comments on a post as a flat list. Caller builds thread tree.
///
/// Sort options: "new" (newest first), "old" (oldest first, default),
/// "top" (highest score first).
pub async fn get_comments(
    pool: &Pool,
    post_id: i64,
    sort: &str,
) -> Result<Vec<CommentRow>, sqlx::Error> {
    let order = match sort {
        "new" => "ORDER BY created_at DESC",
        "top" => "ORDER BY (SELECT COALESCE(SUM(score), 0) FROM votes WHERE comment_id = community_comments.id) DESC, created_at ASC",
        _ => "ORDER BY created_at ASC",
    };

    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let sql = format!(
                "SELECT {COMMENT_COLS} FROM community_comments WHERE post_id = $1 {order}"
            );
            let rows: Vec<CommentTuple> = sqlx::query_as(&sql)
                .bind(post_id)
                .fetch_all(pg)
                .await?;
            Ok(rows.into_iter().map(tuple_to_comment).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let sql = format!(
                "SELECT {COMMENT_COLS} FROM community_comments WHERE post_id = ?1 {order}"
            );
            let rows: Vec<CommentTuple> = sqlx::query_as(&sql)
                .bind(post_id)
                .fetch_all(sq)
                .await?;
            Ok(rows.into_iter().map(tuple_to_comment).collect())
        }
    }
}

/// Soft-remove a comment (set removed = true).
pub async fn remove_comment(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("UPDATE community_comments SET removed = true WHERE id = $1")
                .bind(id)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("UPDATE community_comments SET removed = 1 WHERE id = ?1")
                .bind(id)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Voting
// ---------------------------------------------------------------------------

/// Vote on a post (upsert). Score must be -1 or 1.
pub async fn vote_post(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    post_id: i64,
    score: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO votes (user_id, persona_id, post_id, score, created_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (persona_id, post_id) WHERE post_id IS NOT NULL \
                 DO UPDATE SET score = $4",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(score)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            // SQLite partial-index upsert: delete-then-insert is simpler and
            // avoids ON CONFLICT targeting a partial unique index, which SQLite
            // handles inconsistently across versions.
            sqlx::query(
                "DELETE FROM votes WHERE persona_id = ?1 AND post_id = ?2",
            )
            .bind(persona_id)
            .bind(post_id)
            .execute(&*sq)
            .await?;
            sqlx::query(
                "INSERT INTO votes (user_id, persona_id, post_id, score, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(post_id)
            .bind(score)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Vote on a comment (upsert). Score must be -1 or 1.
pub async fn vote_comment(
    pool: &Pool,
    user_id: i64,
    persona_id: i64,
    comment_id: i64,
    score: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO votes (user_id, persona_id, comment_id, score, created_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (persona_id, comment_id) WHERE comment_id IS NOT NULL \
                 DO UPDATE SET score = $4",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(comment_id)
            .bind(score)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM votes WHERE persona_id = ?1 AND comment_id = ?2",
            )
            .bind(persona_id)
            .bind(comment_id)
            .execute(&*sq)
            .await?;
            sqlx::query(
                "INSERT INTO votes (user_id, persona_id, comment_id, score, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(user_id)
            .bind(persona_id)
            .bind(comment_id)
            .bind(score)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get the total score (SUM of votes) for a post.
pub async fn post_score(pool: &Pool, post_id: i64) -> Result<i64, sqlx::Error> {
    let row: (Option<i64>,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT SUM(score) FROM votes WHERE post_id = $1")
                .bind(post_id)
                .fetch_one(pg)
                .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT SUM(score) FROM votes WHERE post_id = ?1")
                .bind(post_id)
                .fetch_one(sq)
                .await?
        }
    };
    Ok(row.0.unwrap_or(0))
}

/// Get the total score (SUM of votes) for a comment.
pub async fn comment_score(pool: &Pool, comment_id: i64) -> Result<i64, sqlx::Error> {
    let row: (Option<i64>,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as("SELECT SUM(score) FROM votes WHERE comment_id = $1")
                .bind(comment_id)
                .fetch_one(pg)
                .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as("SELECT SUM(score) FROM votes WHERE comment_id = ?1")
                .bind(comment_id)
                .fetch_one(sq)
                .await?
        }
    };
    Ok(row.0.unwrap_or(0))
}

/// Get the current user's vote on a post, if any.
pub async fn user_vote_on_post(
    pool: &Pool,
    persona_id: i64,
    post_id: i64,
) -> Result<Option<i32>, sqlx::Error> {
    let row: Option<(i32,)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT score FROM votes WHERE persona_id = $1 AND post_id = $2",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT score FROM votes WHERE persona_id = ?1 AND post_id = ?2",
            )
            .bind(persona_id)
            .bind(post_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row.map(|r| r.0))
}

// ---------------------------------------------------------------------------
// Bans
// ---------------------------------------------------------------------------

/// Ban a persona from a community.
pub async fn ban_from_community(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
    reason: &str,
    expires: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO community_bans (community_id, persona_id, reason, expires_at, created_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (community_id, persona_id) DO UPDATE SET reason = $3, expires_at = $4",
            )
            .bind(community_id)
            .bind(persona_id)
            .bind(reason)
            .bind(expires)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO community_bans (community_id, persona_id, reason, expires_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT (community_id, persona_id) DO UPDATE SET reason = ?3, expires_at = ?4",
            )
            .bind(community_id)
            .bind(persona_id)
            .bind(reason)
            .bind(expires)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Unban a persona from a community.
pub async fn unban_from_community(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "DELETE FROM community_bans WHERE community_id = $1 AND persona_id = $2",
            )
            .bind(community_id)
            .bind(persona_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "DELETE FROM community_bans WHERE community_id = ?1 AND persona_id = ?2",
            )
            .bind(community_id)
            .bind(persona_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Check if a persona is banned from a community.
pub async fn is_banned(
    pool: &Pool,
    community_id: i64,
    persona_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: (i64,) = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_bans \
                 WHERE community_id = $1 AND persona_id = $2",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM community_bans \
                 WHERE community_id = ?1 AND persona_id = ?2",
            )
            .bind(community_id)
            .bind(persona_id)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(row.0 > 0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();
        // Seed a user and persona for FK constraints
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (100, 1, 'community_persona', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (102, 1, 'bob', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_community() -> CommunityRow {
        CommunityRow {
            persona_id: 100,
            title: "Rust".into(),
            sidebar_html: "<p>Welcome</p>".into(),
            description: "All things Rust".into(),
            nsfw: false,
            posting_restricted: false,
            created_at: 2000,
        }
    }

    fn sample_post(id: i64, ap_id: &str) -> CommunityPostRow {
        CommunityPostRow {
            id,
            community_id: 100,
            author_persona_id: 101,
            author_user_id: 1,
            title: "Hello World".into(),
            body: Some("First post!".into()),
            body_html: Some("<p>First post!</p>".into()),
            url: None,
            ap_id: ap_id.into(),
            locked: false,
            pinned: false,
            removed: false,
            created_at: 3000,
            updated_at: None,
        }
    }

    fn sample_comment(id: i64, post_id: i64, ap_id: &str) -> CommentRow {
        CommentRow {
            id,
            post_id,
            parent_comment_id: None,
            author_persona_id: 102,
            author_user_id: 1,
            content: "Great post!".into(),
            content_html: "<p>Great post!</p>".into(),
            ap_id: ap_id.into(),
            removed: false,
            created_at: 4000,
            updated_at: None,
        }
    }

    // -- Community CRUD --

    #[tokio::test]
    async fn create_and_get_community() {
        let pool = setup().await;
        let c = sample_community();
        create_community(&pool, &c).await.unwrap();

        let fetched = get_community(&pool, 100).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "Rust");
        assert_eq!(fetched.sidebar_html, "<p>Welcome</p>");
        assert_eq!(fetched.description, "All things Rust");
        assert!(!fetched.nsfw);
        assert!(!fetched.posting_restricted);
    }

    #[tokio::test]
    async fn get_nonexistent_community() {
        let pool = setup().await;
        assert!(get_community(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_communities_ordered() {
        let pool = setup().await;
        let mut c1 = sample_community();
        c1.created_at = 2000;
        create_community(&pool, &c1).await.unwrap();

        // Need another persona for second community
        let Pool::Sqlite(sq) = &pool;
        sqlx::raw_sql(
            "INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (200, 1, 'community2', 'pk', 'pub', 1000);",
        )
        .execute(sq)
        .await
        .unwrap();
        let mut c2 = sample_community();
        c2.persona_id = 200;
        c2.title = "Go".into();
        c2.created_at = 3000;
        create_community(&pool, &c2).await.unwrap();

        let list = list_communities(&pool, 10).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "Go"); // newer first
        assert_eq!(list[1].title, "Rust");
    }

    #[tokio::test]
    async fn update_community_partial() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        update_community(&pool, 100, Some("Rust Lang"), None, None)
            .await
            .unwrap();
        let fetched = get_community(&pool, 100).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Rust Lang");
        assert_eq!(fetched.sidebar_html, "<p>Welcome</p>"); // unchanged
    }

    #[tokio::test]
    async fn update_nonexistent_community_is_ok() {
        let pool = setup().await;
        update_community(&pool, 999, Some("X"), None, None)
            .await
            .unwrap();
    }

    // -- Membership --

    #[tokio::test]
    async fn join_and_membership() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        join_community(&pool, 100, 1, 101, 2000).await.unwrap();
        assert!(is_member(&pool, 100, 101).await.unwrap());
        assert!(!is_moderator(&pool, 100, 101).await.unwrap());
        assert_eq!(member_count(&pool, 100).await.unwrap(), 1);

        // Joining again is idempotent
        join_community(&pool, 100, 1, 101, 2001).await.unwrap();
        assert_eq!(member_count(&pool, 100).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn leave_community_removes_membership() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        join_community(&pool, 100, 1, 101, 2000).await.unwrap();
        leave_community(&pool, 100, 101).await.unwrap();
        assert!(!is_member(&pool, 100, 101).await.unwrap());
        assert_eq!(member_count(&pool, 100).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn set_role_to_mod() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        join_community(&pool, 100, 1, 101, 2000).await.unwrap();

        set_role(&pool, 100, 101, "mod").await.unwrap();
        assert!(is_moderator(&pool, 100, 101).await.unwrap());

        set_role(&pool, 100, 101, "member").await.unwrap();
        assert!(!is_moderator(&pool, 100, 101).await.unwrap());
    }

    #[tokio::test]
    async fn admin_is_also_moderator() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        join_community(&pool, 100, 1, 101, 2000).await.unwrap();
        set_role(&pool, 100, 101, "admin").await.unwrap();
        assert!(is_moderator(&pool, 100, 101).await.unwrap());
    }

    #[tokio::test]
    async fn not_a_member() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        assert!(!is_member(&pool, 100, 101).await.unwrap());
        assert!(!is_moderator(&pool, 100, 101).await.unwrap());
        assert_eq!(member_count(&pool, 100).await.unwrap(), 0);
    }

    // -- Posts --

    #[tokio::test]
    async fn create_and_get_post() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        let p = sample_post(1, "https://example.com/post/1");
        create_post(&pool, &p).await.unwrap();

        let fetched = get_post(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.title, "Hello World");
        assert_eq!(fetched.body.as_deref(), Some("First post!"));
        assert!(!fetched.locked);
        assert!(!fetched.pinned);
        assert!(!fetched.removed);
    }

    #[tokio::test]
    async fn get_nonexistent_post() {
        let pool = setup().await;
        assert!(get_post(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_posts_new_sort() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        let mut p1 = sample_post(1, "https://example.com/post/1");
        p1.created_at = 3000;
        let mut p2 = sample_post(2, "https://example.com/post/2");
        p2.created_at = 4000;
        p2.title = "Second".into();
        create_post(&pool, &p1).await.unwrap();
        create_post(&pool, &p2).await.unwrap();

        let posts = list_posts(&pool, 100, "new", 10, 0).await.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Second"); // newer first
        assert_eq!(posts[1].title, "Hello World");
    }

    #[tokio::test]
    async fn list_posts_excludes_removed() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        let p = sample_post(1, "https://example.com/post/1");
        create_post(&pool, &p).await.unwrap();
        remove_post(&pool, 1).await.unwrap();

        let posts = list_posts(&pool, 100, "new", 10, 0).await.unwrap();
        assert!(posts.is_empty());
    }

    #[tokio::test]
    async fn list_posts_pagination() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        for i in 1..=5 {
            let mut p = sample_post(i, &format!("https://example.com/post/{i}"));
            p.created_at = 3000 + i;
            create_post(&pool, &p).await.unwrap();
        }

        let page1 = list_posts(&pool, 100, "new", 2, 0).await.unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].id, 5);
        assert_eq!(page1[1].id, 4);

        let page2 = list_posts(&pool, 100, "new", 2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].id, 3);
    }

    #[tokio::test]
    async fn pin_and_lock_post() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        pin_post(&pool, 1).await.unwrap();
        let fetched = get_post(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.pinned);
        assert!(!fetched.locked);

        lock_post(&pool, 1).await.unwrap();
        let fetched = get_post(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.locked);
    }

    #[tokio::test]
    async fn remove_post_sets_flag() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();
        remove_post(&pool, 1).await.unwrap();
        let fetched = get_post(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.removed);
    }

    // -- Comments --

    #[tokio::test]
    async fn create_and_get_comment() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        let c = sample_comment(1, 1, "https://example.com/comment/1");
        create_comment(&pool, &c).await.unwrap();

        let fetched = get_comment(&pool, 1).await.unwrap().expect("should exist");
        assert_eq!(fetched.content, "Great post!");
        assert!(fetched.parent_comment_id.is_none());
    }

    #[tokio::test]
    async fn threaded_comments() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        let c1 = sample_comment(1, 1, "https://example.com/comment/1");
        create_comment(&pool, &c1).await.unwrap();

        let mut c2 = sample_comment(2, 1, "https://example.com/comment/2");
        c2.parent_comment_id = Some(1);
        c2.content = "Reply!".into();
        c2.content_html = "<p>Reply!</p>".into();
        c2.created_at = 5000;
        create_comment(&pool, &c2).await.unwrap();

        let comments = get_comments(&pool, 1, "old").await.unwrap();
        assert_eq!(comments.len(), 2);
        assert!(comments[0].parent_comment_id.is_none());
        assert_eq!(comments[1].parent_comment_id, Some(1));
    }

    #[tokio::test]
    async fn get_comments_new_sort() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        let mut c1 = sample_comment(1, 1, "https://example.com/comment/1");
        c1.created_at = 4000;
        create_comment(&pool, &c1).await.unwrap();

        let mut c2 = sample_comment(2, 1, "https://example.com/comment/2");
        c2.created_at = 5000;
        create_comment(&pool, &c2).await.unwrap();

        let comments = get_comments(&pool, 1, "new").await.unwrap();
        assert_eq!(comments[0].id, 2); // newer first
        assert_eq!(comments[1].id, 1);
    }

    #[tokio::test]
    async fn remove_comment_sets_flag() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();
        create_comment(&pool, &sample_comment(1, 1, "https://example.com/comment/1"))
            .await
            .unwrap();

        remove_comment(&pool, 1).await.unwrap();
        let fetched = get_comment(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.removed);
    }

    // -- Voting --

    #[tokio::test]
    async fn vote_post_and_score() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        vote_post(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        assert_eq!(post_score(&pool, 1).await.unwrap(), 1);

        // Second user downvotes
        vote_post(&pool, 1, 102, 1, -1, 5001).await.unwrap();
        assert_eq!(post_score(&pool, 1).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn vote_post_upsert_changes_vote() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        vote_post(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        assert_eq!(post_score(&pool, 1).await.unwrap(), 1);

        // Change vote to downvote
        vote_post(&pool, 1, 101, 1, -1, 5001).await.unwrap();
        assert_eq!(post_score(&pool, 1).await.unwrap(), -1);
    }

    #[tokio::test]
    async fn user_vote_on_post_returns_current() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();

        assert!(user_vote_on_post(&pool, 101, 1).await.unwrap().is_none());

        vote_post(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        assert_eq!(user_vote_on_post(&pool, 101, 1).await.unwrap(), Some(1));

        vote_post(&pool, 1, 101, 1, -1, 5001).await.unwrap();
        assert_eq!(user_vote_on_post(&pool, 101, 1).await.unwrap(), Some(-1));
    }

    #[tokio::test]
    async fn vote_comment_and_score() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();
        create_comment(&pool, &sample_comment(1, 1, "https://example.com/comment/1"))
            .await
            .unwrap();

        vote_comment(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        assert_eq!(comment_score(&pool, 1).await.unwrap(), 1);

        vote_comment(&pool, 1, 102, 1, 1, 5001).await.unwrap();
        assert_eq!(comment_score(&pool, 1).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn vote_comment_upsert() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();
        create_comment(&pool, &sample_comment(1, 1, "https://example.com/comment/1"))
            .await
            .unwrap();

        vote_comment(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        vote_comment(&pool, 1, 101, 1, -1, 5001).await.unwrap();
        assert_eq!(comment_score(&pool, 1).await.unwrap(), -1);
    }

    #[tokio::test]
    async fn score_with_no_votes_is_zero() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        create_post(&pool, &sample_post(1, "https://example.com/post/1"))
            .await
            .unwrap();
        create_comment(&pool, &sample_comment(1, 1, "https://example.com/comment/1"))
            .await
            .unwrap();

        assert_eq!(post_score(&pool, 1).await.unwrap(), 0);
        assert_eq!(comment_score(&pool, 1).await.unwrap(), 0);
    }

    // -- Bans --

    #[tokio::test]
    async fn ban_and_unban() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        assert!(!is_banned(&pool, 100, 101).await.unwrap());

        ban_from_community(&pool, 100, 101, "spam", None, 5000)
            .await
            .unwrap();
        assert!(is_banned(&pool, 100, 101).await.unwrap());

        unban_from_community(&pool, 100, 101).await.unwrap();
        assert!(!is_banned(&pool, 100, 101).await.unwrap());
    }

    #[tokio::test]
    async fn ban_upsert_updates_reason() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        ban_from_community(&pool, 100, 101, "spam", None, 5000)
            .await
            .unwrap();
        ban_from_community(&pool, 100, 101, "harassment", Some(9999), 5001)
            .await
            .unwrap();
        // Should still be banned (upsert, not duplicate)
        assert!(is_banned(&pool, 100, 101).await.unwrap());
    }

    #[tokio::test]
    async fn unban_nonexistent_is_ok() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();
        unban_from_community(&pool, 100, 101).await.unwrap();
    }

    // -- list_posts sort modes --

    #[tokio::test]
    async fn list_posts_top_sort() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        let mut p1 = sample_post(1, "https://example.com/post/1");
        p1.created_at = 3000;
        let mut p2 = sample_post(2, "https://example.com/post/2");
        p2.title = "Popular".into();
        p2.created_at = 3001;
        create_post(&pool, &p1).await.unwrap();
        create_post(&pool, &p2).await.unwrap();

        // Give post 2 more upvotes
        vote_post(&pool, 1, 101, 2, 1, 5000).await.unwrap();
        vote_post(&pool, 1, 102, 2, 1, 5001).await.unwrap();
        vote_post(&pool, 1, 101, 1, 1, 5002).await.unwrap();

        let posts = list_posts(&pool, 100, "top", 10, 0).await.unwrap();
        assert_eq!(posts[0].title, "Popular"); // score 2 > score 1
    }

    #[tokio::test]
    async fn list_posts_hot_sort() {
        let pool = setup().await;
        create_community(&pool, &sample_community()).await.unwrap();

        // Create two posts: one old with high score, one new with low score
        let mut p1 = sample_post(1, "https://example.com/post/1");
        p1.title = "Old Popular".into();
        p1.created_at = 1000;
        let mut p2 = sample_post(2, "https://example.com/post/2");
        p2.title = "Fresh".into();
        p2.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 60; // 1 minute ago
        create_post(&pool, &p1).await.unwrap();
        create_post(&pool, &p2).await.unwrap();

        vote_post(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        vote_post(&pool, 1, 101, 2, 1, 5001).await.unwrap();

        // Hot sort should favor the fresh post
        let posts = list_posts(&pool, 100, "hot", 10, 0).await.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Fresh");
    }

    // -- Integration: full lifecycle --

    #[tokio::test]
    async fn full_lifecycle() {
        let pool = setup().await;

        // Create community
        create_community(&pool, &sample_community()).await.unwrap();

        // Members join
        join_community(&pool, 100, 1, 101, 2000).await.unwrap();
        join_community(&pool, 100, 1, 102, 2001).await.unwrap();
        assert_eq!(member_count(&pool, 100).await.unwrap(), 2);

        // Promote alice to mod
        set_role(&pool, 100, 101, "mod").await.unwrap();
        assert!(is_moderator(&pool, 100, 101).await.unwrap());

        // Alice creates a post
        let p = sample_post(1, "https://example.com/post/1");
        create_post(&pool, &p).await.unwrap();

        // Bob comments
        let c = sample_comment(1, 1, "https://example.com/comment/1");
        create_comment(&pool, &c).await.unwrap();

        // Both vote on the post
        vote_post(&pool, 1, 101, 1, 1, 5000).await.unwrap();
        vote_post(&pool, 1, 102, 1, 1, 5001).await.unwrap();
        assert_eq!(post_score(&pool, 1).await.unwrap(), 2);

        // Alice upvotes bob's comment
        vote_comment(&pool, 1, 101, 1, 1, 5002).await.unwrap();
        assert_eq!(comment_score(&pool, 1).await.unwrap(), 1);

        // Mod locks the post
        lock_post(&pool, 1).await.unwrap();
        let fetched = get_post(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.locked);

        // Bob leaves
        leave_community(&pool, 100, 102).await.unwrap();
        assert_eq!(member_count(&pool, 100).await.unwrap(), 1);
    }
}
