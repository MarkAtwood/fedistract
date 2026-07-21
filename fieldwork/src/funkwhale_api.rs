//! Funkwhale-compatible REST API types and route builders.
//!
//! Provides JSON response types and Axum route constructors for a
//! Funkwhale-compatible audio/podcast API. Three server profiles:
//!
//! - **Broadside** — read-only endpoints (GET only)
//! - **Smallhold / Gaja** — full CRUD with auth (GET + POST)
//!
//! # Usage
//!
//! Each server creates handlers that call `fieldwork_db::audio_db` functions,
//! maps results to these response types, and registers routes using the
//! path constants below.
//!
//! ```ignore
//! use fieldwork::funkwhale_api::*;
//!
//! // In your server's router setup:
//! let app = Router::new()
//!     .route(TRACKS_PATH, get(list_tracks_handler))
//!     .route(TRACK_PATH, get(get_track_handler))
//!     // ... etc
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Route path constants
// ---------------------------------------------------------------------------

/// `GET /api/v1/tracks` — list tracks
pub const TRACKS_PATH: &str = "/api/v1/tracks";
/// `GET /api/v1/tracks/{id}` — get track by ID
pub const TRACK_PATH: &str = "/api/v1/tracks/{id}";
/// `GET /api/v1/albums` — list albums (grouped by album field)
pub const ALBUMS_PATH: &str = "/api/v1/albums";
/// `GET /api/v1/playlists` — list playlists
pub const PLAYLISTS_PATH: &str = "/api/v1/playlists";
/// `POST /api/v1/playlists` — create playlist (auth required)
pub const PLAYLISTS_CREATE_PATH: &str = "/api/v1/playlists";
/// `GET /api/v1/playlists/{id}/tracks` — get playlist tracks
pub const PLAYLIST_TRACKS_PATH: &str = "/api/v1/playlists/{id}/tracks";
/// `GET /api/v1/channels` — list audio channels
pub const CHANNELS_PATH: &str = "/api/v1/channels";
/// `GET /api/v1/channels/{name}` — get channel by name
pub const CHANNEL_PATH: &str = "/api/v1/channels/{name}";
/// `GET /feeds/podcast/{series_id}/rss` — podcast RSS feed
pub const PODCAST_RSS_PATH: &str = "/feeds/podcast/{series_id}/rss";
/// `GET /api/v1/listen/{track_id}` — stream/download audio
pub const LISTEN_PATH: &str = "/api/v1/listen/{track_id}";

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Standard pagination query parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    25
}

impl PaginationParams {
    /// Convert to SQL LIMIT/OFFSET.
    pub fn to_limit_offset(&self) -> (i64, i64) {
        let page_size = self.page_size.min(100).max(1) as i64;
        let page = self.page.max(1) as i64;
        let offset = (page - 1) * page_size;
        (page_size, offset)
    }
}

/// Paginated response wrapper.
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub count: usize,
    pub results: Vec<T>,
}

// ---------------------------------------------------------------------------
// Track types
// ---------------------------------------------------------------------------

/// JSON response for a single track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackResponse {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    pub file_size: i64,
    pub mime_type: String,
    pub description: String,
    pub visibility: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_url: Option<String>,
}

/// JSON request to upload a track.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTrackRequest {
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    pub track_number: Option<i32>,
    pub duration: Option<i64>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_visibility() -> String {
    "public".into()
}

// ---------------------------------------------------------------------------
// Album types
// ---------------------------------------------------------------------------

/// JSON response for an album (grouped by track album field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumResponse {
    pub album: String,
    pub artist: String,
    pub track_count: i64,
}

// ---------------------------------------------------------------------------
// Playlist types
// ---------------------------------------------------------------------------

/// JSON response for a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistResponse {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: String,
    pub visibility: String,
    pub created_at: i64,
}

/// JSON request to create a playlist.
#[derive(Debug, Clone, Deserialize)]
pub struct CreatePlaylistRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

// ---------------------------------------------------------------------------
// Channel types
// ---------------------------------------------------------------------------

/// JSON response for an audio channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResponse {
    pub persona_id: i64,
    pub description: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub rss_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itunes_category: Option<String>,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Axum route builder
// ---------------------------------------------------------------------------

/// Build an Axum router with read-only Funkwhale API routes.
///
/// This is the route set for broadside (read-only) servers. Handlers are
/// provided by the caller since they depend on server-specific state.
///
/// # Example
///
/// ```ignore
/// use axum::{Router, routing::get};
/// use fieldwork::funkwhale_api;
///
/// fn read_only_routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
///     funkwhale_api::read_only_router()
///         .route(funkwhale_api::TRACKS_PATH, get(my_list_tracks))
///         // ...
/// }
/// ```
pub fn read_only_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (TRACKS_PATH, "GET"),
        (TRACK_PATH, "GET"),
        (ALBUMS_PATH, "GET"),
        (PLAYLISTS_PATH, "GET"),
        (PLAYLIST_TRACKS_PATH, "GET"),
        (CHANNELS_PATH, "GET"),
        (CHANNEL_PATH, "GET"),
        (PODCAST_RSS_PATH, "GET"),
        (LISTEN_PATH, "GET"),
    ]
}

/// Full route set including write endpoints (for smallhold/gaja).
pub fn full_paths() -> &'static [(&'static str, &'static str)] {
    &[
        (TRACKS_PATH, "GET"),
        (TRACKS_PATH, "POST"),
        (TRACK_PATH, "GET"),
        (ALBUMS_PATH, "GET"),
        (PLAYLISTS_PATH, "GET"),
        (PLAYLISTS_CREATE_PATH, "POST"),
        (PLAYLIST_TRACKS_PATH, "GET"),
        (CHANNELS_PATH, "GET"),
        (CHANNEL_PATH, "GET"),
        (PODCAST_RSS_PATH, "GET"),
        (LISTEN_PATH, "GET"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_defaults() {
        let p = PaginationParams {
            page: 1,
            page_size: 25,
        };
        let (limit, offset) = p.to_limit_offset();
        assert_eq!(limit, 25);
        assert_eq!(offset, 0);
    }

    #[test]
    fn pagination_page_2() {
        let p = PaginationParams {
            page: 2,
            page_size: 10,
        };
        let (limit, offset) = p.to_limit_offset();
        assert_eq!(limit, 10);
        assert_eq!(offset, 10);
    }

    #[test]
    fn pagination_clamps_max() {
        let p = PaginationParams {
            page: 1,
            page_size: 500,
        };
        let (limit, offset) = p.to_limit_offset();
        assert_eq!(limit, 100); // clamped
        assert_eq!(offset, 0);
    }

    #[test]
    fn pagination_clamps_zero() {
        let p = PaginationParams {
            page: 0,
            page_size: 0,
        };
        let (limit, offset) = p.to_limit_offset();
        assert_eq!(limit, 1); // min 1
        assert_eq!(offset, 0); // page 0 treated as page 1
    }

    #[test]
    fn track_response_serializes() {
        let t = TrackResponse {
            id: 1,
            title: "My Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_number: Some(1),
            duration: Some(180),
            file_size: 5_000_000,
            mime_type: "audio/mpeg".into(),
            description: "A song".into(),
            visibility: "public".into(),
            created_at: 1000,
            listen_url: Some("/api/v1/listen/1".into()),
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"title\":\"My Song\""));
        assert!(json.contains("\"listen_url\":\"/api/v1/listen/1\""));
    }

    #[test]
    fn track_response_omits_none_fields() {
        let t = TrackResponse {
            id: 1,
            title: "Minimal".into(),
            artist: "".into(),
            album: "".into(),
            track_number: None,
            duration: None,
            file_size: 1000,
            mime_type: "audio/mpeg".into(),
            description: "".into(),
            visibility: "public".into(),
            created_at: 1000,
            listen_url: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("track_number"));
        assert!(!json.contains("duration"));
        assert!(!json.contains("listen_url"));
    }

    #[test]
    fn album_response_serializes() {
        let a = AlbumResponse {
            album: "Test Album".into(),
            artist: "Artist".into(),
            track_count: 5,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"track_count\":5"));
    }

    #[test]
    fn playlist_response_serializes() {
        let p = PlaylistResponse {
            id: 1,
            user_id: 1,
            title: "My Playlist".into(),
            description: "".into(),
            visibility: "public".into(),
            created_at: 1000,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"title\":\"My Playlist\""));
    }

    #[test]
    fn channel_response_serializes() {
        let c = ChannelResponse {
            persona_id: 100,
            description: "Music channel".into(),
            category: "music".into(),
            language: Some("en".into()),
            rss_enabled: true,
            itunes_category: None,
            created_at: 2000,
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"rss_enabled\":true"));
        assert!(!json.contains("itunes_category"));
    }

    #[test]
    fn create_track_request_deserializes() {
        let json = r#"{"title":"New Song","artist":"Me","duration":200}"#;
        let req: CreateTrackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "New Song");
        assert_eq!(req.artist, "Me");
        assert_eq!(req.album, ""); // default
        assert_eq!(req.duration, Some(200));
        assert_eq!(req.visibility, "public"); // default
    }

    #[test]
    fn create_playlist_request_deserializes() {
        let json = r#"{"title":"My List"}"#;
        let req: CreatePlaylistRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "My List");
        assert_eq!(req.description, "");
        assert_eq!(req.visibility, "public");
    }

    #[test]
    fn paginated_response_serializes() {
        let r = PaginatedResponse {
            count: 2,
            results: vec![
                AlbumResponse {
                    album: "A".into(),
                    artist: "X".into(),
                    track_count: 3,
                },
                AlbumResponse {
                    album: "B".into(),
                    artist: "Y".into(),
                    track_count: 1,
                },
            ],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"count\":2"));
        assert!(json.contains("\"results\":["));
    }

    #[test]
    fn read_only_paths_covers_all_gets() {
        let paths = read_only_paths();
        assert!(paths.iter().all(|(_, method)| *method == "GET"));
        assert!(paths.iter().any(|(p, _)| *p == TRACKS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == TRACK_PATH));
        assert!(paths.iter().any(|(p, _)| *p == ALBUMS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == PLAYLISTS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == PLAYLIST_TRACKS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == CHANNELS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == CHANNEL_PATH));
        assert!(paths.iter().any(|(p, _)| *p == PODCAST_RSS_PATH));
        assert!(paths.iter().any(|(p, _)| *p == LISTEN_PATH));
    }

    #[test]
    fn full_paths_includes_writes() {
        let paths = full_paths();
        let posts: Vec<_> = paths.iter().filter(|(_, m)| *m == "POST").collect();
        assert_eq!(posts.len(), 2); // tracks POST + playlists POST
    }

    #[test]
    fn path_constants_have_expected_format() {
        assert!(TRACKS_PATH.starts_with("/api/v1/"));
        assert!(TRACK_PATH.contains("{id}"));
        assert!(PLAYLIST_TRACKS_PATH.contains("{id}"));
        assert!(CHANNEL_PATH.contains("{name}"));
        assert!(PODCAST_RSS_PATH.contains("{series_id}"));
        assert!(LISTEN_PATH.contains("{track_id}"));
    }
}
