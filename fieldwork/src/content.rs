//! Hashtag, mention, and URL detection for ActivityPub content.
//!
//! Processes HTML content to linkify hashtags, mentions, and bare URLs while
//! skipping text already inside `<a>` tags. Also provides FEP-e232 quote
//! link detection and language detection.

use regex::Regex;
use std::sync::LazyLock;

static HASHTAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s>])#([a-zA-Z][a-zA-Z0-9_]*)").unwrap());

static MENTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s>])@([a-zA-Z0-9_]+)@([a-zA-Z0-9][a-zA-Z0-9.\-]+[a-zA-Z0-9])").unwrap()
});

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://[^\s<>\x22')\]]+[a-zA-Z0-9/]").unwrap());

static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<a\s[^>]*>[\s\S]*?</a>").unwrap());

/// ActivityPub tag type.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub enum TagType {
    Hashtag,
    Mention,
}

/// A tag entry for the ActivityPub Note object.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Tag {
    #[serde(rename = "type")]
    pub tag_type: TagType,
    pub href: String,
    pub name: String,
}

/// Process HTML content: linkify hashtags, mentions, and bare URLs.
///
/// Returns `(processed_html, tags)` where `tags` is suitable for the
/// ActivityPub Note's `tag` array. Skips anything already inside `<a>` tags.
/// Applies replacements right-to-left so byte offsets stay valid.
pub fn process_content(html: &str, domain: &str) -> (String, Vec<Tag>) {
    let mut tags = Vec::new();

    let skip_ranges: Vec<(usize, usize)> = LINK_RE
        .find_iter(html)
        .map(|m| (m.start(), m.end()))
        .collect();

    let in_link = |pos: usize| -> bool {
        let idx = skip_ranges.partition_point(|(start, _)| *start <= pos);
        idx > 0 && pos < skip_ranges[idx - 1].1
    };

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    // Bare URLs
    for m in URL_RE.find_iter(html) {
        if in_link(m.start()) {
            continue;
        }
        let url = m.as_str();
        let linked = format!(
            r#"<a href="{url}" rel="nofollow noopener noreferrer" target="_blank">{url}</a>"#
        );
        replacements.push((m.start(), m.end(), linked));
    }

    // Hashtags
    for cap in HASHTAG_RE.captures_iter(html) {
        let name_match = cap.get(1).unwrap();
        let hash_pos = name_match.start() - 1;
        if in_link(hash_pos) {
            continue;
        }
        let name = name_match.as_str();
        let lower = name.to_lowercase();
        let href = format!("https://{domain}/tags/{lower}");
        let linked = format!(r#"<a href="{href}" class="mention hashtag" rel="tag">#{name}</a>"#);
        replacements.push((hash_pos, name_match.end(), linked));

        if !tags.iter().any(|t: &Tag| t.name == format!("#{lower}")) {
            tags.push(Tag {
                tag_type: TagType::Hashtag,
                href,
                name: format!("#{lower}"),
            });
        }
    }

    // Mentions
    for cap in MENTION_RE.captures_iter(html) {
        let user_match = cap.get(1).unwrap();
        let host_match = cap.get(2).unwrap();
        let at_pos = user_match.start() - 1;
        if in_link(at_pos) {
            continue;
        }
        let user = user_match.as_str();
        let host = host_match.as_str();
        let href = format!("https://{host}/@{user}");
        let actor_href = format!("https://{host}/users/{user}");
        let linked = format!(
            r#"<span class="h-card"><a href="{href}" class="u-url mention">@<span>{user}</span></a></span>"#
        );
        replacements.push((at_pos, host_match.end(), linked));

        let mention_name = format!("@{user}@{host}");
        if !tags.iter().any(|t: &Tag| t.name == mention_name) {
            tags.push(Tag {
                tag_type: TagType::Mention,
                href: actor_href,
                name: mention_name,
            });
        }
    }

    replacements.sort_by_key(|r| std::cmp::Reverse(r.0));

    // Remove overlapping replacements (keep earliest start)
    let mut filtered: Vec<(usize, usize, String)> = Vec::with_capacity(replacements.len());
    for r in replacements {
        if filtered.last().map_or(true, |prev| r.1 <= prev.0) {
            filtered.push(r);
        }
    }

    let mut result = html.to_string();
    for (start, end, replacement) in filtered {
        result.replace_range(start..end, &replacement);
    }

    (result, tags)
}

/// Detect FEP-e232 quote post URLs in HTML content.
///
/// Returns Link tag objects suitable for the Note's `tag` array.
/// Excludes URLs inside existing `<a>` tags.
pub fn detect_quote_links(html: &str) -> Vec<serde_json::Value> {
    static QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"https://[a-zA-Z0-9.\-]+/(?:users/[a-zA-Z0-9_]+/statuses/[a-zA-Z0-9\-]+|@[a-zA-Z0-9_]+/[0-9]+)"#)
            .unwrap()
    });

    let skip_ranges: Vec<(usize, usize)> = LINK_RE
        .find_iter(html)
        .map(|m| (m.start(), m.end()))
        .collect();

    let in_link = |pos: usize| -> bool {
        let idx = skip_ranges.partition_point(|(start, _)| *start <= pos);
        idx > 0 && pos < skip_ranges[idx - 1].1
    };

    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for m in QUOTE_RE.find_iter(html) {
        if in_link(m.start()) {
            continue;
        }
        let url = m.as_str();
        if seen.insert(url.to_string()) {
            links.push(serde_json::json!({
                "type": "Link",
                "href": url,
                "mediaType": "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\""
            }));
        }
    }
    links
}

/// Detect the language of plain text content.
///
/// Returns an ISO 639-3 code (e.g. `"eng"`, `"deu"`) if confidence is high
/// enough, or `None` for short or ambiguous text.
pub fn detect_language(text: &str) -> Option<&'static str> {
    if text.len() < 20 {
        return None;
    }
    let info = whatlang::detect(text)?;
    if info.confidence() < 0.8 {
        return None;
    }
    Some(info.lang().code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkifies_hashtag() {
        let (html, tags) = process_content("Hello #rust world", "example.com");
        assert!(html.contains(r#"class="mention hashtag""#));
        assert!(html.contains("tags/rust"));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "#rust");
    }

    #[test]
    fn linkifies_mention() {
        let (html, tags) = process_content("Hello @user@remote.com", "example.com");
        assert!(html.contains(r#"class="u-url mention""#));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "@user@remote.com");
    }

    #[test]
    fn linkifies_bare_url() {
        let (html, _) = process_content("Check https://example.com/page", "example.com");
        assert!(html.contains(r#"<a href="https://example.com/page""#));
    }

    #[test]
    fn skips_existing_links() {
        let input = r#"<a href="https://example.com">#existing</a> #new"#;
        let (html, tags) = process_content(input, "example.com");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "#new");
        assert!(html.contains("#existing</a>"));
    }

    #[test]
    fn deduplicates_hashtags() {
        let (_, tags) = process_content("#Rust and #rust again", "example.com");
        let hashtags: Vec<_> = tags.iter().filter(|t| t.name == "#rust").collect();
        assert_eq!(hashtags.len(), 1);
    }

    #[test]
    fn detect_language_english() {
        let text = "This is a reasonably long English sentence for language detection.";
        assert_eq!(detect_language(text), Some("eng"));
    }

    #[test]
    fn detect_language_too_short() {
        assert_eq!(detect_language("hi"), None);
    }

    #[test]
    fn detect_quote_links_finds_ap_urls() {
        let html = "Quoting https://mastodon.social/users/admin/statuses/12345 here";
        let links = detect_quote_links(html);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["type"], "Link");
    }

    #[test]
    fn detect_quote_links_skips_linked() {
        let html = r#"<a href="https://mastodon.social/users/admin/statuses/12345">link</a>"#;
        let links = detect_quote_links(html);
        assert!(links.is_empty());
    }
}
