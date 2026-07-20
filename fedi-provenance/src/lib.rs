//! Lightweight content provenance for media — cryptographic proof of who
//! posted what and when.
//!
//! C2PA-lite for the fediverse: attach a signed [`ProvenanceClaim`] to any
//! ActivityPub media attachment to prove origin, timestamp, and assertion
//! type (original, repost, edited, AI-generated).

use std::fmt;
use std::str::FromStr;

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ProvenanceError {
    #[error("invalid assertion: {0}")]
    InvalidAssertion(String),
    #[error("signature verification failed")]
    SignatureVerification,
    #[error("invalid proof encoding: {0}")]
    InvalidProof(String),
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, ProvenanceError>;

/// Content assertion type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assertion {
    Original,
    Repost,
    Edited,
    AiGenerated,
}

impl fmt::Display for Assertion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Assertion::Original => write!(f, "original"),
            Assertion::Repost => write!(f, "repost"),
            Assertion::Edited => write!(f, "edited"),
            Assertion::AiGenerated => write!(f, "ai_generated"),
        }
    }
}

impl FromStr for Assertion {
    type Err = ProvenanceError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "original" => Ok(Assertion::Original),
            "repost" => Ok(Assertion::Repost),
            "edited" => Ok(Assertion::Edited),
            "ai_generated" => Ok(Assertion::AiGenerated),
            _ => Err(ProvenanceError::InvalidAssertion(s.to_string())),
        }
    }
}

impl Serialize for Assertion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Assertion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Assertion::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A signed content provenance claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceClaim {
    #[serde(rename = "type")]
    pub type_: String,
    pub creator: String,
    pub created: String,
    pub integrity: String,
    pub assertion: Assertion,
    pub proof: String,
}

/// Create a signed provenance claim for media content.
///
/// The `integrity_hash` should be an SRI hash of the media bytes
/// (e.g. `"sha256-base64..."`). The claim is signed with the provided
/// Ed25519 private key, and the `creator` field is set to the
/// corresponding `did:key`.
pub fn create_provenance(
    integrity_hash: &str,
    assertion: Assertion,
    private_key: &[u8; 32],
) -> Result<ProvenanceClaim> {
    let public_key = fedi_did::ed25519_public_from_private(private_key);
    let creator = fedi_did::ed25519_to_did_key(&public_key);
    let created = iso8601_now();

    let mut claim = ProvenanceClaim {
        type_: "ContentProvenance".to_string(),
        creator,
        created,
        integrity: integrity_hash.to_string(),
        assertion,
        proof: String::new(),
    };

    let canonical = canonical_without_proof(&claim)?;
    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(canonical.as_bytes());
    claim.proof = multibase::encode(multibase::Base::Base58Btc, signature.to_bytes());

    Ok(claim)
}

/// Verify a provenance claim against the given public key.
///
/// Returns `Ok(true)` if the signature is valid, `Ok(false)` if the
/// signature does not match, or `Err` if the proof or key is malformed.
pub fn verify_provenance(claim: &ProvenanceClaim, public_key: &[u8; 32]) -> Result<bool> {
    let canonical = canonical_without_proof(claim)?;

    let (_, sig_bytes) = multibase::decode(&claim.proof)
        .map_err(|e| ProvenanceError::InvalidProof(e.to_string()))?;

    let signature = ed25519_dalek::Signature::from_slice(&sig_bytes)
        .map_err(|e| ProvenanceError::InvalidProof(e.to_string()))?;

    let verifying_key =
        VerifyingKey::from_bytes(public_key).map_err(|_| ProvenanceError::InvalidPublicKey)?;

    Ok(verifying_key.verify(canonical.as_bytes(), &signature).is_ok())
}

/// Attach a provenance claim to a media attachment JSON object.
///
/// Adds a `"provenance"` field containing the serialized claim.
pub fn attach_provenance(media_json: &mut serde_json::Value, claim: &ProvenanceClaim) {
    let claim_value = serde_json::to_value(claim).expect("ProvenanceClaim always serializes");
    media_json["provenance"] = claim_value;
}

/// Extract a provenance claim from a media attachment JSON object.
///
/// Returns `None` if no `"provenance"` field is present or if it cannot
/// be deserialized.
pub fn extract_provenance(media_json: &serde_json::Value) -> Option<ProvenanceClaim> {
    media_json.get("provenance").and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Serialize the claim without the proof field and canonicalize it.
fn canonical_without_proof(claim: &ProvenanceClaim) -> Result<String> {
    let mut value = serde_json::to_value(claim)
        .map_err(|e| ProvenanceError::Serialization(e.to_string()))?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("proof");
    }
    Ok(fedi_did::canonicalize(&value))
}

/// Format the current UTC time as ISO 8601 (e.g. `"2024-01-15T12:30:45Z"`).
fn iso8601_now() -> String {
    let now = std::time::SystemTime::now();
    let dur = now
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch");
    let secs = dur.as_secs();

    // ponytail: manual UTC formatting avoids chrono dependency.
    // Ceiling: no sub-second precision, no timezone offsets.
    const SECS_PER_DAY: u64 = 86400;
    let days = secs / SECS_PER_DAY;
    let day_secs = secs % SECS_PER_DAY;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Days since 1970-01-01
    let (year, month, day) = civil_from_days(days as i64);

    format!(
        "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
    )
}

/// Convert days since 1970-01-01 to (year, month, day).
///
/// Algorithm from Howard Hinnant's `chrono`-compatible date library.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> ([u8; 32], [u8; 32]) {
        let (private_key, public_key) = fedi_did::generate_recovery_keypair();
        (*private_key, public_key)
    }

    #[test]
    fn sign_verify_round_trip() {
        let (private_key, public_key) = test_keypair();
        let claim =
            create_provenance("sha256-abc123", Assertion::Original, &private_key).unwrap();

        assert_eq!(claim.type_, "ContentProvenance");
        assert!(claim.creator.starts_with("did:key:z6Mk"));
        assert!(!claim.proof.is_empty());

        let valid = verify_provenance(&claim, &public_key).unwrap();
        assert!(valid);
    }

    #[test]
    fn tampered_claim_fails() {
        let (private_key, public_key) = test_keypair();
        let mut claim =
            create_provenance("sha256-abc123", Assertion::Original, &private_key).unwrap();

        claim.integrity = "sha256-TAMPERED".to_string();

        let valid = verify_provenance(&claim, &public_key).unwrap();
        assert!(!valid);
    }

    #[test]
    fn verify_with_wrong_key_fails() {
        let (private_key, _) = test_keypair();
        let (_, wrong_public_key) = test_keypair();

        let claim =
            create_provenance("sha256-abc123", Assertion::Original, &private_key).unwrap();

        let valid = verify_provenance(&claim, &wrong_public_key).unwrap();
        assert!(!valid);
    }

    #[test]
    fn assertion_serialize_all_types() {
        let cases = [
            (Assertion::Original, "\"original\""),
            (Assertion::Repost, "\"repost\""),
            (Assertion::Edited, "\"edited\""),
            (Assertion::AiGenerated, "\"ai_generated\""),
        ];
        for (assertion, expected_json) in &cases {
            let json = serde_json::to_string(assertion).unwrap();
            assert_eq!(&json, expected_json, "serialization mismatch for {assertion}");
            let round_tripped: Assertion = serde_json::from_str(&json).unwrap();
            assert_eq!(&round_tripped, assertion, "round-trip mismatch for {assertion}");
        }
    }

    #[test]
    fn assertion_display_from_str() {
        for assertion in [
            Assertion::Original,
            Assertion::Repost,
            Assertion::Edited,
            Assertion::AiGenerated,
        ] {
            let s = assertion.to_string();
            let parsed: Assertion = s.parse().unwrap();
            assert_eq!(parsed, assertion);
        }
    }

    #[test]
    fn assertion_from_str_invalid() {
        assert!("bogus".parse::<Assertion>().is_err());
    }

    #[test]
    fn attach_extract_round_trip() {
        let (private_key, _) = test_keypair();
        let claim =
            create_provenance("sha256-media-hash", Assertion::Repost, &private_key).unwrap();

        let mut media = serde_json::json!({
            "type": "Image",
            "mediaType": "image/png",
            "url": "https://example.com/image.png"
        });

        attach_provenance(&mut media, &claim);
        assert!(media.get("provenance").is_some());

        let extracted = extract_provenance(&media).unwrap();
        assert_eq!(extracted, claim);
    }

    #[test]
    fn extract_provenance_missing() {
        let media = serde_json::json!({"type": "Image"});
        assert!(extract_provenance(&media).is_none());
    }

    #[test]
    fn claim_serde_type_field() {
        let (private_key, _) = test_keypair();
        let claim =
            create_provenance("sha256-test", Assertion::Original, &private_key).unwrap();

        let json = serde_json::to_value(&claim).unwrap();
        assert_eq!(json["type"], "ContentProvenance");
        assert!(json.get("type_").is_none());
    }

    #[test]
    fn iso8601_timestamp_format() {
        let ts = iso8601_now();
        assert!(ts.ends_with('Z'), "timestamp must end with Z: {ts}");
        assert_eq!(ts.len(), 20, "ISO 8601 UTC timestamp is 20 chars: {ts}");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }
}
