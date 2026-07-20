//! W3C Design Token to CSS custom property compilation.
//!
//! Loads a JSON file containing W3C Design Tokens and compiles them to CSS
//! custom property overrides with light/dark theme support.

use serde_json::Value;
use std::sync::LazyLock;

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
            decls.push_str(var);
            decls.push(':');
            decls.push_str(value);
            decls.push(';');
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

    // Defense in depth: strip </style> sequences even though token values come
    // from operator-controlled JSON. This CSS ends up inside a <style> tag.
    static STYLE_TAG_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)</\s*style").unwrap());
    css = STYLE_TAG_RE.replace_all(&css, "").to_string();

    css
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
    fn strips_style_injection() {
        let json = r##"{"color":{"primary":{"$value":"red</style><script>alert(1)</script>"}}}"##;
        let css = compile_theme_css(json);
        assert!(!css.contains("</style>"));
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
}
