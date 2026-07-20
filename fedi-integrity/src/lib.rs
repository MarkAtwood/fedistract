//! Linked Data Integrity proofs for ActivityPub objects.
//!
//! Implements FEP-8b32 (Object Integrity Proofs) using Ed25519 signatures
//! over JCS-canonicalized JSON. Backwards-compatible: adds a `proof` field
//! that existing AP servers ignore.
//!
//! # Usage
//!
//! ```
//! use fedi_integrity::{sign_object, verify_object};
//! use fedi_did::{generate_recovery_keypair, ed25519_to_did_key};
//!
//! let (private_key, public_key) = generate_recovery_keypair();
//! let did_key = ed25519_to_did_key(&public_key);
//!
//! let mut note = serde_json::json!({
//!     "type": "Note",
//!     "content": "Hello, fediverse!"
//! });
//!
//! sign_object(&mut note, &private_key, &did_key).unwrap();
//! assert!(note.get("proof").is_some());
//!
//! let status = verify_object(&note, &public_key).unwrap();
//! assert!(status.valid);
//! ```

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use fedi_did::canonicalize;

#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error("missing proof field")]
    MissingProof,
    #[error("invalid proof structure: {0}")]
    InvalidProof(String),
    #[error("invalid signature encoding: {0}")]
    InvalidSignatureEncoding(String),
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("invalid key: {0}")]
    InvalidKey(String),
}

/// Result of verifying an integrity proof.
#[derive(Debug, Clone)]
pub struct ProofStatus {
    pub valid: bool,
    pub signer_did: String,
    pub signed_at: String,
}

/// Sign an ActivityPub object by adding a `proof` field.
///
/// The proof contains an Ed25519 signature over the JCS-canonical form
/// of the object (with any existing `proof` field removed before signing).
///
/// Implements the Ed25519Signature2020 proof type per FEP-8b32.
pub fn sign_object(
    object: &mut serde_json::Value,
    private_key: &[u8; 32],
    did_key: &str,
) -> Result<(), IntegrityError> {
    // Remove any existing proof before computing signature
    strip_proof(object);

    let canonical = canonicalize(object);
    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(canonical.as_bytes());

    let proof_value = multibase::encode(
        multibase::Base::Base58Btc,
        signature.to_bytes(),
    );

    let now = unix_now_iso8601();

    let proof = serde_json::json!({
        "type": "Ed25519Signature2020",
        "verificationMethod": did_key,
        "created": now,
        "proofPurpose": "assertionMethod",
        "proofValue": proof_value
    });

    object.as_object_mut()
        .ok_or_else(|| IntegrityError::InvalidProof("object is not a JSON object".into()))?
        .insert("proof".into(), proof);

    Ok(())
}

/// Verify an integrity proof on an ActivityPub object.
///
/// Extracts the `proof` field, reconstructs the canonical form (without proof),
/// and verifies the Ed25519 signature.
pub fn verify_object(
    object: &serde_json::Value,
    public_key: &[u8; 32],
) -> Result<ProofStatus, IntegrityError> {
    let proof = object.get("proof")
        .ok_or(IntegrityError::MissingProof)?;

    let proof_type = proof.get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrityError::InvalidProof("missing proof type".into()))?;

    if proof_type != "Ed25519Signature2020" {
        return Err(IntegrityError::InvalidProof(
            format!("unsupported proof type: {proof_type}")
        ));
    }

    let signer_did = proof.get("verificationMethod")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrityError::InvalidProof("missing verificationMethod".into()))?
        .to_string();

    let signed_at = proof.get("created")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrityError::InvalidProof("missing created timestamp".into()))?
        .to_string();

    let proof_value = proof.get("proofValue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrityError::InvalidProof("missing proofValue".into()))?;

    // Decode signature from multibase
    let (_, sig_bytes) = multibase::decode(proof_value)
        .map_err(|e| IntegrityError::InvalidSignatureEncoding(e.to_string()))?;

    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| IntegrityError::InvalidSignatureEncoding(
            "invalid Ed25519 signature length".into()
        ))?;

    // Reconstruct the object without proof for verification
    let mut verify_obj = object.clone();
    strip_proof(&mut verify_obj);
    let canonical = canonicalize(&verify_obj);

    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|e| IntegrityError::InvalidKey(e.to_string()))?;

    match verifying_key.verify(canonical.as_bytes(), &signature) {
        Ok(()) => Ok(ProofStatus {
            valid: true,
            signer_did,
            signed_at,
        }),
        Err(_) => Ok(ProofStatus {
            valid: false,
            signer_did,
            signed_at,
        }),
    }
}

/// Remove the `proof` field from an object, returning it if present.
pub fn strip_proof(object: &mut serde_json::Value) -> Option<serde_json::Value> {
    object.as_object_mut()
        .and_then(|map| map.remove("proof"))
}

/// Compute the SRI (Subresource Integrity) hash of raw bytes.
///
/// Returns `sha256-{base64}` format suitable for the `integrity` field
/// in the media table.
pub fn compute_sri_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use multibase::Base;

    let hash = Sha256::digest(data);
    let encoded = multibase::encode(Base::Base64, hash.as_slice());
    // Strip the multibase prefix character for SRI format
    format!("sha256-{}", &encoded[1..])
}

/// Verify data against an SRI hash.
pub fn verify_sri_hash(data: &[u8], expected: &str) -> bool {
    let actual = compute_sri_hash(data);
    // ponytail: constant-time comparison not needed here — SRI hashes
    // are public integrity checks, not secrets.
    actual == expected
}

fn unix_now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Manual ISO 8601 without pulling in chrono
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    // Simplified date calculation (good enough for dates 1970-2100)
    let mut y = 1970i64;
    let mut remaining = days_since_epoch as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut m = 0usize;
    while m < 12 && remaining >= month_days[m] {
        remaining -= month_days[m];
        m += 1;
    }
    let d = remaining + 1;
    let h = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    format!("{y:04}-{:02}-{d:02}T{h:02}:{min:02}:{s:02}Z", m + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fedi_did::{ed25519_to_did_key, generate_recovery_keypair, ed25519_public_from_private};

    #[test]
    fn sign_and_verify_round_trip() {
        let (private_key, public_key) = generate_recovery_keypair();
        let did_key = ed25519_to_did_key(&public_key);

        let mut note = serde_json::json!({
            "type": "Note",
            "attributedTo": "https://example.com/users/alice",
            "content": "Hello, fediverse!"
        });

        sign_object(&mut note, &private_key, &did_key).unwrap();

        assert!(note.get("proof").is_some());
        let proof = &note["proof"];
        assert_eq!(proof["type"], "Ed25519Signature2020");
        assert_eq!(proof["verificationMethod"], did_key);
        assert!(proof["created"].as_str().is_some());
        assert!(proof["proofValue"].as_str().is_some());
        assert_eq!(proof["proofPurpose"], "assertionMethod");

        let status = verify_object(&note, &public_key).unwrap();
        assert!(status.valid);
        assert_eq!(status.signer_did, did_key);
    }

    #[test]
    fn verify_detects_tampered_content() {
        let (private_key, public_key) = generate_recovery_keypair();
        let did_key = ed25519_to_did_key(&public_key);

        let mut note = serde_json::json!({
            "type": "Note",
            "content": "Original content"
        });

        sign_object(&mut note, &private_key, &did_key).unwrap();

        // Tamper with content
        note["content"] = serde_json::Value::String("Tampered content".into());

        let status = verify_object(&note, &public_key).unwrap();
        assert!(!status.valid);
    }

    #[test]
    fn verify_detects_wrong_key() {
        let (private_key, public_key) = generate_recovery_keypair();
        let (_, wrong_public_key) = generate_recovery_keypair();
        let did_key = ed25519_to_did_key(&public_key);

        let mut note = serde_json::json!({
            "type": "Note",
            "content": "Hello"
        });

        sign_object(&mut note, &private_key, &did_key).unwrap();

        let status = verify_object(&note, &wrong_public_key).unwrap();
        assert!(!status.valid);
    }

    #[test]
    fn verify_rejects_missing_proof() {
        let (_, public_key) = generate_recovery_keypair();
        let note = serde_json::json!({"type": "Note", "content": "Hello"});

        let result = verify_object(&note, &public_key);
        assert!(matches!(result, Err(IntegrityError::MissingProof)));
    }

    #[test]
    fn verify_rejects_unsupported_proof_type() {
        let (_, public_key) = generate_recovery_keypair();
        let note = serde_json::json!({
            "type": "Note",
            "proof": {
                "type": "RsaSignature2017",
                "verificationMethod": "did:key:z6Mk...",
                "created": "2026-07-20T00:00:00Z",
                "proofValue": "z..."
            }
        });

        let result = verify_object(&note, &public_key);
        assert!(matches!(result, Err(IntegrityError::InvalidProof(_))));
    }

    #[test]
    fn sign_replaces_existing_proof() {
        let (private_key, public_key) = generate_recovery_keypair();
        let did_key = ed25519_to_did_key(&public_key);

        let mut note = serde_json::json!({
            "type": "Note",
            "content": "Hello",
            "proof": {"type": "old-proof"}
        });

        sign_object(&mut note, &private_key, &did_key).unwrap();

        assert_eq!(note["proof"]["type"], "Ed25519Signature2020");
        let status = verify_object(&note, &public_key).unwrap();
        assert!(status.valid);
    }

    #[test]
    fn sign_is_deterministic_for_same_content() {
        let private_key = [42u8; 32];
        let public_key = ed25519_public_from_private(&private_key);
        let did_key = ed25519_to_did_key(&public_key);

        let mut note1 = serde_json::json!({"type": "Note", "content": "Hello"});
        let mut note2 = serde_json::json!({"content": "Hello", "type": "Note"});

        sign_object(&mut note1, &private_key, &did_key).unwrap();
        sign_object(&mut note2, &private_key, &did_key).unwrap();

        // proofValue should be the same because JCS canonicalization
        // normalizes key ordering
        assert_eq!(
            note1["proof"]["proofValue"],
            note2["proof"]["proofValue"],
        );
    }

    #[test]
    fn strip_proof_removes_and_returns() {
        let mut obj = serde_json::json!({
            "type": "Note",
            "proof": {"type": "test"}
        });

        let proof = strip_proof(&mut obj);
        assert!(proof.is_some());
        assert_eq!(proof.unwrap()["type"], "test");
        assert!(obj.get("proof").is_none());
    }

    #[test]
    fn strip_proof_returns_none_when_absent() {
        let mut obj = serde_json::json!({"type": "Note"});
        assert!(strip_proof(&mut obj).is_none());
    }

    #[test]
    fn sri_hash_deterministic() {
        let data = b"hello world";
        let h1 = compute_sri_hash(data);
        let h2 = compute_sri_hash(data);
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256-"));
    }

    #[test]
    fn sri_hash_verify_valid() {
        let data = b"test data for hashing";
        let hash = compute_sri_hash(data);
        assert!(verify_sri_hash(data, &hash));
    }

    #[test]
    fn sri_hash_verify_invalid() {
        let data = b"original data";
        let hash = compute_sri_hash(data);
        assert!(!verify_sri_hash(b"tampered data", &hash));
    }

    #[test]
    fn iso8601_format_is_valid() {
        let ts = unix_now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }
}
