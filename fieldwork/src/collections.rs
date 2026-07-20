//! ActivityPub `OrderedCollection` and `OrderedCollectionPage` builders.
//!
//! Provides serde-serializable structs for the two collection types used
//! throughout ActivityPub (followers, following, outbox, featured, etc.)
//! plus pagination helpers extracted from broadside's pentest-hardened
//! implementation.

use serde::Serialize;

/// ActivityStreams context URL, used as `@context` in all collection documents.
const AS_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";

/// Top-level ordered collection (e.g. `/users/{username}/outbox`).
///
/// When the collection is paginated, `first` points to the first page URL.
/// When it is not paginated, items live directly on the collection (not modelled
/// here — use [`OrderedCollectionPage`] inline instead).
#[derive(Debug, Clone, Serialize)]
pub struct OrderedCollection {
    #[serde(rename = "@context")]
    pub context: serde_json::Value,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "totalItems")]
    pub total_items: u64,
    #[serde(rename = "first", skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(rename = "last", skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// One page of an ordered collection (e.g. `/users/{username}/outbox?page=2`).
#[derive(Debug, Clone, Serialize)]
pub struct OrderedCollectionPage {
    #[serde(rename = "@context")]
    pub context: serde_json::Value,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "partOf")]
    pub part_of: String,
    #[serde(rename = "totalItems")]
    pub total_items: u64,
    #[serde(rename = "next", skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(rename = "prev", skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
    #[serde(rename = "orderedItems")]
    pub ordered_items: Vec<serde_json::Value>,
}

/// Build a top-level [`OrderedCollection`] with optional pagination links.
///
/// If the collection has any items, `first` is set to `{id}?page=1`.
/// `last` is left empty; callers can set it if they know the final page.
pub fn build_collection(id: &str, total_items: u64) -> OrderedCollection {
    let first = if total_items > 0 {
        Some(format!("{id}?page=1"))
    } else {
        None
    };
    OrderedCollection {
        context: serde_json::Value::String(AS_CONTEXT.to_owned()),
        id: id.to_owned(),
        type_: "OrderedCollection".to_owned(),
        total_items,
        first,
        last: None,
    }
}

/// Build one page of an [`OrderedCollectionPage`].
///
/// Computes `next` and `prev` links automatically from `page`, `total`, and
/// `per_page`. Page numbering is 1-based.
pub fn build_collection_page(
    collection_id: &str,
    page: u32,
    items: Vec<serde_json::Value>,
    total: u64,
    per_page: u32,
) -> OrderedCollectionPage {
    let last_page = if total == 0 {
        1
    } else {
        // ceiling division without overflow
        ((total.saturating_sub(1)) / u64::from(per_page)) + 1
    };

    let next = if u64::from(page) < last_page {
        Some(format!("{collection_id}?page={}", page + 1))
    } else {
        None
    };

    let prev = if page > 1 {
        Some(format!("{collection_id}?page={}", page - 1))
    } else {
        None
    };

    OrderedCollectionPage {
        context: serde_json::Value::String(AS_CONTEXT.to_owned()),
        id: format!("{collection_id}?page={page}"),
        type_: "OrderedCollectionPage".to_owned(),
        part_of: collection_id.to_owned(),
        total_items: total,
        next,
        prev,
        ordered_items: items,
    }
}

/// Compute a SQL-friendly byte offset from a 1-based page number.
///
/// Uses saturating arithmetic to prevent overflow — the broadside pentest
/// pattern. Returns `u64` suitable for use as a `LIMIT … OFFSET` value.
pub fn paginate_offset(page: u32, per_page: u32) -> u64 {
    u64::from(page)
        .saturating_sub(1)
        .saturating_mul(u64::from(per_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_collection_empty() {
        let c = build_collection("https://example.com/users/alice/outbox", 0);
        assert_eq!(c.type_, "OrderedCollection");
        assert_eq!(c.total_items, 0);
        assert!(c.first.is_none());
        assert!(c.last.is_none());
    }

    #[test]
    fn build_collection_with_items() {
        let c = build_collection("https://example.com/users/alice/outbox", 42);
        assert_eq!(c.total_items, 42);
        assert_eq!(
            c.first.as_deref(),
            Some("https://example.com/users/alice/outbox?page=1")
        );
    }

    #[test]
    fn build_collection_serializes_correctly() {
        let c = build_collection("https://example.com/users/alice/followers", 5);
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["@context"], "https://www.w3.org/ns/activitystreams");
        assert_eq!(json["type"], "OrderedCollection");
        assert_eq!(json["totalItems"], 5);
        assert_eq!(
            json["first"],
            "https://example.com/users/alice/followers?page=1"
        );
        // `last` should be absent, not null
        assert!(json.get("last").is_none());
    }

    #[test]
    fn build_page_first_of_many() {
        let items = vec![serde_json::json!({"id": "1"}), serde_json::json!({"id": "2"})];
        let p = build_collection_page(
            "https://example.com/users/alice/outbox",
            1,
            items,
            50,
            20,
        );
        assert_eq!(p.type_, "OrderedCollectionPage");
        assert_eq!(
            p.part_of,
            "https://example.com/users/alice/outbox"
        );
        assert!(p.prev.is_none());
        assert_eq!(
            p.next.as_deref(),
            Some("https://example.com/users/alice/outbox?page=2")
        );
        assert_eq!(p.ordered_items.len(), 2);
    }

    #[test]
    fn build_page_middle() {
        let p = build_collection_page(
            "https://example.com/users/alice/outbox",
            2,
            vec![],
            50,
            20,
        );
        assert_eq!(
            p.prev.as_deref(),
            Some("https://example.com/users/alice/outbox?page=1")
        );
        assert_eq!(
            p.next.as_deref(),
            Some("https://example.com/users/alice/outbox?page=3")
        );
    }

    #[test]
    fn build_page_last() {
        let p = build_collection_page(
            "https://example.com/users/alice/outbox",
            3,
            vec![],
            50,
            20,
        );
        assert_eq!(
            p.prev.as_deref(),
            Some("https://example.com/users/alice/outbox?page=2")
        );
        assert!(p.next.is_none());
    }

    #[test]
    fn build_page_single_page() {
        let p = build_collection_page(
            "https://example.com/users/alice/outbox",
            1,
            vec![serde_json::json!("item")],
            1,
            20,
        );
        assert!(p.prev.is_none());
        assert!(p.next.is_none());
    }

    #[test]
    fn build_page_serializes_correctly() {
        let items = vec![serde_json::json!({"type": "Note"})];
        let p = build_collection_page("https://example.com/col", 1, items, 1, 20);
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["@context"], "https://www.w3.org/ns/activitystreams");
        assert_eq!(json["type"], "OrderedCollectionPage");
        assert_eq!(json["partOf"], "https://example.com/col");
        assert_eq!(json["totalItems"], 1);
        assert!(json.get("next").is_none());
        assert!(json.get("prev").is_none());
        assert_eq!(json["orderedItems"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn paginate_offset_page_one() {
        assert_eq!(paginate_offset(1, 20), 0);
    }

    #[test]
    fn paginate_offset_page_two() {
        assert_eq!(paginate_offset(2, 20), 20);
    }

    #[test]
    fn paginate_offset_page_zero_saturates() {
        // page 0 is invalid but should not panic or wrap
        assert_eq!(paginate_offset(0, 20), 0);
    }

    #[test]
    fn paginate_offset_large_values_no_overflow() {
        // (u32::MAX - 1) * u32::MAX fits in u64 without wrapping
        let result = paginate_offset(u32::MAX, u32::MAX);
        let expected = (u32::MAX as u64 - 1) * u32::MAX as u64;
        assert_eq!(result, expected);
    }

    #[test]
    fn paginate_offset_boundary() {
        assert_eq!(paginate_offset(3, 10), 20);
        assert_eq!(paginate_offset(1, 100), 0);
        assert_eq!(paginate_offset(5, 25), 100);
    }
}
