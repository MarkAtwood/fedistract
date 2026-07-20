//! UCAN (User Controlled Authorization Networks) capability delegation tokens
//! for ActivityPub actors identified by DIDs.
//!
//! Implements JWT-like tokens signed with Ed25519, following the UCAN 0.10.0 spec.
//!
//! # Example
//!
//! ```
//! use fedi_ucan::{UcanToken, CapabilityGrant, Capability, issue_ucan, verify_ucan_at, has_capability};
//!
//! let private_key = [42u8; 32];
//! let public_key = fedi_ucan::public_key_from_private(&private_key);
//!
//! let token = UcanToken {
//!     issuer: "did:key:zIssuer".into(),
//!     audience: "did:key:zAudience".into(),
//!     capabilities: vec![CapabilityGrant {
//!         resource: "https://example.com/posts/*".into(),
//!         ability: Capability::Create,
//!     }],
//!     expiration: Some(9_999_999_999),
//!     not_before: None,
//!     nonce: None,
//! };
//!
//! let encoded = issue_ucan(&token, &private_key).unwrap();
//! let decoded = verify_ucan_at(&encoded, &public_key, 1_000_000).unwrap();
//! assert!(has_capability(&decoded, "https://example.com/posts/123", &Capability::Create));
//! ```

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum UcanError {
    #[error("encoding error: {0}")]
    Encoding(String),
    #[error("invalid token format: {0}")]
    InvalidFormat(String),
    #[error("signature verification failed")]
    SignatureVerification,
    #[error("token expired")]
    Expired,
    #[error("token not yet valid")]
    NotYetValid,
    #[error("invalid capability: {0}")]
    InvalidCapability(String),
}

pub type Result<T> = std::result::Result<T, UcanError>;

// ---------------------------------------------------------------------------
// Capability
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Create,
    Delete,
    Update,
    Read,
    Moderate,
    Admin,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Capability::Create => "create",
            Capability::Delete => "delete",
            Capability::Update => "update",
            Capability::Read => "read",
            Capability::Moderate => "moderate",
            Capability::Admin => "admin",
        };
        f.write_str(s)
    }
}

impl FromStr for Capability {
    type Err = UcanError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "create" => Ok(Capability::Create),
            "delete" => Ok(Capability::Delete),
            "update" => Ok(Capability::Update),
            "read" => Ok(Capability::Read),
            "moderate" => Ok(Capability::Moderate),
            "admin" => Ok(Capability::Admin),
            _ => Err(UcanError::InvalidCapability(s.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// CapabilityGrant
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub resource: String,
    pub ability: Capability,
}

// ---------------------------------------------------------------------------
// UcanToken
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UcanToken {
    #[serde(rename = "iss")]
    pub issuer: String,
    #[serde(rename = "aud")]
    pub audience: String,
    #[serde(rename = "cap")]
    pub capabilities: Vec<CapabilityGrant>,
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub expiration: Option<i64>,
    #[serde(rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,
    #[serde(rename = "nnc", skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

// ---------------------------------------------------------------------------
// Key helper
// ---------------------------------------------------------------------------

/// Derive the Ed25519 public key from a 32-byte private key seed.
pub fn public_key_from_private(private_key: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(private_key).verifying_key().to_bytes()
}

// ---------------------------------------------------------------------------
// Issue
// ---------------------------------------------------------------------------

/// Issue a signed UCAN token.
///
/// Builds a JWT-like `base64url(header).base64url(payload).base64url(signature)`
/// string, signed with Ed25519.
pub fn issue_ucan(token: &UcanToken, private_key: &[u8; 32]) -> Result<String> {
    let header = serde_json::json!({
        "alg": "EdDSA",
        "typ": "JWT",
        "ucv": "0.10.0"
    });

    let header_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|e| UcanError::Encoding(e.to_string()))?);

    let payload_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(token).map_err(|e| UcanError::Encoding(e.to_string()))?);

    let signing_input = format!("{header_b64}.{payload_b64}");
    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

    Ok(format!("{signing_input}.{sig_b64}"))
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

/// Verify a UCAN token string using the current system time.
///
/// Checks Ed25519 signature and temporal validity (expiration / not-before).
pub fn verify_ucan(token_str: &str, public_key: &[u8; 32]) -> Result<UcanToken> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64;
    verify_ucan_at(token_str, public_key, now)
}

/// Verify a UCAN token string at an explicit timestamp (for deterministic testing).
pub fn verify_ucan_at(token_str: &str, public_key: &[u8; 32], now: i64) -> Result<UcanToken> {
    let parts: Vec<&str> = token_str.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err(UcanError::InvalidFormat(
            "expected 3 dot-separated segments".into(),
        ));
    }

    let (header_b64, payload_b64, sig_b64) = (parts[0], parts[1], parts[2]);

    // Decode and sanity-check header
    let header_bytes = URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|e| UcanError::InvalidFormat(format!("header base64: {e}")))?;
    let header: serde_json::Value = serde_json::from_slice(&header_bytes)
        .map_err(|e| UcanError::InvalidFormat(format!("header JSON: {e}")))?;
    if header["alg"] != "EdDSA" {
        return Err(UcanError::InvalidFormat("unsupported alg".into()));
    }

    // Verify signature
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|e| UcanError::InvalidFormat(format!("signature base64: {e}")))?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| UcanError::InvalidFormat("invalid signature length".into()))?;
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| UcanError::InvalidFormat("invalid public key".into()))?;

    let signing_input = format!("{header_b64}.{payload_b64}");
    verifying_key
        .verify(signing_input.as_bytes(), &signature)
        .map_err(|_| UcanError::SignatureVerification)?;

    // Decode payload
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| UcanError::InvalidFormat(format!("payload base64: {e}")))?;
    let token: UcanToken = serde_json::from_slice(&payload_bytes)
        .map_err(|e| UcanError::InvalidFormat(format!("payload JSON: {e}")))?;

    // Temporal checks
    if let Some(exp) = token.expiration {
        if now >= exp {
            return Err(UcanError::Expired);
        }
    }
    if let Some(nbf) = token.not_before {
        if now < nbf {
            return Err(UcanError::NotYetValid);
        }
    }

    Ok(token)
}

// ---------------------------------------------------------------------------
// Capability checking
// ---------------------------------------------------------------------------

/// Check whether a token grants the given ability on a resource.
///
/// Uses prefix matching: a grant on `"posts/*"` matches `"posts/123"`.
/// A trailing `*` in the grant resource acts as a wildcard for any suffix.
pub fn has_capability(token: &UcanToken, resource: &str, ability: &Capability) -> bool {
    token.capabilities.iter().any(|grant| {
        if &grant.ability != ability {
            return false;
        }
        if grant.resource == resource {
            return true;
        }
        // Wildcard: "https://example.com/posts/*" matches "https://example.com/posts/123"
        if let Some(prefix) = grant.resource.strip_suffix('*') {
            resource.starts_with(prefix)
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> ([u8; 32], [u8; 32]) {
        let private = [42u8; 32];
        let public = public_key_from_private(&private);
        (private, public)
    }

    fn sample_token() -> UcanToken {
        UcanToken {
            issuer: "did:key:zIssuer".into(),
            audience: "did:key:zAudience".into(),
            capabilities: vec![CapabilityGrant {
                resource: "https://example.com/users/alice/posts/*".into(),
                ability: Capability::Create,
            }],
            expiration: Some(9_999_999_999),
            not_before: None,
            nonce: None,
        }
    }

    #[test]
    fn issue_and_verify_round_trip() {
        let (private, public) = test_keypair();
        let token = sample_token();
        let encoded = issue_ucan(&token, &private).unwrap();
        let decoded = verify_ucan_at(&encoded, &public, 1_000_000).unwrap();
        assert_eq!(decoded.issuer, token.issuer);
        assert_eq!(decoded.audience, token.audience);
        assert_eq!(decoded.capabilities, token.capabilities);
        assert_eq!(decoded.expiration, token.expiration);
    }

    #[test]
    fn expired_token_rejected() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            expiration: Some(1_000),
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let err = verify_ucan_at(&encoded, &public, 2_000).unwrap_err();
        assert!(matches!(err, UcanError::Expired));
    }

    #[test]
    fn not_before_in_future_rejected() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            not_before: Some(5_000),
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let err = verify_ucan_at(&encoded, &public, 1_000).unwrap_err();
        assert!(matches!(err, UcanError::NotYetValid));
    }

    #[test]
    fn not_before_at_boundary_accepted() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            not_before: Some(5_000),
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let decoded = verify_ucan_at(&encoded, &public, 5_000).unwrap();
        assert_eq!(decoded.issuer, token.issuer);
    }

    #[test]
    fn capability_exact_match() {
        let token = sample_token();
        assert!(has_capability(
            &token,
            "https://example.com/users/alice/posts/*",
            &Capability::Create,
        ));
    }

    #[test]
    fn capability_wildcard_match() {
        let token = sample_token();
        assert!(has_capability(
            &token,
            "https://example.com/users/alice/posts/123",
            &Capability::Create,
        ));
        assert!(has_capability(
            &token,
            "https://example.com/users/alice/posts/456/comments",
            &Capability::Create,
        ));
    }

    #[test]
    fn capability_no_match_wrong_ability() {
        let token = sample_token();
        assert!(!has_capability(
            &token,
            "https://example.com/users/alice/posts/123",
            &Capability::Delete,
        ));
    }

    #[test]
    fn capability_no_match_wrong_resource() {
        let token = sample_token();
        assert!(!has_capability(
            &token,
            "https://example.com/users/bob/posts/123",
            &Capability::Create,
        ));
    }

    #[test]
    fn wrong_key_verification_fails() {
        let (private, _) = test_keypair();
        let wrong_public = public_key_from_private(&[99u8; 32]);
        let token = sample_token();
        let encoded = issue_ucan(&token, &private).unwrap();
        let err = verify_ucan_at(&encoded, &wrong_public, 1_000_000).unwrap_err();
        assert!(matches!(err, UcanError::SignatureVerification));
    }

    #[test]
    fn all_capability_types_serialize_deserialize() {
        let caps = vec![
            Capability::Create,
            Capability::Delete,
            Capability::Update,
            Capability::Read,
            Capability::Moderate,
            Capability::Admin,
        ];
        for cap in &caps {
            let json = serde_json::to_string(cap).unwrap();
            let roundtripped: Capability = serde_json::from_str(&json).unwrap();
            assert_eq!(&roundtripped, cap);
        }
    }

    #[test]
    fn all_capability_types_display_fromstr() {
        let caps = vec![
            ("create", Capability::Create),
            ("delete", Capability::Delete),
            ("update", Capability::Update),
            ("read", Capability::Read),
            ("moderate", Capability::Moderate),
            ("admin", Capability::Admin),
        ];
        for (s, cap) in &caps {
            assert_eq!(&cap.to_string(), s);
            assert_eq!(&Capability::from_str(s).unwrap(), cap);
        }
    }

    #[test]
    fn invalid_capability_fromstr() {
        assert!(Capability::from_str("fly").is_err());
    }

    #[test]
    fn token_without_expiration_accepted() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            expiration: None,
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let decoded = verify_ucan_at(&encoded, &public, 999_999_999_999).unwrap();
        assert_eq!(decoded.expiration, None);
    }

    #[test]
    fn token_with_nonce() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            nonce: Some("abc123".into()),
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let decoded = verify_ucan_at(&encoded, &public, 1_000_000).unwrap();
        assert_eq!(decoded.nonce.as_deref(), Some("abc123"));
    }

    #[test]
    fn malformed_token_rejected() {
        let public = [0u8; 32];
        assert!(verify_ucan_at("not.a.token", &public, 0).is_err());
        assert!(verify_ucan_at("only-one-segment", &public, 0).is_err());
    }

    #[test]
    fn multiple_capabilities() {
        let (private, public) = test_keypair();
        let token = UcanToken {
            capabilities: vec![
                CapabilityGrant {
                    resource: "https://example.com/posts/*".into(),
                    ability: Capability::Create,
                },
                CapabilityGrant {
                    resource: "https://example.com/posts/*".into(),
                    ability: Capability::Delete,
                },
                CapabilityGrant {
                    resource: "https://example.com/admin".into(),
                    ability: Capability::Admin,
                },
            ],
            ..sample_token()
        };
        let encoded = issue_ucan(&token, &private).unwrap();
        let decoded = verify_ucan_at(&encoded, &public, 1_000_000).unwrap();
        assert!(has_capability(
            &decoded,
            "https://example.com/posts/1",
            &Capability::Create,
        ));
        assert!(has_capability(
            &decoded,
            "https://example.com/posts/1",
            &Capability::Delete,
        ));
        assert!(has_capability(
            &decoded,
            "https://example.com/admin",
            &Capability::Admin,
        ));
        assert!(!has_capability(
            &decoded,
            "https://example.com/admin/sub",
            &Capability::Admin,
        ));
    }
}
