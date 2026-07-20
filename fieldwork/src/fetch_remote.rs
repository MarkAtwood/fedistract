//! Remote object and actor fetching for ActivityPub federation.
//!
//! Provides SSRF-safe, optionally HTTP-Signature-signed HTTP fetching
//! for actor documents, WebFinger, nodeinfo, and public objects.
//!
//! # Feature gate
//!
//! This module requires the `fetch` feature:
//!
//! ```toml
//! fieldwork = { version = "0.1", features = ["fetch"] }
//! ```

use std::sync::Arc;
use std::time::Duration;

/// Configuration for remote fetch operations.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    /// User-Agent header value.
    pub user_agent: String,
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum response body size in bytes.
    pub max_body_size: usize,
    /// Accept header value for ActivityPub requests.
    pub accept: String,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            user_agent: String::new(),
            timeout: Duration::from_secs(10),
            max_body_size: 65536,
            accept: "application/activity+json".to_string(),
        }
    }
}

/// Errors that can occur during remote fetching.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// URL failed SSRF validation (private/internal host or non-HTTPS).
    #[error("SSRF rejected: {0}")]
    Ssrf(#[from] ssrf_guard::SsrfError),

    /// HTTP request failed (network error, DNS failure, TLS error).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Remote server returned a non-success status code.
    #[error("HTTP {0}")]
    Status(reqwest::StatusCode),

    /// Request timed out.
    #[error("request timed out")]
    Timeout,

    /// Response body exceeded the configured size limit.
    #[error("response body exceeds {limit} bytes")]
    BodyTooLarge { limit: usize },

    /// Response body is not valid JSON.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP signature generation failed.
    #[error("signature error: {0}")]
    Signature(#[from] cavage_httpsig::SignError),

    /// URL could not be parsed.
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

/// Build a reqwest client with SSRF-safe DNS resolver, timeout, and user-agent.
fn build_client(config: &FetchConfig) -> Result<reqwest::Client, FetchError> {
    let mut builder = reqwest::Client::builder()
        .timeout(config.timeout)
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(ssrf_guard::SsrfSafeResolver));
    if !config.user_agent.is_empty() {
        builder = builder.user_agent(&config.user_agent);
    }
    Ok(builder.build()?)
}

/// Fetch a remote ActivityPub actor document with HTTP Signature authentication.
///
/// Validates the URL against SSRF, signs the GET request using draft-cavage
/// HTTP Signatures, enforces the body size limit, and parses the response as JSON.
pub async fn fetch_actor(
    url: &str,
    config: &FetchConfig,
    key_id: &str,
    private_key_pem: &str,
) -> Result<serde_json::Value, FetchError> {
    let parsed_url: url::Url = url.parse()?;
    ssrf_guard::validate_url(&parsed_url)?;

    let client = build_client(config)?;
    let sig_headers = cavage_httpsig::sign_get(private_key_pem, key_id, &parsed_url)?;

    // Convert http::HeaderMap to reqwest::header::HeaderMap.
    // Both are re-exports of the same http crate type when versions align,
    // but we go through the safe conversion path.
    let mut req = client
        .get(parsed_url.as_str())
        .header("Accept", &config.accept);
    for (name, value) in sig_headers.iter() {
        req = req.header(name.as_str(), value.as_bytes());
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            FetchError::Timeout
        } else {
            FetchError::Http(e)
        }
    })?;

    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Status(status));
    }

    let body = read_body_limited(resp, config.max_body_size).await?;
    let doc: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(doc)
}

/// Fetch a remote object without HTTP Signature authentication.
///
/// Suitable for WebFinger, nodeinfo, and public objects that do not
/// require authentication. Validates the URL against SSRF, enforces
/// the body size limit, and parses the response as JSON.
pub async fn fetch_object(
    url: &str,
    config: &FetchConfig,
) -> Result<serde_json::Value, FetchError> {
    let parsed_url: url::Url = url.parse()?;
    ssrf_guard::validate_url(&parsed_url)?;

    let client = build_client(config)?;

    let resp = client
        .get(parsed_url.as_str())
        .header("Accept", &config.accept)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                FetchError::Timeout
            } else {
                FetchError::Http(e)
            }
        })?;

    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Status(status));
    }

    let body = read_body_limited(resp, config.max_body_size).await?;
    let doc: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(doc)
}

/// Read a response body up to `max_bytes`, returning an error if exceeded.
///
/// Checks Content-Length first as a fast path, then enforces the limit
/// on the actual bytes read.
async fn read_body_limited(
    resp: reqwest::Response,
    max_bytes: usize,
) -> Result<bytes::Bytes, FetchError> {
    if let Some(len) = resp.content_length() {
        if len as usize > max_bytes {
            return Err(FetchError::BodyTooLarge { limit: max_bytes });
        }
    }
    let body = resp.bytes().await?;
    if body.len() > max_bytes {
        return Err(FetchError::BodyTooLarge { limit: max_bytes });
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_config_defaults() {
        let config = FetchConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.max_body_size, 65536);
        assert_eq!(config.accept, "application/activity+json");
        assert!(config.user_agent.is_empty());
    }

    #[test]
    fn fetch_config_custom() {
        let config = FetchConfig {
            user_agent: "TestBot/1.0".into(),
            timeout: Duration::from_secs(30),
            max_body_size: 1024,
            accept: "application/json".into(),
        };
        assert_eq!(config.user_agent, "TestBot/1.0");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_body_size, 1024);
        assert_eq!(config.accept, "application/json");
    }

    #[test]
    fn fetch_error_ssrf_display() {
        let err = FetchError::Ssrf(ssrf_guard::SsrfError::PrivateHost("127.0.0.1".into()));
        assert!(err.to_string().contains("127.0.0.1"));
    }

    #[test]
    fn fetch_error_body_too_large_display() {
        let err = FetchError::BodyTooLarge { limit: 65536 };
        assert!(err.to_string().contains("65536"));
    }

    #[test]
    fn fetch_error_timeout_display() {
        let err = FetchError::Timeout;
        assert_eq!(err.to_string(), "request timed out");
    }

    #[test]
    fn fetch_error_status_display() {
        let err = FetchError::Status(reqwest::StatusCode::NOT_FOUND);
        assert!(err.to_string().contains("404"));
    }

    #[test]
    fn fetch_error_json_display() {
        let bad = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = FetchError::Json(bad);
        assert!(err.to_string().contains("JSON parse error"));
    }

    #[test]
    fn fetch_error_invalid_url() {
        let err = FetchError::InvalidUrl("not a url".parse::<url::Url>().unwrap_err());
        assert!(err.to_string().contains("invalid URL"));
    }

    #[test]
    fn fetch_error_variants_are_debug() {
        // Ensure all variants implement Debug (compile-time check exercised at runtime).
        let err = FetchError::Timeout;
        let _ = format!("{err:?}");
        let err = FetchError::BodyTooLarge { limit: 100 };
        let _ = format!("{err:?}");
    }
}
