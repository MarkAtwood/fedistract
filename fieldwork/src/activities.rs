//! ActivityPub activity and object construction.
//!
//! Provides serde-serializable structs for common ActivityPub activities
//! (Create, Announce, Follow, Accept, Undo, Delete, Update) and builder
//! functions that produce spec-compliant JSON.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The ActivityStreams Public collection URI.
pub const PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";

/// Standard ActivityPub JSON-LD `@context` value (Mastodon-compatible).
pub fn default_context() -> Value {
    json!([
        "https://www.w3.org/ns/activitystreams",
        "https://w3id.org/security/v1"
    ])
}

/// An ActivityPub activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    #[serde(rename = "@context")]
    pub context: Value,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub actor: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub object: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// An ActivityPub Note object (used as the `object` of Create activities).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "attributedTo")]
    pub attributed_to: String,
    pub content: String,
    #[serde(rename = "inReplyTo", skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<bool>,
    #[serde(rename = "summary", skip_serializing_if = "Option::is_none")]
    pub spoiler_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tag: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachment: Vec<Value>,
}

/// Build a `Create{Note}` activity.
///
/// The note ID is `note_id`, and the activity ID is `{note_id}/activity`.
pub fn create_note(
    actor: &str,
    note_id: &str,
    content_html: &str,
    to: &[&str],
    cc: &[&str],
) -> Activity {
    let note = Note {
        id: note_id.to_string(),
        type_: "Note".to_string(),
        attributed_to: actor.to_string(),
        content: content_html.to_string(),
        in_reply_to: None,
        sensitive: None,
        spoiler_text: None,
        url: None,
        to: to.iter().map(|s| s.to_string()).collect(),
        cc: cc.iter().map(|s| s.to_string()).collect(),
        published: None,
        tag: Vec::new(),
        attachment: Vec::new(),
    };

    Activity {
        context: default_context(),
        id: format!("{note_id}/activity"),
        type_: "Create".to_string(),
        actor: actor.to_string(),
        to: to.iter().map(|s| s.to_string()).collect(),
        cc: cc.iter().map(|s| s.to_string()).collect(),
        object: serde_json::to_value(&note).expect("Note serialization is infallible"),
        published: None,
        target: None,
    }
}

/// Build an `Announce` (boost) activity.
///
/// The activity ID is `{actor}#announces/{object_uri_hash}` where the hash
/// is a truncated representation. Callers who need a specific ID can overwrite
/// the `id` field after construction.
pub fn announce(actor: &str, object_uri: &str, to: &[&str]) -> Activity {
    Activity {
        context: default_context(),
        id: format!("{actor}#announces/{}", stable_hash(object_uri)),
        type_: "Announce".to_string(),
        actor: actor.to_string(),
        to: to.iter().map(|s| s.to_string()).collect(),
        cc: Vec::new(),
        object: json!(object_uri),
        published: None,
        target: None,
    }
}

/// Build a `Follow` activity.
pub fn follow(actor: &str, target: &str) -> Activity {
    Activity {
        context: default_context(),
        id: format!("{actor}#follows/{}", stable_hash(target)),
        type_: "Follow".to_string(),
        actor: actor.to_string(),
        to: vec![target.to_string()],
        cc: Vec::new(),
        object: json!(target),
        published: None,
        target: None,
    }
}

/// Build an `Accept{Follow}` activity.
///
/// `follow_id` is the `id` of the Follow activity being accepted.
pub fn accept_follow(actor: &str, follow_id: &str) -> Activity {
    Activity {
        context: default_context(),
        id: format!("{actor}#accepts/{}", stable_hash(follow_id)),
        type_: "Accept".to_string(),
        actor: actor.to_string(),
        to: Vec::new(),
        cc: Vec::new(),
        object: json!({
            "id": follow_id,
            "type": "Follow"
        }),
        published: None,
        target: None,
    }
}

/// Build an `Undo` activity wrapping an arbitrary object.
pub fn undo(actor: &str, object: Value) -> Activity {
    let object_id_str = object["id"].as_str().unwrap_or("unknown");
    Activity {
        context: default_context(),
        id: format!("{actor}#undoes/{}", stable_hash(object_id_str)),
        type_: "Undo".to_string(),
        actor: actor.to_string(),
        to: Vec::new(),
        cc: Vec::new(),
        object,
        published: None,
        target: None,
    }
}

/// Build a `Delete` activity for an object URI.
///
/// The object is serialized as a Tombstone.
pub fn delete(actor: &str, object_uri: &str) -> Activity {
    Activity {
        context: default_context(),
        id: format!("{actor}#deletes/{}", stable_hash(object_uri)),
        type_: "Delete".to_string(),
        actor: actor.to_string(),
        to: vec![PUBLIC.to_string()],
        cc: Vec::new(),
        object: json!({
            "id": object_uri,
            "type": "Tombstone"
        }),
        published: None,
        target: None,
    }
}

/// Build an `Update` activity for an actor.
///
/// The object is the actor URI itself; the caller should replace `object`
/// with the full actor document before delivery.
pub fn update_actor(actor: &str) -> Activity {
    Activity {
        context: default_context(),
        id: format!("{actor}#updates/{}", now_epoch_secs()),
        type_: "Update".to_string(),
        actor: actor.to_string(),
        to: vec![PUBLIC.to_string()],
        cc: Vec::new(),
        object: json!(actor),
        published: None,
        target: None,
    }
}

// ponytail: FNV-1a 64-bit truncated to 16 hex chars for deterministic, compact IDs.
// Collision probability is negligible for activity IDs within a single actor's namespace.
fn stable_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_note_json_structure() {
        let actor = "https://example.com/users/alice";
        let note_id = "https://example.com/users/alice/statuses/123";
        let content = "<p>Hello, world!</p>";

        let activity = create_note(
            actor,
            note_id,
            content,
            &[PUBLIC],
            &["https://example.com/users/alice/followers"],
        );

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["@context"][0], "https://www.w3.org/ns/activitystreams");
        assert_eq!(json["type"], "Create");
        assert_eq!(json["actor"], actor);
        assert_eq!(
            json["id"],
            "https://example.com/users/alice/statuses/123/activity"
        );
        assert_eq!(json["to"][0], PUBLIC);
        assert_eq!(
            json["cc"][0],
            "https://example.com/users/alice/followers"
        );

        // Object (Note) checks
        assert_eq!(json["object"]["type"], "Note");
        assert_eq!(json["object"]["id"], note_id);
        assert_eq!(json["object"]["attributedTo"], actor);
        assert_eq!(json["object"]["content"], content);
        assert_eq!(json["object"]["to"][0], PUBLIC);
    }

    #[test]
    fn announce_json_structure() {
        let actor = "https://example.com/users/alice";
        let object_uri = "https://remote.example/users/bob/statuses/456";

        let activity = announce(actor, object_uri, &[PUBLIC]);

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Announce");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"], object_uri);
        assert_eq!(json["to"][0], PUBLIC);
        assert!(json["id"].as_str().unwrap().starts_with(actor));
    }

    #[test]
    fn follow_json_structure() {
        let actor = "https://example.com/users/alice";
        let target = "https://remote.example/users/bob";

        let activity = follow(actor, target);

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Follow");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"], target);
        assert_eq!(json["to"][0], target);
    }

    #[test]
    fn accept_follow_json_structure() {
        let actor = "https://example.com/users/alice";
        let follow_id = "https://remote.example/activities/follow-abc123";

        let activity = accept_follow(actor, follow_id);

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Accept");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"]["type"], "Follow");
        assert_eq!(json["object"]["id"], follow_id);
    }

    #[test]
    fn undo_json_structure() {
        let actor = "https://example.com/users/alice";
        let inner_follow = json!({
            "id": "https://example.com/users/alice#follows/abc",
            "type": "Follow",
            "actor": actor,
            "object": "https://remote.example/users/bob"
        });

        let activity = undo(actor, inner_follow.clone());

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Undo");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"]["type"], "Follow");
        assert_eq!(json["object"]["id"], inner_follow["id"]);
    }

    #[test]
    fn delete_json_structure() {
        let actor = "https://example.com/users/alice";
        let object_uri = "https://example.com/users/alice/statuses/789";

        let activity = delete(actor, object_uri);

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Delete");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"]["type"], "Tombstone");
        assert_eq!(json["object"]["id"], object_uri);
        assert_eq!(json["to"][0], PUBLIC);
    }

    #[test]
    fn update_actor_json_structure() {
        let actor = "https://example.com/users/alice";

        let activity = update_actor(actor);

        let json = serde_json::to_value(&activity).unwrap();

        assert_eq!(json["type"], "Update");
        assert_eq!(json["actor"], actor);
        assert_eq!(json["object"], actor);
        assert_eq!(json["to"][0], PUBLIC);
    }

    #[test]
    fn note_serialization_skips_empty_optionals() {
        let note = Note {
            id: "https://example.com/note/1".to_string(),
            type_: "Note".to_string(),
            attributed_to: "https://example.com/users/alice".to_string(),
            content: "<p>hi</p>".to_string(),
            in_reply_to: None,
            sensitive: None,
            spoiler_text: None,
            url: None,
            to: vec![PUBLIC.to_string()],
            cc: Vec::new(),
            published: None,
            tag: Vec::new(),
            attachment: Vec::new(),
        };

        let json = serde_json::to_value(&note).unwrap();

        // Optional fields that are None should be absent
        assert!(json.get("inReplyTo").is_none());
        assert!(json.get("sensitive").is_none());
        assert!(json.get("summary").is_none());
        assert!(json.get("url").is_none());
        assert!(json.get("published").is_none());
        // Empty vecs with skip_serializing_if should be absent
        assert!(json.get("tag").is_none());
        assert!(json.get("attachment").is_none());
    }

    #[test]
    fn note_serialization_includes_present_optionals() {
        let note = Note {
            id: "https://example.com/note/2".to_string(),
            type_: "Note".to_string(),
            attributed_to: "https://example.com/users/alice".to_string(),
            content: "<p>CW post</p>".to_string(),
            in_reply_to: Some("https://example.com/note/1".to_string()),
            sensitive: Some(true),
            spoiler_text: Some("Content warning".to_string()),
            url: Some("https://example.com/@alice/2".to_string()),
            to: vec![PUBLIC.to_string()],
            cc: vec!["https://example.com/users/alice/followers".to_string()],
            published: Some("2025-01-01T00:00:00Z".to_string()),
            tag: vec![json!({"type": "Hashtag", "name": "#test"})],
            attachment: vec![json!({"type": "Document", "url": "https://example.com/image.png"})],
        };

        let json = serde_json::to_value(&note).unwrap();

        assert_eq!(json["inReplyTo"], "https://example.com/note/1");
        assert_eq!(json["sensitive"], true);
        assert_eq!(json["summary"], "Content warning");
        assert_eq!(json["url"], "https://example.com/@alice/2");
        assert_eq!(json["published"], "2025-01-01T00:00:00Z");
        assert_eq!(json["tag"][0]["type"], "Hashtag");
        assert_eq!(json["attachment"][0]["type"], "Document");
    }

    #[test]
    fn activity_roundtrip_deserialization() {
        let activity = create_note(
            "https://example.com/users/alice",
            "https://example.com/users/alice/statuses/1",
            "<p>test</p>",
            &[PUBLIC],
            &[],
        );

        let json_str = serde_json::to_string(&activity).unwrap();
        let parsed: Activity = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.type_, "Create");
        assert_eq!(parsed.actor, "https://example.com/users/alice");
        assert_eq!(parsed.object["type"], "Note");
    }

    #[test]
    fn stable_hash_is_deterministic() {
        let h1 = stable_hash("https://example.com/users/bob");
        let h2 = stable_hash("https://example.com/users/bob");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn stable_hash_differs_for_different_inputs() {
        let h1 = stable_hash("https://example.com/users/alice");
        let h2 = stable_hash("https://example.com/users/bob");
        assert_ne!(h1, h2);
    }

    #[test]
    fn public_constant_matches_activitystreams_spec() {
        assert_eq!(PUBLIC, "https://www.w3.org/ns/activitystreams#Public");
    }
}
