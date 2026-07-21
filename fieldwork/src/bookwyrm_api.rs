//! Bookwyrm-compatible REST API types and route builders.
//!
//! Provides JSON response/request types and route path constants for a
//! Bookwyrm-compatible book tracking API. Three server profiles:
//!
//! - **Broadside** — read-only endpoints (GET only)
//! - **Smallhold / Gaja** — full CRUD with auth (GET + POST)
//!
//! # Usage
//!
//! Each server creates handlers that call `fieldwork_db::books_db` functions,
//! maps results to these response types, and registers routes using the
//! path constants below.
//!
//! ```ignore
//! use fieldwork::bookwyrm_api::*;
//!
//! // In your server's router setup:
//! let app = Router::new()
//!     .route(BOOKS_PATH, get(search_books_handler))
//!     .route(BOOK_PATH, get(get_book_handler))
//!     // ... etc
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Route path constants
// ---------------------------------------------------------------------------

/// `GET /api/v1/books` — search books (query param `q`)
pub const BOOKS_PATH: &str = "/api/v1/books";
/// `GET /api/v1/books/{id}` — get book by ID
pub const BOOK_PATH: &str = "/api/v1/books/{id}";
/// `POST /api/v1/books` — add book to database (auth required)
pub const BOOKS_CREATE_PATH: &str = "/api/v1/books";
/// `GET /api/v1/shelves` — get user's shelf summary (counts per status)
pub const SHELVES_PATH: &str = "/api/v1/shelves";
/// `GET /api/v1/shelves/{status}` — list books on a shelf by status
pub const SHELF_STATUS_PATH: &str = "/api/v1/shelves/{status}";
/// `POST /api/v1/shelves` — set reading status for a book (auth required)
pub const SHELVES_UPDATE_PATH: &str = "/api/v1/shelves";
/// `POST /api/v1/books/{id}/rate` — rate a book 1-5 (auth required)
pub const BOOK_RATE_PATH: &str = "/api/v1/books/{id}/rate";
/// `GET /api/v1/books/{id}/reviews` — get reviews for a book
pub const BOOK_REVIEWS_PATH: &str = "/api/v1/books/{id}/reviews";
/// `POST /api/v1/books/{id}/reviews` — write a review (auth required)
pub const BOOK_REVIEWS_CREATE_PATH: &str = "/api/v1/books/{id}/reviews";
/// `GET /api/v1/users/{id}/reading` — user's reading activity
pub const USER_READING_PATH: &str = "/api/v1/users/{id}/reading";

// ---------------------------------------------------------------------------
// Book types
// ---------------------------------------------------------------------------

/// JSON response for a single book.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookResponse {
    pub id: i64,
    pub title: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn13: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openlibrary_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub created_at: i64,
}

/// JSON request to add a book.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBookRequest {
    pub title: String,
    #[serde(default)]
    pub author: String,
    pub isbn: Option<String>,
    pub isbn13: Option<String>,
    pub openlibrary_id: Option<String>,
    pub cover_url: Option<String>,
    #[serde(default)]
    pub description: String,
    pub pages: Option<i32>,
    pub published_year: Option<i32>,
    pub language: Option<String>,
}

/// Query parameters for book search.
#[derive(Debug, Clone, Deserialize)]
pub struct BookSearchParams {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    25
}

// ---------------------------------------------------------------------------
// Shelf types
// ---------------------------------------------------------------------------

/// JSON response for a user's shelf summary.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShelfSummaryResponse {
    pub to_read: i64,
    pub reading: i64,
    pub read: i64,
}

/// JSON response for a book on a shelf.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShelfBookResponse {
    pub book_id: i64,
    pub title: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
}

/// JSON request to set reading status.
#[derive(Debug, Clone, Deserialize)]
pub struct SetReadingStatusRequest {
    pub book_id: i64,
    pub status: String,
}

/// JSON request to rate a book.
#[derive(Debug, Clone, Deserialize)]
pub struct RateBookRequest {
    pub rating: i32,
}

// ---------------------------------------------------------------------------
// Review types
// ---------------------------------------------------------------------------

/// JSON response for a book review.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewResponse {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub book_id: i64,
    pub content: String,
    pub content_html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    pub spoiler: bool,
    pub ap_id: String,
    pub created_at: i64,
}

/// JSON request to write a review.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateReviewRequest {
    pub content: String,
    pub rating: Option<i32>,
    #[serde(default)]
    pub spoiler: bool,
}

// ---------------------------------------------------------------------------
// User reading activity
// ---------------------------------------------------------------------------

/// JSON response for a user's reading activity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserReadingResponse {
    pub shelves: ShelfSummaryResponse,
    pub recent_reviews: Vec<ReviewResponse>,
}

// ---------------------------------------------------------------------------
// Axum route builders
// ---------------------------------------------------------------------------

/// Read-only route set for broadside servers.
pub fn read_only_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (BOOKS_PATH, "GET"),
        (BOOK_PATH, "GET"),
        (SHELVES_PATH, "GET"),
        (SHELF_STATUS_PATH, "GET"),
        (BOOK_REVIEWS_PATH, "GET"),
        (USER_READING_PATH, "GET"),
    ]
}

/// Full route set including write endpoints (for smallhold/gaja).
pub fn full_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (BOOKS_PATH, "GET"),
        (BOOKS_CREATE_PATH, "POST"),
        (BOOK_PATH, "GET"),
        (SHELVES_PATH, "GET"),
        (SHELF_STATUS_PATH, "GET"),
        (SHELVES_UPDATE_PATH, "POST"),
        (BOOK_RATE_PATH, "POST"),
        (BOOK_REVIEWS_PATH, "GET"),
        (BOOK_REVIEWS_CREATE_PATH, "POST"),
        (USER_READING_PATH, "GET"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_response_serializes() {
        let b = BookResponse {
            id: 1,
            title: "Dune".into(),
            author: "Frank Herbert".into(),
            isbn: Some("0441172717".into()),
            isbn13: None,
            openlibrary_id: None,
            cover_url: None,
            description: "A science fiction novel.".into(),
            pages: Some(412),
            published_year: Some(1965),
            language: Some("en".into()),
            created_at: 1000,
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"title\":\"Dune\""));
        assert!(json.contains("\"author\":\"Frank Herbert\""));
        assert!(json.contains("\"pages\":412"));
    }

    #[test]
    fn book_response_omits_none_fields() {
        let b = BookResponse {
            id: 1,
            title: "Minimal".into(),
            author: "".into(),
            isbn: None,
            isbn13: None,
            openlibrary_id: None,
            cover_url: None,
            description: "".into(),
            pages: None,
            published_year: None,
            language: None,
            created_at: 1000,
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(!json.contains("isbn"));
        assert!(!json.contains("pages"));
        assert!(!json.contains("language"));
        assert!(!json.contains("cover_url"));
    }

    #[test]
    fn create_book_request_deserializes() {
        let json = r#"{"title":"Dune","author":"Frank Herbert","pages":412}"#;
        let req: CreateBookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Dune");
        assert_eq!(req.author, "Frank Herbert");
        assert_eq!(req.pages, Some(412));
        assert_eq!(req.description, "");
        assert!(req.isbn.is_none());
    }

    #[test]
    fn book_search_params_defaults() {
        let json = r#"{"q":"dune"}"#;
        let params: BookSearchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.q, "dune");
        assert_eq!(params.limit, 25);
    }

    #[test]
    fn shelf_summary_serializes() {
        let s = ShelfSummaryResponse {
            to_read: 10,
            reading: 3,
            read: 42,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"to_read\":10"));
        assert!(json.contains("\"reading\":3"));
        assert!(json.contains("\"read\":42"));
    }

    #[test]
    fn shelf_book_response_serializes() {
        let sb = ShelfBookResponse {
            book_id: 1,
            title: "Dune".into(),
            author: "Frank Herbert".into(),
            cover_url: Some("https://example.com/cover.jpg".into()),
            rating: Some(5),
        };
        let json = serde_json::to_string(&sb).unwrap();
        assert!(json.contains("\"book_id\":1"));
        assert!(json.contains("\"rating\":5"));
    }

    #[test]
    fn shelf_book_omits_none() {
        let sb = ShelfBookResponse {
            book_id: 1,
            title: "X".into(),
            author: "Y".into(),
            cover_url: None,
            rating: None,
        };
        let json = serde_json::to_string(&sb).unwrap();
        assert!(!json.contains("cover_url"));
        assert!(!json.contains("rating"));
    }

    #[test]
    fn set_reading_status_request_deserializes() {
        let json = r#"{"book_id":1,"status":"reading"}"#;
        let req: SetReadingStatusRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.book_id, 1);
        assert_eq!(req.status, "reading");
    }

    #[test]
    fn rate_book_request_deserializes() {
        let json = r#"{"rating":4}"#;
        let req: RateBookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.rating, 4);
    }

    #[test]
    fn review_response_serializes() {
        let r = ReviewResponse {
            id: 1,
            user_id: 1,
            persona_id: 101,
            book_id: 1,
            content: "Great book!".into(),
            content_html: "<p>Great book!</p>".into(),
            rating: Some(5),
            spoiler: false,
            ap_id: "https://example.com/reviews/1".into(),
            created_at: 3000,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"spoiler\":false"));
        assert!(json.contains("\"rating\":5"));
    }

    #[test]
    fn review_response_omits_none_rating() {
        let r = ReviewResponse {
            id: 1,
            user_id: 1,
            persona_id: 101,
            book_id: 1,
            content: "OK".into(),
            content_html: "<p>OK</p>".into(),
            rating: None,
            spoiler: true,
            ap_id: "https://example.com/reviews/1".into(),
            created_at: 3000,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("rating"));
        assert!(json.contains("\"spoiler\":true"));
    }

    #[test]
    fn create_review_request_deserializes() {
        let json = r#"{"content":"Amazing","rating":5}"#;
        let req: CreateReviewRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.content, "Amazing");
        assert_eq!(req.rating, Some(5));
        assert!(!req.spoiler);
    }

    #[test]
    fn create_review_request_with_spoiler() {
        let json = r#"{"content":"The ending...","spoiler":true}"#;
        let req: CreateReviewRequest = serde_json::from_str(json).unwrap();
        assert!(req.spoiler);
        assert!(req.rating.is_none());
    }

    #[test]
    fn user_reading_response_serializes() {
        let r = UserReadingResponse {
            shelves: ShelfSummaryResponse {
                to_read: 5,
                reading: 2,
                read: 10,
            },
            recent_reviews: vec![],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"to_read\":5"));
        assert!(json.contains("\"recent_reviews\":[]"));
    }

    #[test]
    fn read_only_paths_all_get() {
        let paths = read_only_paths();
        assert!(paths.iter().all(|(_, method)| *method == "GET"));
        assert!(paths.iter().any(|(p, _)| *p == BOOKS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == BOOK_PATH));
        assert!(paths.iter().any(|(p, _)| *p == SHELVES_PATH));
        assert!(paths.iter().any(|(p, _)| *p == SHELF_STATUS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == BOOK_REVIEWS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == USER_READING_PATH));
    }

    #[test]
    fn full_paths_includes_writes() {
        let paths = full_paths();
        let posts: Vec<_> = paths.iter().filter(|(_, m)| *m == "POST").collect();
        assert_eq!(posts.len(), 4); // books POST, shelves POST, rate POST, reviews POST
    }

    #[test]
    fn path_constants_have_expected_format() {
        assert!(BOOKS_PATH.starts_with("/api/v1/"));
        assert!(BOOK_PATH.contains("{id}"));
        assert!(SHELF_STATUS_PATH.contains("{status}"));
        assert!(BOOK_RATE_PATH.contains("{id}"));
        assert!(BOOK_REVIEWS_PATH.contains("{id}"));
        assert!(USER_READING_PATH.contains("{id}"));
    }
}
