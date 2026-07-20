//! Mastodon Client API account serialization.
//!
//! Provides typed structs for the JSON returned by `/api/v1/accounts/:id`
//! and `/api/v1/accounts/verify_credentials`, plus builder functions for
//! local and remote accounts.
//!
//! # Feature gate
//!
//! This module requires the `profile` feature:
//!
//! ```toml
//! fieldwork = { version = "0.1", features = ["profile"] }
//! ```

use serde::{Deserialize, Serialize};

/// A profile metadata field (name/value pair shown on the profile).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileField {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

/// Mastodon Client API account entity.
///
/// Matches the JSON shape returned by `GET /api/v1/accounts/:id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: String,
    pub username: String,
    pub acct: String,
    pub display_name: String,
    pub note: String,
    pub url: String,
    pub avatar: String,
    pub avatar_static: String,
    pub header: String,
    pub header_static: String,
    pub locked: bool,
    pub bot: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discoverable: Option<bool>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status_at: Option<String>,
    pub statuses_count: u64,
    pub followers_count: u64,
    pub following_count: u64,
    pub fields: Vec<ProfileField>,
    pub emojis: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did: Option<String>,
}

/// Extended account entity returned by `GET /api/v1/accounts/verify_credentials`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CredentialAccount {
    #[serde(flatten)]
    pub account: Account,
    pub source: AccountSource,
}

/// The `source` sub-object inside [`CredentialAccount`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountSource {
    pub note: String,
    pub fields: Vec<ProfileField>,
    pub privacy: String,
    pub sensitive: bool,
    pub language: String,
}

/// Build an [`Account`] for a local user with sensible defaults.
///
/// Avatar and header URLs default to the standard Mastodon missing-image paths.
/// Counts default to zero; callers should fill them from the database.
pub fn build_local_account(
    domain: &str,
    id: &str,
    username: &str,
    display_name: &str,
    bio_html: &str,
    created_at: &str,
) -> Account {
    Account {
        id: id.to_string(),
        username: username.to_string(),
        acct: username.to_string(),
        display_name: display_name.to_string(),
        note: bio_html.to_string(),
        url: format!("https://{domain}/@{username}"),
        avatar: format!("https://{domain}/avatars/original/missing.png"),
        avatar_static: format!("https://{domain}/avatars/original/missing.png"),
        header: format!("https://{domain}/headers/original/missing.png"),
        header_static: format!("https://{domain}/headers/original/missing.png"),
        locked: false,
        bot: false,
        discoverable: Some(true),
        created_at: created_at.to_string(),
        last_status_at: None,
        statuses_count: 0,
        followers_count: 0,
        following_count: 0,
        fields: Vec::new(),
        emojis: Vec::new(),
        did: None,
    }
}

/// Build an [`Account`] for a remote (federated) user.
///
/// The `acct` field is set to `username@domain` per Mastodon convention.
pub fn build_remote_account(
    id: &str,
    username: &str,
    domain: &str,
    display_name: &str,
    bio_html: &str,
    url: &str,
    avatar: &str,
    created_at: &str,
) -> Account {
    Account {
        id: id.to_string(),
        username: username.to_string(),
        acct: format!("{username}@{domain}"),
        display_name: display_name.to_string(),
        note: bio_html.to_string(),
        url: url.to_string(),
        avatar: avatar.to_string(),
        avatar_static: avatar.to_string(),
        header: String::new(),
        header_static: String::new(),
        locked: false,
        bot: false,
        discoverable: None,
        created_at: created_at.to_string(),
        last_status_at: None,
        statuses_count: 0,
        followers_count: 0,
        following_count: 0,
        fields: Vec::new(),
        emojis: Vec::new(),
        did: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_account_json_field_names() {
        let acct = build_local_account(
            "example.com",
            "12345",
            "alice",
            "Alice",
            "<p>Hello world</p>",
            "2024-01-15T00:00:00.000Z",
        );
        let json = serde_json::to_value(&acct).unwrap();
        let obj = json.as_object().unwrap();

        // Mastodon field names use snake_case
        assert!(obj.contains_key("followers_count"));
        assert!(obj.contains_key("following_count"));
        assert!(obj.contains_key("statuses_count"));
        assert!(obj.contains_key("display_name"));
        assert!(obj.contains_key("avatar_static"));
        assert!(obj.contains_key("header_static"));
        assert!(obj.contains_key("created_at"));
        // last_status_at is None by default, so it should be omitted
        assert!(!obj.contains_key("last_status_at"));

        // Values
        assert_eq!(obj["id"], "12345");
        assert_eq!(obj["username"], "alice");
        assert_eq!(obj["acct"], "alice");
        assert_eq!(obj["url"], "https://example.com/@alice");
        assert_eq!(obj["followers_count"], 0);
        assert_eq!(obj["statuses_count"], 0);
        assert_eq!(obj["locked"], false);
        assert_eq!(obj["bot"], false);
        assert_eq!(obj["emojis"], serde_json::json!([]));
    }

    #[test]
    fn optional_fields_omitted_when_none() {
        let mut acct = build_local_account(
            "example.com",
            "1",
            "bob",
            "Bob",
            "",
            "2024-01-01T00:00:00Z",
        );
        acct.discoverable = None;
        acct.last_status_at = None;
        acct.did = None;

        let json = serde_json::to_value(&acct).unwrap();
        let obj = json.as_object().unwrap();

        assert!(!obj.contains_key("discoverable"));
        assert!(!obj.contains_key("last_status_at"));
        assert!(!obj.contains_key("did"));
    }

    #[test]
    fn optional_fields_present_when_some() {
        let mut acct = build_local_account(
            "example.com",
            "1",
            "bob",
            "Bob",
            "",
            "2024-01-01T00:00:00Z",
        );
        acct.discoverable = Some(true);
        acct.last_status_at = Some("2024-06-15".to_string());
        acct.did = Some("did:scid:abc123".to_string());

        let json = serde_json::to_value(&acct).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj["discoverable"], true);
        assert_eq!(obj["last_status_at"], "2024-06-15");
        assert_eq!(obj["did"], "did:scid:abc123");
    }

    #[test]
    fn credential_account_includes_source() {
        let acct = build_local_account(
            "example.com",
            "1",
            "alice",
            "Alice",
            "<p>Hi</p>",
            "2024-01-01T00:00:00Z",
        );
        let cred = CredentialAccount {
            account: acct,
            source: AccountSource {
                note: "Hi".to_string(),
                fields: vec![ProfileField {
                    name: "Website".to_string(),
                    value: "https://alice.example".to_string(),
                    verified_at: None,
                }],
                privacy: "public".to_string(),
                sensitive: false,
                language: "en".to_string(),
            },
        };

        let json = serde_json::to_value(&cred).unwrap();
        let obj = json.as_object().unwrap();

        // Flattened account fields at top level
        assert_eq!(obj["username"], "alice");
        assert_eq!(obj["id"], "1");

        // Source sub-object
        let source = obj["source"].as_object().unwrap();
        assert_eq!(source["note"], "Hi");
        assert_eq!(source["privacy"], "public");
        assert_eq!(source["sensitive"], false);
        assert_eq!(source["language"], "en");
        assert_eq!(source["fields"][0]["name"], "Website");
        assert!(!source["fields"][0].as_object().unwrap().contains_key("verified_at"));
    }

    #[test]
    fn remote_account_acct_has_domain() {
        let acct = build_remote_account(
            "99",
            "carol",
            "remote.example",
            "Carol",
            "<p>Remote user</p>",
            "https://remote.example/@carol",
            "https://remote.example/avatars/carol.png",
            "2023-06-01T00:00:00Z",
        );

        assert_eq!(acct.acct, "carol@remote.example");
        assert_eq!(acct.url, "https://remote.example/@carol");

        let json = serde_json::to_value(&acct).unwrap();
        assert_eq!(json["acct"], "carol@remote.example");
    }

    #[test]
    fn profile_field_verified_at_serialization() {
        let field = ProfileField {
            name: "Website".to_string(),
            value: "<a href=\"https://ex.com\">ex.com</a>".to_string(),
            verified_at: Some("2024-03-01T12:00:00Z".to_string()),
        };
        let json = serde_json::to_value(&field).unwrap();
        assert_eq!(json["verified_at"], "2024-03-01T12:00:00Z");

        let unverified = ProfileField {
            name: "Blog".to_string(),
            value: "https://blog.example".to_string(),
            verified_at: None,
        };
        let json = serde_json::to_value(&unverified).unwrap();
        assert!(!json.as_object().unwrap().contains_key("verified_at"));
    }

    #[test]
    fn account_roundtrip_deserialize() {
        let acct = build_local_account(
            "example.com",
            "42",
            "test",
            "Test User",
            "<p>bio</p>",
            "2024-01-01T00:00:00Z",
        );
        let json_str = serde_json::to_string(&acct).unwrap();
        let parsed: Account = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, acct);
    }
}
