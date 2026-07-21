//! Shared utility functions used across fediverse API handlers.

/// Current Unix timestamp in seconds.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Current time as ISO 8601 string (requires chrono at call site).
/// Returns seconds-precision UTC: `2026-01-02T15:04:05.000000Z`
pub fn now_iso() -> String {
    epoch_to_iso(now_secs())
}

/// Convert Unix epoch seconds to ISO 8601 string.
/// Uses Howard Hinnant's civil_from_days algorithm.
pub fn epoch_to_iso(epoch: i64) -> String {
    const SECS_PER_DAY: i64 = 86400;

    // Floor-divide to get day count and time-of-day
    let mut days = epoch.div_euclid(SECS_PER_DAY);
    let day_secs = epoch.rem_euclid(SECS_PER_DAY);

    // Shift epoch from 1970-01-01 to 0000-03-01 (civil_from_days convention)
    days += 719_468;

    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let h = day_secs / 3600;
    let min = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.000000Z")
}

/// Convert a title string to a URL-safe slug.
///
/// Lowercases, replaces non-alphanumeric chars with hyphens, collapses
/// consecutive hyphens, and trims leading/trailing hyphens.
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

/// Render markdown-ish text to basic HTML.
///
/// Uses ammonia for sanitization. This is intentionally simple — full
/// pulldown-cmark rendering is available via the `sanitize` feature.
pub fn render_markdown_simple(md: &str) -> String {
    // ponytail: paragraph-splitting is good enough for WriteFreely API.
    // Ceiling: use pulldown-cmark when we need headings, code blocks, etc.
    let cleaned = ammonia::clean(md);
    format!(
        "<p>{}</p>",
        cleaned
            .replace("\n\n", "</p><p>")
            .replace('\n', "<br>")
    )
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
        // After 2024-01-01
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
        assert_eq!(s.len(), 27); // YYYY-MM-DDTHH:MM:SS.000000Z
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
    fn render_markdown_simple_basic() {
        let html = render_markdown_simple("Hello **world**");
        assert!(html.contains("<p>"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn render_markdown_simple_paragraphs() {
        let html = render_markdown_simple("First\n\nSecond");
        assert!(html.contains("</p><p>"));
    }
}
