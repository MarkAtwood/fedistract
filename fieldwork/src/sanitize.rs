//! HTML sanitization and Markdown rendering for ActivityPub content.
//!
//! Uses a Mastodon-compatible allowlist: basic formatting tags plus `<a href>`.

use ammonia::Builder;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static SANITIZE_BUILDER: LazyLock<Builder<'static>> = LazyLock::new(|| {
    let tags: HashSet<&'static str> = [
        "p", "br", "a", "span", "em", "strong", "del", "blockquote", "code", "pre", "ul", "ol",
        "li",
    ]
    .into_iter()
    .collect();

    let mut attrs: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    attrs.insert("a", ["href"].into_iter().collect());

    let mut b = Builder::new();
    b.tags(tags).tag_attributes(attrs);
    b
});

/// Sanitize HTML to the Mastodon-compatible allowlist.
pub fn sanitize_html(html: &str) -> String {
    SANITIZE_BUILDER.clean(html).to_string()
}

/// Render Markdown to sanitized HTML.
pub fn markdown_to_html(markdown: &str) -> String {
    let parser = pulldown_cmark::Parser::new(markdown);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    sanitize_html(&html)
}

/// Strip all HTML tags to get plain text.
pub fn html_to_text(html: &str) -> String {
    // ponytail: ammonia with no tags allowed strips everything to text
    Builder::new()
        .tags(HashSet::new())
        .clean(html)
        .to_string()
}

/// Truncate a string at a UTF-8 safe boundary.
pub fn truncate_utf8(s: &mut String, max_len: usize) {
    if s.len() <= max_len {
        return;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

/// Escape a string for safe interpolation in double-quoted HTML attributes.
pub fn escape_html_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tags() {
        assert_eq!(sanitize_html("<script>alert(1)</script>hello"), "hello");
    }

    #[test]
    fn preserves_allowed_tags() {
        let input = "<p><strong>bold</strong> and <em>italic</em></p>";
        assert_eq!(sanitize_html(input), input);
    }

    #[test]
    fn preserves_links() {
        let input = r#"<a href="https://example.com">link</a>"#;
        let output = sanitize_html(input);
        assert!(output.contains("href=\"https://example.com\""));
        assert!(output.contains("link"));
    }

    #[test]
    fn markdown_renders_and_sanitizes() {
        let html = markdown_to_html("**bold** and <script>xss</script>");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn html_to_text_strips_everything() {
        assert_eq!(html_to_text("<p>hello <b>world</b></p>"), "hello world");
    }

    #[test]
    fn truncate_utf8_respects_boundaries() {
        let mut s = "hello \u{1F600} world".to_string(); // emoji is 4 bytes
        truncate_utf8(&mut s, 8); // mid-emoji
        assert!(s.len() <= 8);
        assert!(s.is_char_boundary(s.len()));
    }

    #[test]
    fn truncate_utf8_noop_when_short() {
        let mut s = "short".to_string();
        truncate_utf8(&mut s, 100);
        assert_eq!(s, "short");
    }

    #[test]
    fn escape_html_attr_basics() {
        assert_eq!(escape_html_attr(r#"a"b<c>d&e"#), "a&quot;b&lt;c&gt;d&amp;e");
    }
}
