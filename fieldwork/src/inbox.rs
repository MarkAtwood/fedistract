//! Inbox verification for inbound ActivityPub POST requests.
//!
//! Provides pre-signature validation (size limits, digest verification) extracted
//! from pentest-hardened patterns in broadside. Designed to run before HTTP
//! signature verification to reject malformed or oversized requests cheaply.
//!
//! # Security properties
//!
//! - **256KB body limit** prevents memory exhaustion from large payloads.
//! - **8KB Signature header limit** bounds allocation before parsing.
//! - **Constant-time digest comparison** prevents timing side-channels.
//! - **Cross-domain inbox host validation** prevents delivery redirection.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sha2::Digest;
use subtle::ConstantTimeEq;

/// Configuration for inbox POST validation.
///
/// Defaults match broadside pentest hardening: 256KB body, 8KB Signature header.
#[derive(Debug, Clone)]
pub struct InboxConfig {
    /// Maximum body size in bytes. Requests exceeding this are rejected before
    /// any parsing or signature verification.
    pub max_body_size: usize,
    /// Maximum `Signature` header size in bytes. Bounds allocation during
    /// header parsing.
    pub max_signature_header_size: usize,
}

impl Default for InboxConfig {
    fn default() -> Self {
        Self {
            max_body_size: 256 * 1024,
            max_signature_header_size: 8192,
        }
    }
}

/// Errors from inbox validation.
#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("request body exceeds {limit} byte limit")]
    BodyTooLarge { limit: usize },
    #[error("Signature header exceeds {limit} byte limit")]
    SignatureHeaderTooLarge { limit: usize },
    #[error("Digest header mismatch")]
    DigestMismatch,
    #[error("missing Digest header")]
    MissingDigest,
    #[error("invalid Digest header: {reason}")]
    InvalidDigest { reason: &'static str },
    #[error("inbox host {inbox_host:?} does not match actor host {actor_host:?}")]
    InboxHostMismatch {
        inbox_host: String,
        actor_host: String,
    },
}

/// Verify the `Digest` header against the request body.
///
/// Supports `SHA-256=<base64>` format (the only algorithm required by ActivityPub
/// implementations). Uses constant-time comparison to prevent timing side-channels.
pub fn verify_digest(body: &[u8], digest_header: &str) -> Result<(), InboxError> {
    let expected_b64 = digest_header
        .strip_prefix("SHA-256=")
        .ok_or(InboxError::InvalidDigest {
            reason: "unsupported algorithm, expected SHA-256=",
        })?;

    let expected = B64.decode(expected_b64).map_err(|_| InboxError::InvalidDigest {
        reason: "invalid base64",
    })?;

    let actual = sha2::Sha256::digest(body);

    // ponytail: subtle::ConstantTimeEq requires equal-length slices. SHA-256 output
    // is always 32 bytes; if the decoded expected value is a different length, the
    // digest is definitionally wrong. Short-circuit is safe here because length
    // leakage reveals nothing about the actual hash.
    if actual.len() != expected.len() {
        return Err(InboxError::DigestMismatch);
    }

    if actual.ct_eq(&expected).unwrap_u8() != 1 {
        return Err(InboxError::DigestMismatch);
    }

    Ok(())
}

/// Validate an inbox POST request before heavier operations (signature verification,
/// actor fetch, JSON parsing).
///
/// Checks:
/// - Body size against `config.max_body_size` (using `Content-Length` if available)
/// - Signature header size against `config.max_signature_header_size`
///
/// Call this before `verify_digest` and before HTTP signature verification.
pub fn validate_inbox_post(
    content_length: Option<u64>,
    signature_header: &str,
    config: &InboxConfig,
) -> Result<(), InboxError> {
    if let Some(len) = content_length {
        if len > config.max_body_size as u64 {
            return Err(InboxError::BodyTooLarge {
                limit: config.max_body_size,
            });
        }
    }

    if signature_header.len() > config.max_signature_header_size {
        return Err(InboxError::SignatureHeaderTooLarge {
            limit: config.max_signature_header_size,
        });
    }

    Ok(())
}

/// Validate that an inbox URI's host matches the actor URI's host.
///
/// Prevents a compromised actor document from redirecting deliveries to an
/// attacker-controlled inbox on a different domain.
pub fn validate_inbox_host(actor_uri: &str, inbox_uri: &str) -> Result<(), InboxError> {
    let actor_host = extract_host(actor_uri);
    let inbox_host = extract_host(inbox_uri);

    match (&actor_host, &inbox_host) {
        (Some(a), Some(i)) if a == i => Ok(()),
        (a, i) => Err(InboxError::InboxHostMismatch {
            actor_host: a.clone().unwrap_or_default(),
            inbox_host: i.clone().unwrap_or_default(),
        }),
    }
}

/// Extract the host from a URI string.
// ponytail: manual parsing avoids pulling in the `url` crate. Only needs
// the host component from an https:// URI. Ceiling: use url::Url if we need
// port-aware or IDN-aware comparison.
fn extract_host(uri: &str) -> Option<String> {
    let after_scheme = uri.strip_prefix("https://").or_else(|| uri.strip_prefix("http://"))?;
    let host_part = after_scheme.split('/').next()?;
    let host = host_part.split(':').next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent oracle: echo -n '{"type":"Create"}' | openssl dgst -sha256 -binary | openssl base64
    // = JeE18werLvQnEoHViKDam+ZK1D8E27TBC2kIISI7pIY=
    const TEST_BODY: &[u8] = b"{\"type\":\"Create\"}";
    const TEST_DIGEST_VALID: &str = "SHA-256=JeE18werLvQnEoHViKDam+ZK1D8E27TBC2kIISI7pIY=";

    #[test]
    fn verify_digest_valid() {
        assert!(verify_digest(TEST_BODY, TEST_DIGEST_VALID).is_ok());
    }

    #[test]
    fn verify_digest_mismatch() {
        let wrong = "SHA-256=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        assert!(matches!(
            verify_digest(TEST_BODY, wrong),
            Err(InboxError::DigestMismatch)
        ));
    }

    #[test]
    fn verify_digest_wrong_length() {
        // 16 bytes instead of 32 -- length check short-circuits
        let short = format!("SHA-256={}", B64.encode([0u8; 16]));
        assert!(matches!(
            verify_digest(TEST_BODY, &short),
            Err(InboxError::DigestMismatch)
        ));
    }

    #[test]
    fn verify_digest_unsupported_algorithm() {
        let md5_digest = "MD5=rL0Y20zC+Fzt72VPzMSk2A==";
        assert!(matches!(
            verify_digest(TEST_BODY, md5_digest),
            Err(InboxError::InvalidDigest { .. })
        ));
    }

    #[test]
    fn verify_digest_invalid_base64() {
        let bad = "SHA-256=not-valid-base64!!!";
        assert!(matches!(
            verify_digest(TEST_BODY, bad),
            Err(InboxError::InvalidDigest { .. })
        ));
    }

    #[test]
    fn verify_digest_empty_body() {
        // Independent oracle: echo -n '' | openssl dgst -sha256 -binary | openssl base64
        // = 47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=
        let digest = "SHA-256=47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        assert!(verify_digest(b"", digest).is_ok());
    }

    #[test]
    fn validate_inbox_post_body_too_large() {
        let config = InboxConfig::default();
        let result = validate_inbox_post(Some(300_000), "sig", &config);
        assert!(matches!(result, Err(InboxError::BodyTooLarge { .. })));
    }

    #[test]
    fn validate_inbox_post_body_at_limit() {
        let config = InboxConfig::default();
        let result = validate_inbox_post(Some(256 * 1024), "sig", &config);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_inbox_post_no_content_length() {
        let config = InboxConfig::default();
        let result = validate_inbox_post(None, "sig", &config);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_inbox_post_signature_too_large() {
        let config = InboxConfig::default();
        let big_sig = "A".repeat(9000);
        let result = validate_inbox_post(Some(100), &big_sig, &config);
        assert!(matches!(
            result,
            Err(InboxError::SignatureHeaderTooLarge { .. })
        ));
    }

    #[test]
    fn validate_inbox_post_signature_at_limit() {
        let config = InboxConfig::default();
        let sig = "A".repeat(8192);
        let result = validate_inbox_post(Some(100), &sig, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_inbox_host_same_domain() {
        assert!(validate_inbox_host(
            "https://example.com/users/alice",
            "https://example.com/users/alice/inbox"
        )
        .is_ok());
    }

    #[test]
    fn validate_inbox_host_cross_domain() {
        let result = validate_inbox_host(
            "https://good.example/users/alice",
            "https://evil.example/inbox",
        );
        assert!(matches!(result, Err(InboxError::InboxHostMismatch { .. })));
    }

    #[test]
    fn validate_inbox_host_case_insensitive() {
        assert!(validate_inbox_host(
            "https://EXAMPLE.COM/users/alice",
            "https://example.com/inbox"
        )
        .is_ok());
    }

    #[test]
    fn validate_inbox_host_with_port() {
        // Port is stripped -- only host compared
        assert!(validate_inbox_host(
            "https://example.com:443/users/alice",
            "https://example.com/inbox"
        )
        .is_ok());
    }

    #[test]
    fn validate_inbox_host_invalid_uri() {
        let result = validate_inbox_host("not-a-uri", "https://example.com/inbox");
        assert!(matches!(result, Err(InboxError::InboxHostMismatch { .. })));
    }

    #[test]
    fn constant_time_comparison_same_result_regardless_of_position() {
        // Verify that both early-differing and late-differing digests return the same
        // error variant. This is a structural test -- actual timing verification requires
        // statistical measurement which is not practical in unit tests.
        let early_diff = format!("SHA-256={}", B64.encode([0xFF; 32]));
        let mut late_bytes = sha2::Sha256::digest(TEST_BODY).to_vec();
        // Flip last byte only
        let last = late_bytes.len() - 1;
        late_bytes[last] ^= 0x01;
        let late_diff = format!("SHA-256={}", B64.encode(&late_bytes));

        let r1 = verify_digest(TEST_BODY, &early_diff);
        let r2 = verify_digest(TEST_BODY, &late_diff);
        assert!(matches!(r1, Err(InboxError::DigestMismatch)));
        assert!(matches!(r2, Err(InboxError::DigestMismatch)));
    }
}
