//! Pure link-card logic: OG tag parsing, HTML entity decoding, card type
//! classification, and Mastodon-compatible JSON serialization.
//!
//! This module contains no HTTP client, no SSRF validation, and no persistence.
//! Those concerns belong to the consuming application.

use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// CardData
// ---------------------------------------------------------------------------

pub struct CardData {
    pub url: String,
    pub card_type: String,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub author_name: String,
    pub author_url: String,
    pub provider_name: String,
    pub provider_url: String,
    pub html: String,
    pub width: i32,
    pub height: i32,
}

// ---------------------------------------------------------------------------
// URL extraction
// ---------------------------------------------------------------------------

static URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https://[^\s<>\]\)]+").unwrap());

/// Extract the first `https://` URL from raw post text.
pub fn extract_first_url(text: &str) -> Option<String> {
    URL_RE.find(text).map(|m| {
        let url = m
            .as_str()
            .trim_end_matches(['.', ',', ';', ')', ']', '!', '?']);
        url.to_string()
    })
}

// ---------------------------------------------------------------------------
// HTML entity decoding
// ---------------------------------------------------------------------------

/// Decode common HTML entities and numeric character references.
pub fn decode_html_entities(s: &str) -> String {
    static ENTITY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"&#(\d+);|&#x([0-9a-fA-F]+);").unwrap());

    let decoded = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    // Decode numeric entities (&#8217; &#x2019; etc.)
    ENTITY_RE
        .replace_all(&decoded, |caps: &regex::Captures| {
            let code = if let Some(dec) = caps.get(1) {
                dec.as_str().parse::<u32>().ok()
            } else if let Some(hex) = caps.get(2) {
                u32::from_str_radix(hex.as_str(), 16).ok()
            } else {
                None
            };
            code.and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
}

// ---------------------------------------------------------------------------
// OG tag parsing
// ---------------------------------------------------------------------------

/// Parse OpenGraph and Twitter Card meta tags from HTML.
pub fn parse_og_tags(html: &str) -> HashMap<String, String> {
    static META_RE: LazyLock<Regex> = LazyLock::new(|| {
        // Match: <meta property="og:X" content="Y"> or <meta name="twitter:X" content="Y">
        // Also handles reversed attribute order: content="Y" property="og:X"
        Regex::new(
            r#"(?i)<meta\s+(?:[^>]*?\s)?(?:(?:property|name)\s*=\s*"((?:og|twitter|article):[^"]+)"[^>]*?\scontent\s*=\s*"([^"]*)"|content\s*=\s*"([^"]*)"[^>]*?(?:property|name)\s*=\s*"((?:og|twitter|article):[^"]+)")"#,
        )
        .unwrap()
    });

    let mut tags = HashMap::new();
    for cap in META_RE.captures_iter(html) {
        let (key, value) = if let (Some(k), Some(v)) = (cap.get(1), cap.get(2)) {
            (k.as_str().to_string(), v.as_str().to_string())
        } else if let (Some(v), Some(k)) = (cap.get(3), cap.get(4)) {
            (k.as_str().to_string(), v.as_str().to_string())
        } else {
            continue;
        };
        // Don't overwrite — first occurrence wins
        tags.entry(key).or_insert(value);
    }
    tags
}

// ---------------------------------------------------------------------------
// HTML <title> extraction
// ---------------------------------------------------------------------------

/// Extract `<title>` content as fallback when OG tags are missing.
pub fn parse_html_title(html: &str) -> Option<String> {
    static TITLE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<title[^>]*>([^<]+)</title>").unwrap());

    TITLE_RE.captures(html).map(|c| c[1].trim().to_string())
}

// ---------------------------------------------------------------------------
// Card type classification
// ---------------------------------------------------------------------------

/// Classify a card as `"video"`, `"photo"`, or `"link"` based on OG metadata.
///
/// `has_video_tag` should be true when the HTML contains an `og:video` meta tag,
/// even if `og:type` doesn't say "video".
pub fn classify_card_type(og_type: &str, has_video_tag: bool, has_image: bool, has_title: bool) -> String {
    if og_type.contains("video") || has_video_tag {
        "video".to_string()
    } else if has_image && !has_title {
        "photo".to_string()
    } else {
        "link".to_string()
    }
}

// ---------------------------------------------------------------------------
// Card JSON serialization (Mastodon format)
// ---------------------------------------------------------------------------

pub fn card_to_json(card: &CardData) -> Value {
    json!({
        "url": card.url,
        "title": card.title,
        "description": card.description,
        "type": card.card_type,
        "author_name": card.author_name,
        "author_url": card.author_url,
        "provider_name": card.provider_name,
        "provider_url": card.provider_url,
        "html": card.html,
        "width": card.width,
        "height": card.height,
        "image": card.image_url,
        "embed_url": "",
        "blurhash": null,
        "published_at": null
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_url_basic() {
        let text = "Check this out https://example.com/page and more";
        assert_eq!(
            extract_first_url(text),
            Some("https://example.com/page".to_string())
        );
    }

    #[test]
    fn extract_url_strips_trailing_punctuation() {
        assert_eq!(
            extract_first_url("See https://example.com."),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn extract_url_none_for_http() {
        assert_eq!(extract_first_url("http://example.com"), None);
    }

    #[test]
    fn decode_entities() {
        assert_eq!(decode_html_entities("&amp; &lt; &gt;"), "& < >");
        assert_eq!(decode_html_entities("&#8217;"), "\u{2019}");
        assert_eq!(decode_html_entities("&#x2019;"), "\u{2019}");
    }

    #[test]
    fn parse_og_tags_basic() {
        let html = r#"<meta property="og:title" content="Hello World">"#;
        let tags = parse_og_tags(html);
        assert_eq!(tags.get("og:title").unwrap(), "Hello World");
    }

    #[test]
    fn parse_og_tags_reversed_order() {
        let html = r#"<meta content="Reversed" property="og:description">"#;
        let tags = parse_og_tags(html);
        assert_eq!(tags.get("og:description").unwrap(), "Reversed");
    }

    #[test]
    fn parse_html_title_basic() {
        assert_eq!(
            parse_html_title("<title>My Page</title>"),
            Some("My Page".to_string())
        );
    }

    #[test]
    fn classify_video_by_type() {
        assert_eq!(classify_card_type("video.other", false, true, true), "video");
    }

    #[test]
    fn classify_video_by_tag() {
        assert_eq!(classify_card_type("article", true, true, true), "video");
    }

    #[test]
    fn classify_photo() {
        assert_eq!(classify_card_type("", false, true, false), "photo");
    }

    #[test]
    fn classify_link() {
        assert_eq!(classify_card_type("article", false, true, true), "link");
        assert_eq!(classify_card_type("", false, false, false), "link");
    }
}
