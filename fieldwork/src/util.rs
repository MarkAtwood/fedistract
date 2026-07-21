//! Shared utility functions used across fediverse API handlers.

/// Current Unix timestamp in seconds.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Current time as ISO 8601 string.
pub fn now_iso() -> String {
    epoch_to_iso(now_secs())
}

/// Convert Unix epoch seconds to ISO 8601 string.
pub fn epoch_to_iso(epoch: i64) -> String {
    chrono::DateTime::from_timestamp(epoch, 0)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

/// Convert a title string to a URL-safe slug.
///
/// Lowercases, replaces non-alphanumeric chars with hyphens, collapses
/// consecutive hyphens, and trims leading/trailing hyphens.
// ponytail: ASCII-only. Handles English titles fine. Ceiling: add `deunicode`
// crate for Unicode folding (é→e, ñ→n) when non-English titles appear.
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Render Markdown to sanitized HTML.
///
/// Uses pulldown-cmark for parsing and ammonia for sanitization.
pub fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(md, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    ammonia::clean(&html_output)
}

/// CSS for Instagram-style photo grid layout.
pub const PHOTO_GRID_CSS: &str = "\
.photo-grid { display: grid; gap: 4px; grid-template-columns: repeat(3, 1fr); }\
.photo-grid a { display: block; position: relative; aspect-ratio: 1; overflow: hidden; }\
.photo-grid img { width: 100%; height: 100%; object-fit: cover; transition: transform 0.2s; }\
.photo-grid a:hover img { transform: scale(1.05); }\
@media (max-width: 640px) { .photo-grid { grid-template-columns: repeat(2, 1fr); } }\
@media (max-width: 400px) { .photo-grid { grid-template-columns: 1fr; } }";

/// Render a photo grid HTML fragment from media items.
///
/// Each item is `(file_url, alt_text, post_url)`.
pub fn render_photo_grid(items: &[(String, String, String)]) -> String {
    if items.is_empty() {
        return "<p class=\"empty\">No photos yet.</p>".into();
    }
    let mut html = String::from("<div class=\"photo-grid\">");
    for (file_url, alt, post_url) in items {
        let alt_escaped = ammonia::clean(alt);
        html.push_str(&format!(
            "<a href=\"{post_url}\"><img src=\"{file_url}\" alt=\"{alt_escaped}\" loading=\"lazy\"></a>"
        ));
    }
    html.push_str("</div>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_secs_is_reasonable() {
        let ts = now_secs();
        assert!(ts > 1_704_067_200);
    }

    #[test]
    fn epoch_to_iso_known_value() {
        assert_eq!(epoch_to_iso(0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(epoch_to_iso(1704067200), "2024-01-01T00:00:00.000000Z");
    }

    #[test]
    fn now_iso_format() {
        let s = now_iso();
        assert!(s.ends_with('Z'));
        assert!(s.contains('T'));
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  My  Post  "), "my-post");
        assert_eq!(slugify("CamelCase123"), "camelcase123");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("foo/bar/baz"), "foo-bar-baz");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn render_markdown_heading() {
        let html = render_markdown("# Hello");
        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn render_markdown_bold_italic() {
        let html = render_markdown("**bold** and *italic*");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn render_markdown_sanitizes_script() {
        let html = render_markdown("<script>alert('xss')</script>");
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn render_markdown_paragraphs() {
        let html = render_markdown("First\n\nSecond");
        assert!(html.contains("<p>First</p>"));
        assert!(html.contains("<p>Second</p>"));
    }

    #[test]
    fn render_markdown_code_block() {
        let html = render_markdown("```\nlet x = 1;\n```");
        assert!(html.contains("<code>"));
    }
}
