//! ActivityPub actor document construction and serialization.
//!
//! Produces JSON-LD actor documents compatible with Mastodon's expectations,
//! including the full `@context` array with `toot:` and `schema:` extensions.
//!
//! # Example
//!
//! ```
//! use fieldwork::actor::{ActorDocument, ActorPublicKey};
//!
//! let actor = ActorDocument::build(
//!     "example.com",
//!     "alice",
//!     "Alice",
//!     "-----BEGIN PUBLIC KEY-----\nMIIB...\n-----END PUBLIC KEY-----",
//! );
//!
//! let json = serde_json::to_string(&actor).unwrap();
//! assert!(json.contains("\"preferredUsername\":\"alice\""));
//! ```

use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

/// ActivityPub actor document with JSON-LD context.
///
/// Serializes to a JSON-LD document matching the format Mastodon expects,
/// including the `@context` array with ActivityStreams, security, and Mastodon
/// extension vocabularies.
#[derive(Debug, Clone, Deserialize)]
pub struct ActorDocument {
    /// Actor URI (e.g., `https://example.com/users/alice`).
    pub id: String,

    /// Actor type: `"Person"`, `"Service"`, `"Application"`, etc.
    pub type_: String,

    /// Username without domain (e.g., `alice`).
    pub preferred_username: String,

    /// Display name.
    pub name: String,

    /// Bio/about as HTML.
    pub summary: String,

    /// Profile page URL (e.g., `https://example.com/@alice`).
    pub url: String,

    /// Inbox endpoint URL.
    pub inbox: String,

    /// Outbox endpoint URL.
    pub outbox: String,

    /// Followers collection URL.
    pub followers: String,

    /// Following collection URL.
    pub following: String,

    /// RSA public key for HTTP Signatures.
    pub public_key: ActorPublicKey,

    /// Canonical DID (`did:scid:...`), if the server supports DIDs.
    pub did: Option<String>,

    /// Alternate identifiers: `did:scid`, `did:key`, `did:web`, optionally `did:plc`.
    pub also_known_as: Vec<String>,

    /// Avatar image.
    pub icon: Option<MediaAttachment>,

    /// Header/banner image.
    pub image: Option<MediaAttachment>,

    /// Whether follow requests require approval.
    pub manually_approves_followers: bool,

    /// Whether the actor appears in directories/search.
    pub discoverable: bool,

    /// Shared inbox and other endpoints.
    pub endpoints: Option<ActorEndpoints>,

    /// Profile metadata fields (Mastodon `PropertyValue` attachments).
    pub attachment: Vec<serde_json::Value>,
}

/// RSA public key block for HTTP Signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorPublicKey {
    /// Key ID (e.g., `https://example.com/users/alice#main-key`).
    pub id: String,

    /// Actor URI that owns this key.
    pub owner: String,

    /// PEM-encoded RSA public key.
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
}

/// Server endpoints advertised in the actor document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorEndpoints {
    /// Shared inbox URL for efficient delivery.
    #[serde(rename = "sharedInbox", skip_serializing_if = "Option::is_none")]
    pub shared_inbox: Option<String>,
}

/// Image attachment (avatar or header).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    /// Always `"Image"` for actor icons/headers.
    #[serde(rename = "type")]
    pub type_: String,

    /// MIME type (e.g., `"image/png"`).
    #[serde(rename = "mediaType", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// URL to the image.
    pub url: String,
}

/// Build the Mastodon-compatible JSON-LD `@context` array.
///
/// ActivityPub JSON-LD context array.
///
/// Includes the standard ActivityStreams + security contexts, Mastodon's `toot:`
/// and `schema:` extensions, and the fedistract context for DID, abstract,
/// and content warning fields.
fn ap_context() -> serde_json::Value {
    serde_json::json!([
        "https://www.w3.org/ns/activitystreams",
        "https://w3id.org/security/v1",
        {
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "toot": "http://joinmastodon.org/ns#",
            "featured": {"@id": "toot:featured", "@type": "@id"},
            "featuredTags": {"@id": "toot:featuredTags", "@type": "@id"},
            "alsoKnownAs": {"@id": "as:alsoKnownAs", "@type": "@id"},
            "movedTo": {"@id": "as:movedTo", "@type": "@id"},
            "schema": "http://schema.org#",
            "PropertyValue": "schema:PropertyValue",
            "value": "schema:value",
            "discoverable": "toot:discoverable",
            "suspended": "toot:suspended",
            "memorial": "toot:memorial",
            "Hashtag": "as:Hashtag",
            "Emoji": "toot:Emoji",
            "blurhash": "toot:blurhash",
            "focalPoint": {"@container": "@list", "@id": "toot:focalPoint"},
            "fedistract": "https://raw.githubusercontent.com/MarkAtwood/fedistract/main/contexts/fedistract.jsonld#",
            "did": {"@id": "fedistract:did", "@type": "@id"},
            "abstract": "fedistract:abstract",
            "contentWarning": "fedistract:contentWarning"
        }
    ])
}

impl Serialize for ActorDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Count fields: 16 fixed + optionals
        let mut count = 16; // @context, id, type, preferredUsername, name, summary, url,
                            // inbox, outbox, followers, following, publicKey,
                            // manuallyApprovesFollowers, discoverable, alsoKnownAs, attachment
        if self.did.is_some() {
            count += 1;
        }
        if self.icon.is_some() {
            count += 1;
        }
        if self.image.is_some() {
            count += 1;
        }
        if self.endpoints.is_some() {
            count += 1;
        }

        let mut map = serializer.serialize_map(Some(count))?;

        map.serialize_entry("@context", &ap_context())?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("type", &self.type_)?;
        map.serialize_entry("preferredUsername", &self.preferred_username)?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("summary", &self.summary)?;
        map.serialize_entry("url", &self.url)?;
        map.serialize_entry("inbox", &self.inbox)?;
        map.serialize_entry("outbox", &self.outbox)?;
        map.serialize_entry("followers", &self.followers)?;
        map.serialize_entry("following", &self.following)?;
        map.serialize_entry("publicKey", &self.public_key)?;
        map.serialize_entry("manuallyApprovesFollowers", &self.manually_approves_followers)?;
        map.serialize_entry("discoverable", &self.discoverable)?;
        map.serialize_entry("alsoKnownAs", &self.also_known_as)?;

        if let Some(ref did) = self.did {
            map.serialize_entry("did", did)?;
        }
        if let Some(ref icon) = self.icon {
            map.serialize_entry("icon", icon)?;
        }
        if let Some(ref image) = self.image {
            map.serialize_entry("image", image)?;
        }
        if let Some(ref endpoints) = self.endpoints {
            map.serialize_entry("endpoints", endpoints)?;
        }

        map.serialize_entry("attachment", &self.attachment)?;

        map.end()
    }
}

impl ActorDocument {
    /// Build an actor document with reasonable defaults for a standard fediverse server.
    ///
    /// Derives inbox, outbox, followers, following, and key URLs from `domain` and `username`.
    /// Sets `type` to `"Person"`, `discoverable` to `true`, and `manually_approves_followers`
    /// to `false`. The shared inbox defaults to `https://{domain}/inbox`.
    pub fn build(
        domain: &str,
        username: &str,
        display_name: &str,
        public_key_pem: &str,
    ) -> Self {
        let actor_uri = format!("https://{domain}/users/{username}");

        ActorDocument {
            id: actor_uri.clone(),
            type_: "Person".to_string(),
            preferred_username: username.to_string(),
            name: display_name.to_string(),
            summary: String::new(),
            url: format!("https://{domain}/@{username}"),
            inbox: format!("{actor_uri}/inbox"),
            outbox: format!("{actor_uri}/outbox"),
            followers: format!("{actor_uri}/followers"),
            following: format!("{actor_uri}/following"),
            public_key: ActorPublicKey {
                id: format!("{actor_uri}#main-key"),
                owner: actor_uri,
                public_key_pem: public_key_pem.to_string(),
            },
            did: None,
            also_known_as: Vec::new(),
            icon: None,
            image: None,
            manually_approves_followers: false,
            discoverable: true,
            endpoints: Some(ActorEndpoints {
                shared_inbox: Some(format!("https://{domain}/inbox")),
            }),
            attachment: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMIIBIjANB...test...AQAB\n-----END PUBLIC KEY-----";

    #[test]
    fn build_produces_valid_actor() {
        let actor = ActorDocument::build("example.com", "alice", "Alice", TEST_PEM);

        assert_eq!(actor.id, "https://example.com/users/alice");
        assert_eq!(actor.type_, "Person");
        assert_eq!(actor.preferred_username, "alice");
        assert_eq!(actor.name, "Alice");
        assert_eq!(actor.inbox, "https://example.com/users/alice/inbox");
        assert_eq!(actor.outbox, "https://example.com/users/alice/outbox");
        assert_eq!(actor.followers, "https://example.com/users/alice/followers");
        assert_eq!(actor.following, "https://example.com/users/alice/following");
        assert_eq!(actor.url, "https://example.com/@alice");
        assert_eq!(actor.public_key.id, "https://example.com/users/alice#main-key");
        assert_eq!(actor.public_key.owner, "https://example.com/users/alice");
        assert_eq!(actor.public_key.public_key_pem, TEST_PEM);
        assert!(actor.discoverable);
        assert!(!actor.manually_approves_followers);
    }

    #[test]
    fn serializes_with_context() {
        let actor = ActorDocument::build("example.com", "alice", "Alice", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        // @context must be present and be an array
        let context = json.get("@context").expect("@context missing");
        let ctx_arr = context.as_array().expect("@context should be array");
        assert_eq!(ctx_arr[0], "https://www.w3.org/ns/activitystreams");
        assert_eq!(ctx_arr[1], "https://w3id.org/security/v1");
        assert!(ctx_arr[2].is_object(), "@context[2] should be extension object");
    }

    #[test]
    fn serializes_correct_field_names() {
        let actor = ActorDocument::build("example.com", "bob", "Bob", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        // JSON-LD field names (camelCase), not Rust field names (snake_case)
        assert!(json.get("preferredUsername").is_some(), "missing preferredUsername");
        assert!(json.get("publicKey").is_some(), "missing publicKey");
        assert!(json.get("manuallyApprovesFollowers").is_some(), "missing manuallyApprovesFollowers");
        assert!(json.get("alsoKnownAs").is_some(), "missing alsoKnownAs");

        // Must use "type" not "type_"
        assert!(json.get("type").is_some(), "missing type");
        assert!(json.get("type_").is_none(), "should not have type_");
    }

    #[test]
    fn serializes_public_key_correctly() {
        let actor = ActorDocument::build("example.com", "carol", "Carol", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        let pk = json.get("publicKey").expect("missing publicKey");
        assert_eq!(pk["id"], "https://example.com/users/carol#main-key");
        assert_eq!(pk["owner"], "https://example.com/users/carol");
        assert_eq!(pk["publicKeyPem"], TEST_PEM);
    }

    #[test]
    fn serializes_endpoints() {
        let actor = ActorDocument::build("example.com", "dave", "Dave", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        let ep = json.get("endpoints").expect("missing endpoints");
        assert_eq!(ep["sharedInbox"], "https://example.com/inbox");
    }

    #[test]
    fn omits_none_fields() {
        let actor = ActorDocument::build("example.com", "eve", "Eve", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        // did, icon, image should not appear when None
        assert!(json.get("did").is_none(), "did should be absent when None");
        assert!(json.get("icon").is_none(), "icon should be absent when None");
        assert!(json.get("image").is_none(), "image should be absent when None");
    }

    #[test]
    fn includes_optional_fields_when_set() {
        let mut actor = ActorDocument::build("example.com", "frank", "Frank", TEST_PEM);
        actor.did = Some("did:scid:sha256:abc123".to_string());
        actor.also_known_as = vec![
            "did:web:example.com:users:frank".to_string(),
            "did:key:z6Mk1234".to_string(),
        ];
        actor.icon = Some(MediaAttachment {
            type_: "Image".to_string(),
            media_type: Some("image/png".to_string()),
            url: "https://example.com/avatars/frank.png".to_string(),
        });
        actor.image = Some(MediaAttachment {
            type_: "Image".to_string(),
            media_type: Some("image/jpeg".to_string()),
            url: "https://example.com/headers/frank.jpg".to_string(),
        });

        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        assert_eq!(json["did"], "did:scid:sha256:abc123");
        assert_eq!(json["alsoKnownAs"][0], "did:web:example.com:users:frank");
        assert_eq!(json["alsoKnownAs"][1], "did:key:z6Mk1234");
        assert_eq!(json["icon"]["type"], "Image");
        assert_eq!(json["icon"]["mediaType"], "image/png");
        assert_eq!(json["icon"]["url"], "https://example.com/avatars/frank.png");
        assert_eq!(json["image"]["type"], "Image");
        assert_eq!(json["image"]["url"], "https://example.com/headers/frank.jpg");
    }

    #[test]
    fn attachment_property_values() {
        let mut actor = ActorDocument::build("example.com", "grace", "Grace", TEST_PEM);
        actor.attachment = vec![
            serde_json::json!({
                "type": "PropertyValue",
                "name": "Website",
                "value": "<a href=\"https://grace.dev\">grace.dev</a>"
            }),
            serde_json::json!({
                "type": "PropertyValue",
                "name": "GitHub",
                "value": "grace"
            }),
        ];

        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();
        let att = json["attachment"].as_array().expect("attachment should be array");
        assert_eq!(att.len(), 2);
        assert_eq!(att[0]["type"], "PropertyValue");
        assert_eq!(att[0]["name"], "Website");
        assert_eq!(att[1]["name"], "GitHub");
    }

    #[test]
    fn service_type_for_bots() {
        let mut actor = ActorDocument::build("example.com", "bot", "News Bot", TEST_PEM);
        actor.type_ = "Service".to_string();

        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();
        assert_eq!(json["type"], "Service");
    }

    #[test]
    fn media_attachment_omits_none_media_type() {
        let attachment = MediaAttachment {
            type_: "Image".to_string(),
            media_type: None,
            url: "https://example.com/img.png".to_string(),
        };

        let json: serde_json::Value = serde_json::to_value(&attachment).unwrap();
        assert!(json.get("mediaType").is_none(), "mediaType should be absent when None");
        assert_eq!(json["type"], "Image");
        assert_eq!(json["url"], "https://example.com/img.png");
    }

    #[test]
    fn round_trip_json_string() {
        let mut actor = ActorDocument::build("example.com", "alice", "Alice", TEST_PEM);
        actor.summary = "<p>Hello world</p>".to_string();
        actor.manually_approves_followers = true;

        let json_str = serde_json::to_string(&actor).unwrap();

        // Verify it's valid JSON and key fields survive serialization
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["id"], "https://example.com/users/alice");
        assert_eq!(parsed["summary"], "<p>Hello world</p>");
        assert_eq!(parsed["manuallyApprovesFollowers"], true);
    }

    #[test]
    fn mastodon_compatible_context_extensions() {
        let actor = ActorDocument::build("example.com", "test", "Test", TEST_PEM);
        let json: serde_json::Value = serde_json::to_value(&actor).unwrap();

        let ext = &json["@context"][2];
        // Key Mastodon extensions that remote servers expect
        assert_eq!(ext["toot"], "http://joinmastodon.org/ns#");
        assert_eq!(ext["schema"], "http://schema.org#");
        assert_eq!(ext["PropertyValue"], "schema:PropertyValue");
        assert_eq!(ext["discoverable"], "toot:discoverable");
        assert_eq!(ext["manuallyApprovesFollowers"], "as:manuallyApprovesFollowers");
    }
}
