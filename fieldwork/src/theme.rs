//! W3C Design Token to CSS custom property compilation.
//!
//! Loads a JSON file containing W3C Design Tokens and compiles them to CSS
//! custom property overrides with light/dark theme support. Includes base
//! stylesheet and custom CSS loading with injection defense.

use serde_json::Value;
use std::sync::LazyLock;

/// Base CSS shared by profile and index pages. Uses CSS custom properties
/// so theme tokens and dark mode can override colors without changing rules.
pub const BASE_CSS: &str = "\
:root { --text: #1d1d1f; --muted: #6e6e73; --bg: #fff; --card: #f5f5f7; --border: #d2d2d7; --link: #0066cc; }\
* { box-sizing: border-box; margin: 0; padding: 0; }\
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; \
color: var(--text); background: var(--bg); line-height: 1.6; }\
main { max-width: 640px; margin: 0 auto; padding: 2rem 1.5rem; }\
h1 { font-size: 1.75rem; font-weight: 600; margin-bottom: 0.5rem; }\
a { color: var(--link); text-decoration: none; }\
a:hover { text-decoration: underline; }\
@media (prefers-color-scheme: dark) { \
:root { --text: #f5f5f7; --muted: #98989d; --bg: #1d1d1f; --card: #2c2c2e; --border: #3a3a3c; --link: #2997ff; }\
body { background: var(--bg); color: var(--text); } }";

fn token_to_css_var(name: &str) -> Option<&'static str> {
    match name {
        "primary" => Some("--link"),
        "background" => Some("--bg"),
        "surface" => Some("--card"),
        "text" => Some("--text"),
        "muted" => Some("--muted"),
        "border" => Some("--border"),
        _ => None,
    }
}

/// Reject CSS values that could inject rules or exfiltrate data.
fn is_safe_css_value(s: &str) -> bool {
    s.len() <= 100
        && !s.contains('}')
        && !s.contains(';')
        && !s.contains("url(")
        && !s.contains("expression(")
        && !s.contains("@import")
        && !s.contains("javascript:")
}

fn declarations_from_group(group: &Value) -> String {
    let obj = match group.as_object() {
        Some(o) => o,
        None => return String::new(),
    };
    let mut decls = String::new();
    for (name, token) in obj {
        if let (Some(var), Some(value)) = (
            token_to_css_var(name),
            token.get("$value").and_then(|v| v.as_str()),
        ) {
            if is_safe_css_value(value) {
                decls.push_str(var);
                decls.push(':');
                decls.push_str(value);
                decls.push(';');
            } else {
                tracing::warn!(token = name, "rejecting unsafe CSS token value");
            }
        }
    }
    decls
}

/// Load a W3C Design Tokens JSON file and compile it to CSS custom property overrides.
///
/// - `color` group produces `:root { ... }`
/// - `color-dark` group produces `@media (prefers-color-scheme: dark) { :root { ... } }`
///
/// Returns an empty string if the path is empty or on any error.
pub fn load_theme_css(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path, "failed to load theme tokens: {e}");
            return String::new();
        }
    };
    compile_theme_css(&contents)
}

/// Compile a W3C Design Tokens JSON string to CSS custom property overrides.
pub fn compile_theme_css(json: &str) -> String {
    let doc: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse theme tokens JSON: {e}");
            return String::new();
        }
    };

    let mut css = String::new();

    if let Some(light) = doc.get("color") {
        let decls = declarations_from_group(light);
        if !decls.is_empty() {
            css.push_str(":root{");
            css.push_str(&decls);
            css.push('}');
        }
    }

    if let Some(dark) = doc.get("color-dark") {
        let decls = declarations_from_group(dark);
        if !decls.is_empty() {
            css.push_str("@media(prefers-color-scheme:dark){:root{");
            css.push_str(&decls);
            css.push_str("}}");
        }
    }

    css
}

/// Load operator-supplied CSS and combine with theme token overrides.
///
/// Strips `</style>` sequences from the combined output to prevent
/// breakout from the `<style>` tag. Returns the combined CSS string
/// ready for embedding in `<style>{result}</style>`.
pub fn load_extra_css(theme_tokens_path: &str, custom_css_path: &str) -> String {
    let theme_css = load_theme_css(theme_tokens_path);
    let custom_css = if custom_css_path.is_empty() {
        String::new()
    } else {
        match std::fs::read_to_string(custom_css_path) {
            Ok(css) => css,
            Err(e) => {
                tracing::warn!(path = custom_css_path, "failed to read custom CSS: {e}");
                String::new()
            }
        }
    };

    sanitize_css(&format!("{theme_css}{custom_css}"))
}

/// Strip `</style>` sequences from CSS to prevent breakout from `<style>` tags.
pub fn sanitize_css(css: &str) -> String {
    static STYLE_TAG_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::RegexBuilder::new(r"</style\s*>")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    STYLE_TAG_RE.replace_all(css, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_light_theme() {
        let json = r##"{"color":{"primary":{"$value":"#ff0000"},"text":{"$value":"#000"}}}"##;
        let css = compile_theme_css(json);
        assert!(css.contains(":root{"));
        assert!(css.contains("--link:#ff0000"));
        assert!(css.contains("--text:#000"));
    }

    #[test]
    fn compiles_dark_theme() {
        let json = r##"{"color-dark":{"background":{"$value":"#111"}}}"##;
        let css = compile_theme_css(json);
        assert!(css.contains("prefers-color-scheme:dark"));
        assert!(css.contains("--bg:#111"));
    }

    #[test]
    fn rejects_unsafe_css_values() {
        // url() injection
        let json = r##"{"color":{"primary":{"$value":"url(https://evil.com/exfil)"}}}"##;
        let css = compile_theme_css(json);
        assert!(!css.contains("url("));

        // Semicolon injection
        let json = r##"{"color":{"primary":{"$value":"red; background: url(evil)"}}}"##;
        let css = compile_theme_css(json);
        assert_eq!(css, "");

        // Brace injection
        let json = r##"{"color":{"primary":{"$value":"red} body{display:none"}}}"##;
        let css = compile_theme_css(json);
        assert_eq!(css, "");

        // @import injection
        let json = r##"{"color":{"primary":{"$value":"@import url(evil)"}}}"##;
        let css = compile_theme_css(json);
        assert_eq!(css, "");
    }

    #[test]
    fn rejects_overlong_css_values() {
        let long = "a".repeat(101);
        let json = format!(r##"{{"color":{{"primary":{{"$value":"{long}"}}}}}}"##);
        let css = compile_theme_css(&json);
        assert_eq!(css, "");
    }

    #[test]
    fn strips_style_injection() {
        let css = sanitize_css("color:red</style><script>alert(1)</script>");
        assert!(!css.contains("</style>"), "must strip </style> breakout");
        // <script> after </style> is harmless inside CSS context once breakout is prevented
        assert!(css.starts_with("color:red"));
    }

    #[test]
    fn strips_style_injection_case_insensitive() {
        let css = sanitize_css("color:red</STYLE ><script>alert(1)</script>");
        assert!(!css.contains("</STYLE"));
    }

    #[test]
    fn empty_on_invalid_json() {
        assert_eq!(compile_theme_css("not json"), "");
    }

    #[test]
    fn empty_on_no_groups() {
        assert_eq!(compile_theme_css(r#"{"other":{}}"#), "");
    }

    #[test]
    fn ignores_unknown_tokens() {
        let json = r##"{"color":{"unknown_token":{"$value":"#fff"}}}"##;
        let css = compile_theme_css(json);
        assert_eq!(css, "");
    }

    #[test]
    fn base_css_has_custom_properties() {
        assert!(BASE_CSS.contains("--text:"));
        assert!(BASE_CSS.contains("--bg:"));
        assert!(BASE_CSS.contains("prefers-color-scheme: dark"));
    }
}
