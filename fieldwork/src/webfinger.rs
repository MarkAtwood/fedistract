//! WebFinger (RFC 7033) data types and response construction.
//!
//! Provides JRD (JSON Resource Descriptor) structs for building standard
//! fediverse WebFinger responses, plus `acct:` URI parsing. No HTTP handler
//! is included — that is framework-specific.

use serde::{Deserialize, Serialize};

/// JSON Resource Descriptor response per RFC 7033 Section 4.4.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebFingerResponse {
    pub subject: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub links: Vec<WebFingerLink>,
}

/// A single link in a JRD response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebFingerLink {
    pub rel: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

/// Build a standard fediverse WebFinger response.
///
/// Includes:
/// - `rel=self` link to the actor URL (`application/activity+json`)
/// - `rel=http://webfinger.net/rel/profile-page` link to the profile page
/// - `rel=did` link to the DID document URL (if `did` is provided)
/// - Aliases for the actor URL and optionally the DID
pub fn build_webfinger_response(
    domain: &str,
    username: &str,
    actor_url: &str,
    did: Option<&str>,
) -> WebFingerResponse {
    let profile_url = format!("https://{domain}/@{username}");

    let mut aliases = vec![actor_url.to_string()];
    if let Some(did) = did {
        aliases.push(did.to_string());
    }

    let mut links = vec![
        WebFingerLink {
            rel: "http://webfinger.net/rel/profile-page".into(),
            type_: Some("text/html".into()),
            href: Some(profile_url),
            template: None,
        },
        WebFingerLink {
            rel: "self".into(),
            type_: Some("application/activity+json".into()),
            href: Some(actor_url.to_string()),
            template: None,
        },
    ];

    if let Some(did) = did {
        links.push(WebFingerLink {
            rel: "did".into(),
            type_: Some("application/did+json".into()),
            href: Some(did.to_string()),
            template: None,
        });
    }

    WebFingerResponse {
        subject: format!("acct:{username}@{domain}"),
        aliases,
        links,
    }
}

/// Parse an `acct:user@domain` URI, returning `(username, domain)`.
///
/// Returns `None` if the input does not start with `acct:` or lacks a `@` separator.
pub fn parse_acct_uri(resource: &str) -> Option<(String, String)> {
    let acct = resource.strip_prefix("acct:")?;
    let (username, domain) = acct.split_once('@')?;
    if username.is_empty() || domain.is_empty() {
        return None;
    }
    Some((username.to_string(), domain.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_acct_valid() {
        let result = parse_acct_uri("acct:alice@example.com");
        assert_eq!(
            result,
            Some(("alice".into(), "example.com".into()))
        );
    }

    #[test]
    fn parse_acct_missing_prefix() {
        assert_eq!(parse_acct_uri("alice@example.com"), None);
    }

    #[test]
    fn parse_acct_missing_at() {
        assert_eq!(parse_acct_uri("acct:alice"), None);
    }

    #[test]
    fn parse_acct_empty_username() {
        assert_eq!(parse_acct_uri("acct:@example.com"), None);
    }

    #[test]
    fn parse_acct_empty_domain() {
        assert_eq!(parse_acct_uri("acct:alice@"), None);
    }

    #[test]
    fn build_response_without_did() {
        let resp = build_webfinger_response(
            "example.com",
            "alice",
            "https://example.com/users/alice",
            None,
        );

        assert_eq!(resp.subject, "acct:alice@example.com");
        assert_eq!(resp.aliases, vec!["https://example.com/users/alice"]);
        assert_eq!(resp.links.len(), 2);

        assert_eq!(resp.links[0].rel, "http://webfinger.net/rel/profile-page");
        assert_eq!(resp.links[0].type_.as_deref(), Some("text/html"));
        assert_eq!(
            resp.links[0].href.as_deref(),
            Some("https://example.com/@alice")
        );

        assert_eq!(resp.links[1].rel, "self");
        assert_eq!(
            resp.links[1].type_.as_deref(),
            Some("application/activity+json")
        );
        assert_eq!(
            resp.links[1].href.as_deref(),
            Some("https://example.com/users/alice")
        );
    }

    #[test]
    fn build_response_with_did() {
        let did = "did:web:example.com:users:alice";
        let resp = build_webfinger_response(
            "example.com",
            "alice",
            "https://example.com/users/alice",
            Some(did),
        );

        assert_eq!(resp.aliases.len(), 2);
        assert_eq!(resp.aliases[1], did);
        assert_eq!(resp.links.len(), 3);

        assert_eq!(resp.links[2].rel, "did");
        assert_eq!(resp.links[2].type_.as_deref(), Some("application/did+json"));
        assert_eq!(resp.links[2].href.as_deref(), Some(did));
    }

    #[test]
    fn response_serializes_to_valid_jrd() {
        let resp = build_webfinger_response(
            "example.com",
            "bob",
            "https://example.com/users/bob",
            None,
        );
        let json = serde_json::to_value(&resp).unwrap();

        assert_eq!(json["subject"], "acct:bob@example.com");
        // "type" field name in JSON, not "type_"
        assert_eq!(json["links"][0]["type"], "text/html");
        assert_eq!(json["links"][1]["type"], "application/activity+json");
        // template should be absent (not null) due to skip_serializing_if
        assert!(json["links"][0].get("template").is_none());
    }

    #[test]
    fn response_roundtrips_through_json() {
        let original = build_webfinger_response(
            "example.com",
            "carol",
            "https://example.com/users/carol",
            Some("did:web:example.com:users:carol"),
        );
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: WebFingerResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn empty_aliases_omitted_in_json() {
        let resp = WebFingerResponse {
            subject: "acct:test@example.com".into(),
            aliases: vec![],
            links: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("aliases").is_none());
    }
}
