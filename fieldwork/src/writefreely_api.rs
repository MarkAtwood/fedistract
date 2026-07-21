//! WriteFreely-compatible REST API types and route path constants.
//!
//! Provides JSON response/request types for a WriteFreely-compatible
//! blogging API. Three server profiles:
//!
//! - **Broadside** — read-only (GET endpoints for public posts/collections)
//! - **Smallhold / Gaja** — full CRUD with auth

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Route path constants
// ---------------------------------------------------------------------------

/// `POST /api/posts` — create post (anonymous or authenticated)
pub const POSTS_PATH: &str = "/api/posts";
/// `GET /api/posts/{id}` — get post by ID
pub const POST_PATH: &str = "/api/posts/{id}";
/// `POST /api/posts/claim` — claim anonymous posts
pub const POSTS_CLAIM_PATH: &str = "/api/posts/claim";
/// `POST /api/collections` — create collection
pub const COLLECTIONS_PATH: &str = "/api/collections";
/// `GET /api/collections/{alias}` — get collection
pub const COLLECTION_PATH: &str = "/api/collections/{alias}";
/// `GET /api/collections/{alias}/posts` — list collection posts
pub const COLLECTION_POSTS_PATH: &str = "/api/collections/{alias}/posts";
/// `GET /api/collections/{alias}/posts/{slug}` — get post by slug
pub const COLLECTION_POST_PATH: &str = "/api/collections/{alias}/posts/{slug}";
/// `POST /api/collections/{alias}/collect` — move posts into collection
pub const COLLECTION_COLLECT_PATH: &str = "/api/collections/{alias}/collect";
/// `POST /api/collections/{alias}/pin` — pin posts
pub const COLLECTION_PIN_PATH: &str = "/api/collections/{alias}/pin";
/// `POST /api/collections/{alias}/unpin` — unpin posts
pub const COLLECTION_UNPIN_PATH: &str = "/api/collections/{alias}/unpin";
/// `GET /api/me` — current user
pub const ME_PATH: &str = "/api/me";
/// `GET /api/me/posts` — user's posts
pub const ME_POSTS_PATH: &str = "/api/me/posts";
/// `GET /api/me/collections` — user's collections
pub const ME_COLLECTIONS_PATH: &str = "/api/me/collections";
/// `POST /api/markdown` — render markdown to HTML
pub const MARKDOWN_PATH: &str = "/api/markdown";

// ---------------------------------------------------------------------------
// Response wrapper
// ---------------------------------------------------------------------------

/// WriteFreely wraps all responses in `{ "code": N, "data": ... }`.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct WfResponse<T> {
    pub code: u16,
    pub data: T,
}

/// Error response: `{ "code": N, "error_msg": "..." }`.
#[derive(Debug, Clone, Serialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct WfError {
    pub code: u16,
    pub error_msg: String,
}

// ---------------------------------------------------------------------------
// Post types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct PostResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    pub views: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<CollectionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CollectionRef {
    pub alias: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CreatePostRequest {
    pub body: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub rtl: Option<bool>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct UpdatePostRequest {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub rtl: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct ClaimPostRequest {
    pub id: String,
    pub token: String,
}

// ---------------------------------------------------------------------------
// Collection types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CollectionResponse {
    pub alias: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_sheet: Option<String>,
    pub public: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CreateCollectionRequest {
    pub alias: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct UpdateCollectionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style_sheet: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CollectRequest {
    pub id: String,
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct PinRequest {
    pub id: String,
    #[serde(default = "default_position")]
    pub position: i32,
}

fn default_position() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct UnpinRequest {
    pub id: String,
}

// ---------------------------------------------------------------------------
// User types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct UserResponse {
    pub username: String,
}

// ---------------------------------------------------------------------------
// Markdown render
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct MarkdownRequest {
    pub raw_body: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[non_exhaustive]
#[non_exhaustive]
pub struct MarkdownResponse {
    pub body: String,
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[non_exhaustive]
pub struct CollectionPostsQuery {
    #[serde(default)]
    pub body: Option<bool>,
    #[serde(default = "default_page")]
    pub page: u32,
}

fn default_page() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Route helpers
// ---------------------------------------------------------------------------

pub fn read_only_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (POST_PATH, "GET"),
        (COLLECTION_PATH, "GET"),
        (COLLECTION_POSTS_PATH, "GET"),
        (COLLECTION_POST_PATH, "GET"),
        (MARKDOWN_PATH, "POST"),
    ]
}

pub fn full_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (POSTS_PATH, "POST"),
        (POST_PATH, "GET"),
        (POST_PATH, "POST"),
        (POST_PATH, "DELETE"),
        (POSTS_CLAIM_PATH, "POST"),
        (COLLECTIONS_PATH, "POST"),
        (COLLECTION_PATH, "GET"),
        (COLLECTION_PATH, "POST"),
        (COLLECTION_PATH, "DELETE"),
        (COLLECTION_POSTS_PATH, "GET"),
        (COLLECTION_POSTS_PATH, "POST"),
        (COLLECTION_POST_PATH, "GET"),
        (COLLECTION_COLLECT_PATH, "POST"),
        (COLLECTION_PIN_PATH, "POST"),
        (COLLECTION_UNPIN_PATH, "POST"),
        (ME_PATH, "GET"),
        (ME_POSTS_PATH, "GET"),
        (ME_COLLECTIONS_PATH, "GET"),
        (MARKDOWN_PATH, "POST"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_response_serializes() {
        let p = PostResponse {
            id: "abc123".into(),
            slug: Some("hello-world".into()),
            token: None,
            title: "Hello".into(),
            body: "Content".into(),
            font: Some("norm".into()),
            lang: Some("en".into()),
            rtl: None,
            created: "2026-01-01T00:00:00Z".into(),
            updated: None,
            views: 42,
            collection: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"id\":\"abc123\""));
        assert!(!json.contains("token"));
        assert!(!json.contains("rtl"));
    }

    #[test]
    fn wf_response_wraps() {
        let resp = WfResponse {
            code: 200,
            data: "hello",
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"code\":200"));
        assert!(json.contains("\"data\":\"hello\""));
    }

    #[test]
    fn create_post_request_deserializes_minimal() {
        let json = r#"{"body":"Hello world"}"#;
        let req: CreatePostRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.body, "Hello world");
        assert!(req.title.is_none());
        assert!(req.font.is_none());
    }

    #[test]
    fn create_collection_request_deserializes() {
        let json = r#"{"alias":"my-blog","title":"My Blog"}"#;
        let req: CreateCollectionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.alias, "my-blog");
        assert_eq!(req.title.as_deref(), Some("My Blog"));
    }

    #[test]
    fn collection_response_serializes() {
        let c = CollectionResponse {
            alias: "blog".into(),
            title: "My Blog".into(),
            description: "".into(),
            style_sheet: None,
            public: true,
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"public\":true"));
        assert!(!json.contains("style_sheet"));
    }

    #[test]
    fn read_only_paths_all_get_or_post() {
        let paths = read_only_paths();
        assert!(paths.iter().any(|(p, _)| *p == POST_PATH));
        assert!(paths.iter().any(|(p, _)| *p == COLLECTION_POSTS_PATH));
    }

    #[test]
    fn full_paths_includes_writes() {
        let paths = full_paths();
        let posts: Vec<_> = paths.iter().filter(|(_, m)| *m == "POST").collect();
        assert!(posts.len() >= 8);
        let deletes: Vec<_> = paths.iter().filter(|(_, m)| *m == "DELETE").collect();
        assert_eq!(deletes.len(), 2);
    }
}
