//! NodeInfo 2.1 data types and construction helpers.
//!
//! Produces the JSON structures for:
//! - `/.well-known/nodeinfo` (discovery document)
//! - `/nodeinfo/2.1` (full NodeInfo response)
//!
//! No HTTP handler — callers wire these into their own router.

use serde::{Deserialize, Serialize};

/// NodeInfo 2.1 root document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub version: String,
    pub software: Software,
    pub protocols: Vec<String>,
    pub services: Services,
    pub usage: Usage,
    pub open_registrations: bool,
    pub metadata: serde_json::Value,
}

/// Software identification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Software {
    pub name: String,
    pub version: String,
}

/// Supported third-party service integrations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Services {
    pub inbound: Vec<String>,
    pub outbound: Vec<String>,
}

/// Server usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub users: UserStats,
    pub local_posts: u64,
}

/// User activity statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserStats {
    pub total: u64,
    pub active_month: u64,
    pub active_half_year: u64,
}

/// Build a complete NodeInfo 2.1 document.
pub fn build_nodeinfo(
    software_name: &str,
    software_version: &str,
    total_users: u64,
    active_month: u64,
    active_half_year: u64,
    local_posts: u64,
    open_registrations: bool,
) -> NodeInfo {
    NodeInfo {
        version: "2.1".to_string(),
        software: Software {
            name: software_name.to_string(),
            version: software_version.to_string(),
        },
        protocols: vec!["activitypub".to_string()],
        services: Services {
            inbound: Vec::new(),
            outbound: Vec::new(),
        },
        usage: Usage {
            users: UserStats {
                total: total_users,
                active_month,
                active_half_year,
            },
            local_posts,
        },
        open_registrations,
        metadata: serde_json::json!({}),
    }
}

/// Build the `.well-known/nodeinfo` discovery document.
///
/// `base_url` should be the server's public origin, e.g. `"https://example.com"`.
pub fn well_known_nodeinfo(base_url: &str) -> serde_json::Value {
    serde_json::json!({
        "links": [{
            "rel": "http://nodeinfo.diaspora.software/ns/schema/2.1",
            "href": format!("{}/nodeinfo/2.1", base_url.trim_end_matches('/'))
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_nodeinfo_fields() {
        let ni = build_nodeinfo("testserver", "0.1.0", 42, 10, 30, 100, false);

        assert_eq!(ni.version, "2.1");
        assert_eq!(ni.software.name, "testserver");
        assert_eq!(ni.software.version, "0.1.0");
        assert_eq!(ni.protocols, vec!["activitypub"]);
        assert!(ni.services.inbound.is_empty());
        assert!(ni.services.outbound.is_empty());
        assert_eq!(ni.usage.users.total, 42);
        assert_eq!(ni.usage.users.active_month, 10);
        assert_eq!(ni.usage.users.active_half_year, 30);
        assert_eq!(ni.usage.local_posts, 100);
        assert!(!ni.open_registrations);
        assert_eq!(ni.metadata, serde_json::json!({}));
    }

    #[test]
    fn nodeinfo_serializes_camel_case() {
        let ni = build_nodeinfo("x", "1.0", 1, 1, 1, 5, true);
        let json = serde_json::to_value(&ni).unwrap();

        // camelCase fields
        assert!(json.get("openRegistrations").is_some());
        assert!(json.get("open_registrations").is_none());
        assert!(json["usage"].get("localPosts").is_some());
        assert!(json["usage"]["users"].get("activeMonth").is_some());
        assert!(json["usage"]["users"].get("activeHalfYear").is_some());
    }

    #[test]
    fn nodeinfo_round_trips() {
        let ni = build_nodeinfo("demo", "2.0.0", 5, 3, 4, 20, true);
        let json_str = serde_json::to_string(&ni).unwrap();
        let deserialized: NodeInfo = serde_json::from_str(&json_str).unwrap();
        assert_eq!(ni, deserialized);
    }

    #[test]
    fn well_known_nodeinfo_structure() {
        let doc = well_known_nodeinfo("https://example.com");
        let links = doc["links"].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0]["rel"],
            "http://nodeinfo.diaspora.software/ns/schema/2.1"
        );
        assert_eq!(links[0]["href"], "https://example.com/nodeinfo/2.1");
    }

    #[test]
    fn well_known_strips_trailing_slash() {
        let doc = well_known_nodeinfo("https://example.com/");
        let href = doc["links"][0]["href"].as_str().unwrap();
        assert_eq!(href, "https://example.com/nodeinfo/2.1");
    }
}
