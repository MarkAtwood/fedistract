//! Mastodon Client API response types.
//!
//! Typed structs for the JSON entities returned by Mastodon-compatible
//! API endpoints. These replace inline `json!()` macro calls in server
//! handler code, enabling shared serialization across all 3 servers.
//!
//! Account types live in [`crate::profile`] (already extracted).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Status entity
// ---------------------------------------------------------------------------

/// Mastodon API Status entity (`/api/v1/statuses`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Status {
    pub id: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to_account_id: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String,
    pub visibility: String,
    pub language: String,
    pub uri: String,
    pub url: String,
    pub replies_count: i64,
    pub reblogs_count: i64,
    pub favourites_count: i64,
    pub favourited: bool,
    pub reblogged: bool,
    pub muted: bool,
    pub bookmarked: bool,
    pub pinned: bool,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reblog: Option<Box<Status>>,
    pub application: Application,
    /// Kept as `serde_json::Value` intentionally: servers build account JSON
    /// from heterogeneous sources (e.g. `json!()` macros, `account_to_json()`
    /// helpers) that do not always originate from a `profile::Account` struct.
    /// Using `Value` avoids forcing every server to construct a full `Account`.
    pub account: serde_json::Value,
    pub media_attachments: Vec<MediaAttachment>,
    pub mentions: Vec<Mention>,
    pub tags: Vec<Tag>,
    pub emojis: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,
}

/// Application sub-object inside Status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Application {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
}

/// Media attachment entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct MediaAttachment {
    pub id: String,
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: String,
    pub preview_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blurhash: Option<String>,
}

/// Mention entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Mention {
    pub id: String,
    pub username: String,
    pub acct: String,
    pub url: String,
}

/// Hashtag entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Tag {
    pub name: String,
    pub url: String,
}

// ---------------------------------------------------------------------------
// Relationship entity
// ---------------------------------------------------------------------------

/// Mastodon API Relationship entity (`/api/v1/accounts/relationships`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Relationship {
    pub id: String,
    pub following: bool,
    pub showing_reblogs: bool,
    pub notifying: bool,
    pub followed_by: bool,
    pub blocking: bool,
    pub blocked_by: bool,
    pub muting: bool,
    pub muting_notifications: bool,
    pub requested: bool,
    pub requested_by: bool,
    pub domain_blocking: bool,
    pub endorsed: bool,
    pub note: String,
}

impl Relationship {
    /// Default relationship (not following, not blocking, etc.)
    pub fn default_for(id: i64) -> Self {
        Self {
            id: id.to_string(),
            following: false,
            showing_reblogs: true,
            notifying: false,
            followed_by: false,
            blocking: false,
            blocked_by: false,
            muting: false,
            muting_notifications: false,
            requested: false,
            requested_by: false,
            domain_blocking: false,
            endorsed: false,
            note: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Context entity
// ---------------------------------------------------------------------------

/// Status context (ancestors + descendants).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Context {
    pub ancestors: Vec<Status>,
    pub descendants: Vec<Status>,
}

// ---------------------------------------------------------------------------
// Instance entity (v2)
// ---------------------------------------------------------------------------

/// Instance configuration sub-object.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct InstanceConfiguration {
    pub statuses: StatusesConfig,
    pub media_attachments: MediaConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct StatusesConfig {
    pub max_characters: u32,
    pub max_media_attachments: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct MediaConfig {
    pub supported_mime_types: Vec<String>,
    pub image_size_limit: u64,
    pub video_size_limit: u64,
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

/// Build a Status entity from common fields.
#[non_exhaustive]
pub struct StatusBuilder {
    pub id: i64,
    pub created_at: String,
    pub content_html: String,
    pub spoiler_text: String,
    pub visibility: String,
    pub language: String,
    pub username: String,
    pub domain: String,
    pub sensitive: bool,
    pub in_reply_to_id: Option<i64>,
    pub edited_at: Option<String>,
    pub abstract_text: Option<String>,
    pub app_name: String,
    pub app_website: Option<String>,
    pub account_json: serde_json::Value,
    pub media_attachments: Vec<MediaAttachment>,
    pub mentions: Vec<Mention>,
    pub tags: Vec<Tag>,
    pub favourited: bool,
    pub reblogged: bool,
    pub muted: bool,
    pub bookmarked: bool,
    pub pinned: bool,
}

impl Default for StatusBuilder {
    fn default() -> Self {
        Self {
            id: 0,
            created_at: String::new(),
            content_html: String::new(),
            spoiler_text: String::new(),
            visibility: "public".into(),
            language: "en".into(),
            username: String::new(),
            domain: String::new(),
            sensitive: false,
            in_reply_to_id: None,
            edited_at: None,
            abstract_text: None,
            app_name: String::new(),
            app_website: None,
            account_json: serde_json::Value::Object(Default::default()),
            media_attachments: Vec::new(),
            mentions: Vec::new(),
            tags: Vec::new(),
            favourited: false,
            reblogged: false,
            muted: false,
            bookmarked: false,
            pinned: false,
        }
    }
}

impl StatusBuilder {
    pub fn build(self) -> Status {
        let id_str = self.id.to_string();
        Status {
            uri: format!("https://{}/users/{}/statuses/{}", self.domain, self.username, id_str),
            url: format!("https://{}/@{}/{}", self.domain, self.username, id_str),
            id: id_str,
            created_at: self.created_at,
            in_reply_to_id: self.in_reply_to_id.map(|id| id.to_string()),
            in_reply_to_account_id: None,
            sensitive: self.sensitive,
            spoiler_text: self.spoiler_text,
            visibility: self.visibility,
            language: self.language,
            replies_count: 0,
            reblogs_count: 0,
            favourites_count: 0,
            favourited: self.favourited,
            reblogged: self.reblogged,
            muted: self.muted,
            bookmarked: self.bookmarked,
            pinned: self.pinned,
            content: self.content_html,
            reblog: None,
            application: Application {
                name: self.app_name,
                website: self.app_website,
            },
            account: self.account_json,
            media_attachments: self.media_attachments,
            mentions: self.mentions,
            tags: self.tags,
            emojis: Vec::new(),
            card: None,
            poll: None,
            edited_at: self.edited_at,
            abstract_text: self.abstract_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes() {
        let status = StatusBuilder {
            id: 12345,
            created_at: "2026-01-01T00:00:00Z".into(),
            content_html: "<p>Hello</p>".into(),
            spoiler_text: String::new(),
            visibility: "public".into(),
            language: "en".into(),
            username: "alice".into(),
            domain: "example.com".into(),
            sensitive: false,
            in_reply_to_id: None,
            edited_at: None,
            abstract_text: None,
            app_name: "test".into(),
            app_website: None,
            account_json: serde_json::json!({}),
            media_attachments: vec![],
            mentions: vec![],
            tags: vec![],
            favourited: false,
            reblogged: false,
            muted: false,
            bookmarked: false,
            pinned: false,
        }
        .build();

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["id"], "12345");
        assert_eq!(json["visibility"], "public");
        assert_eq!(json["uri"], "https://example.com/users/alice/statuses/12345");
        assert!(!json.as_object().unwrap().contains_key("in_reply_to_id"));
        assert!(!json.as_object().unwrap().contains_key("abstract"));
    }

    #[test]
    fn status_with_optional_fields() {
        let status = StatusBuilder {
            id: 1,
            created_at: "2026-01-01T00:00:00Z".into(),
            content_html: "<p>Reply</p>".into(),
            spoiler_text: "CW".into(),
            visibility: "unlisted".into(),
            language: "en".into(),
            username: "bob".into(),
            domain: "test.com".into(),
            sensitive: true,
            in_reply_to_id: Some(999),
            edited_at: Some("2026-01-02T00:00:00Z".into()),
            abstract_text: Some("A reply post".into()),
            app_name: "app".into(),
            app_website: Some("https://app.example".into()),
            account_json: serde_json::json!({}),
            media_attachments: vec![],
            mentions: vec![],
            tags: vec![],
            favourited: true,
            reblogged: false,
            muted: false,
            bookmarked: true,
            pinned: false,
        }
        .build();

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["in_reply_to_id"], "999");
        assert_eq!(json["edited_at"], "2026-01-02T00:00:00Z");
        assert_eq!(json["abstract"], "A reply post");
        assert_eq!(json["sensitive"], true);
        assert_eq!(json["favourited"], true);
    }

    #[test]
    fn relationship_default() {
        let rel = Relationship::default_for(42);
        let json = serde_json::to_value(&rel).unwrap();
        assert_eq!(json["id"], "42");
        assert_eq!(json["following"], false);
        assert_eq!(json["showing_reblogs"], true);
        assert_eq!(json["note"], "");
    }

    #[test]
    fn media_attachment_serializes() {
        let ma = MediaAttachment {
            id: "1".into(),
            media_type: "image".into(),
            url: "https://example.com/img.jpg".into(),
            preview_url: "https://example.com/img_thumb.jpg".into(),
            remote_url: None,
            description: "A photo".into(),
            blurhash: Some("LEHV6nWB2y".into()),
        };
        let json = serde_json::to_value(&ma).unwrap();
        assert_eq!(json["type"], "image");
        assert!(!json.as_object().unwrap().contains_key("remote_url"));
    }

    #[test]
    fn context_serializes() {
        let ctx = Context {
            ancestors: vec![],
            descendants: vec![],
        };
        let json = serde_json::to_value(&ctx).unwrap();
        assert_eq!(json["ancestors"], serde_json::json!([]));
        assert_eq!(json["descendants"], serde_json::json!([]));
    }
}
